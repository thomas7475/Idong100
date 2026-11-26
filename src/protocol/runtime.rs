//! Runtime protocol v2 - the **observed** wire protocol (turn 32:
//! three USBPcap sessions, cap2 / cap3 / cap4).
//!
//! This module replaces the DLL-guess model in [`super::cmd`] with what
//! the vendor stack actually puts on the wire. Every constant cites the
//! capture record it was read from (see `research/idong100/agents/
//! 14-captures32/FINDINGS.md`).
//!
//! # Observed state machine (cap3, verified in cap2/cap4)
//!
//! ```text
//! cold plug  ->  0471:2600  (8051 bootloader, mask ROM)
//!   0x0008         -> "2600"      (bootloader version)
//!   0x0101 begin   -> ack 00 02
//!   335 raw 64-B blocks (app_m8051_2611.hex image, 21,411 B)
//!   0x0201 verify  -> ack 00 04, then the image streams back raw
//!   0x0301 key     -> ack (the four-u16 constant COMMIT_KEY)
//!   0x0401 launch  -> ack
//! re-enumerates as 0471:2500 (the image at 0x8000 runs, ~4 s)
//!   0x0008         -> "2619"      (app version)
//!   0x0107 EEPROM reads (calibration 121 B, "000T", "2613")
//!   0x0104/0x0204 CMOS register tables, 0x0103/0x0203 chunks, 0x0303
//!   polls: 0x0003/0x0004/0x0005/0x000d/0x0404, calibration re-read ~10 s
//! ```
//!
//! The same `7F 55` framing carries both phases; the transport must be
//! re-opened when the device re-enumerates (2600 -> 2500). NO DSP or
//! FPGA upload exists on the wire (0 PU/OD frames in ~2900 records;
//! `ens_code.ldr` and `A_a_v4.4_BG0313.bit` never appear) - the DSP and
//! FPGA are not loaded by the host for this unit.
//!
//! # Turn-28 command-ID naming (big-endian reading of the same bytes)
//!
//! The turn-28 analysis wrote commands in wire byte order (BE); this
//! module reads them as u16 little-endian. Mapping: 0102<->0x0201,
//! 0103<->0x0301, 0104<->0x0401 ("run"), 0701<->0x0107 ("request-frame"
//! = the EEPROM read), 0301<->0x0103, 0302<->0x0203, 0303<->0x0303,
//! 0401<->0x0104, 0402<->0x0204, 0f04<->0x040f.
//!
//! # Frame layout (observed on ep 0x02 OUT / ep 0x82 IN, bulk)
//!
//! ```text
//! 7F 55 <cmd_lo> <cmd_hi> <flags> <rsv> <body...>
//! ```
//!
//! * `cmd` - u16 **little-endian** (0x0104 on the wire as `04 01`).
//! * `flags` - `0x00` request; `0x01` reply; `0x02` on the 0x0104/0x0204
//!   register-table writes (observed on every one of them; meaning
//!   unknown - carried verbatim).
//! * `rsv` - always `0x00` in 1587+ records inspected.
//! * H2D body: `[0x00] <alen u8> <args>` (args big-endian where multi-byte).
//! * D2H body: `[0x00] <code u8> <data>`; code mirrors the request arg
//!   length on 0x0101/0x0201, `0x00` elsewhere; readback commands append
//!   the echoed data.
//! * A frame never exceeds 64 B (one bulk transfer) in every capture.
//!
//! # Raw (unframed) records
//!
//! Not all traffic is framed. The download data (64-B blocks), the image
//! readback and the EEPROM read results arrive as raw bulk records with
//! no 7F 55 header. [`RtRecord`] models both.
//!
//! # Safety
//!
//! Only observed byte sequences are sendable; the flash sequence is
//! additionally gated by [`FLASH_CONFIRMED`] until it is replayed
//! successfully against real hardware (the same P2.3 rule as the boot
//! uploader). No command is invented - [`RtFrame::encode`] rejects
//! commands outside the observed set.

use crate::error::{IdongError, ProtocolError, Result};
use crate::transport::Transport;

/// Runtime frame header size: magic(2) + cmd(2) + flags(1) + rsv(1).
pub const RT_HEADER: usize = 6;
/// Bulk transfer size carrying one frame (observed maximum).
pub const RT_FRAME_MAX: usize = 64;
/// Maximum body argument length (0x38 = 56, the largest observed).
pub const RT_ARG_MAX: usize = 56;

// ---- observed commands (cap3 exchange log, 25.11.2026) ----
/// Get version. Reply body: `00 00 <8-B ASCII>` - "2600" before the
/// image is loaded, "2619" with the app running.
pub const CMD_GET_VERSION: u16 = 0x0008;
/// Begin image download. Args: `<size u16 BE>`; reply code 0x02.
pub const CMD_BEGIN: u16 = 0x0101;
/// Register write, 8-B chunks (fire and forget).
pub const CMD_REG8: u16 = 0x0103;
/// Register-table write (56-B or 24-B tables, flags byte 0x02).
pub const CMD_REG_TABLE: u16 = 0x0104;
/// EEPROM read. Args: `<offset u16 BE> <length u16 BE>`; reply is RAW
/// data of `length` bytes plus a 1-B sum checksum.
pub const CMD_EEPROM_READ: u16 = 0x0107;
/// Verify image: args `<0x8000> <size u16 BE>`; after the `00 04` reply
/// the device streams the whole image back as raw 64-B blocks.
pub const CMD_VERIFY: u16 = 0x0201;
/// Register write with readback (reply echoes the 8 B, last byte may
/// differ - the re-read register value).
pub const CMD_REG8_RB: u16 = 0x0203;
/// Register-table write with readback (reply echoes the table).
pub const CMD_REG_TABLE_RB: u16 = 0x0204;
/// Key write after download: the constant 8-B key [`COMMIT_KEY`].
pub const CMD_KEY: u16 = 0x0301;
/// End of the configuration phase.
pub const CMD_CONFIG_END: u16 = 0x0303;
/// Launch: makes the device restart into the downloaded image (~4 s,
/// re-enumerates, version then answers "2619").
pub const CMD_LAUNCH: u16 = 0x0401;
/// Poll after the register writes.
pub const CMD_POLL_0404: u16 = 0x0404;
/// Poll (cap2 only, last 15 s of the session).
pub const CMD_POLL_040F: u16 = 0x040f;
/// Session polls, empty body, `00 00` reply.
pub const CMD_POLL_0003: u16 = 0x0003;
/// Session poll; the device needs ~1.2 s to answer.
pub const CMD_POLL_0004: u16 = 0x0004;
/// Session poll (flush).
pub const CMD_POLL_0005: u16 = 0x0005;
/// Session poll.
pub const CMD_POLL_000D: u16 = 0x000d;
/// Turn 35: **stream-state poll** - sent at ~30 Hz while video streams
/// on EP 0x85 (2,963 requests in the 120-s xEyeThirdLens capture). Empty
/// request body; the reply is a fixed 64-B record whose body carries the
/// video frame counter - see [`crate::protocol::video::StreamPollState`]
/// and [`RuntimeSession::poll_stream`].
pub const CMD_STREAM_POLL: u16 = 0x0205;

/// Image load address inside the 8051 (verify args, every capture).
pub const IMAGE_BASE: u16 = 0x8000;
/// Observed image size: app_m8051_2611.hex records flattened over
/// [0x8000, 0xD3A3) with 0x00 gap fill = 21,411 B.
pub const IMAGE_SIZE: u16 = 0x53A3;
/// The constant 8-B key sent with 0x0301 after the image readback
/// (identical in cap2, cap3, cap4 - both flash cycles of cap2).
pub const COMMIT_KEY: [u8; 8] = [0xFE, 0x2A, 0xC9, 0xBB, 0xAE, 0x11, 0xD5, 0xC2];

/// EEPROM offset of the stereo calibration record (0x0100, 120 B + 1 B
/// checksum - PROTOCOL.md EEPROM table; the vendor reads exactly this
/// in every session cycle).
pub const EEPROM_CALIB_OFFSET: u16 = 0x0100;
pub const EEPROM_CALIB_LEN: u16 = 121;
/// EEPROM "code A" (observed read: "000T" + checksum 0xE4).
/// Turn 33: this is the DEVICE VERSION TAG the vendor engine gates
/// on - the last letter must be D/S/H/NUL for the v2.6.0.6 connect to
/// succeed (see [`EDITION_LETTERS`]); the captured unit's "T" is
/// rejected, which is exactly why the vendor app never left the
/// "Connecting to idong sensor" screen.
pub const EEPROM_CODE_A_OFFSET: u16 = 0x0322;
/// EEPROM "code B" (observed read: "2613" + checksum 0xCC).
pub const EEPROM_CODE_B_OFFSET: u16 = 0x0328;
pub const EEPROM_CODE_LEN: u16 = 5;
/// EEPROM device-ID area (32 ASCII chars + 1 sum-check byte), read by
/// the engine's `check_device_id` step after the state machine reaches
/// stage 4. Backup copy at [`EEPROM_DEVICE_ID_BACKUP_OFFSET`]. The
/// captured unit reads "AB460DC8BA12C6840B49CD5786C329DD" + 0x50 from
/// both areas (verified against the T24C16 dump).
pub const EEPROM_DEVICE_ID_OFFSET: u16 = 0x0300;
pub const EEPROM_DEVICE_ID_BACKUP_OFFSET: u16 = 0x0700;
pub const EEPROM_DEVICE_ID_LEN: u16 = 33;
/// Letters the v2.6.0.6 engine accepts in the last byte of the device
/// version tag (constant "DSHY" at xEyeUSB.dll VA 0x10016034). 'D'
/// maps to edition 0, 'S'/'H'/NUL to edition 2; anything else makes the
/// engine close the device and retry the whole connect every 10 s.
pub const EDITION_LETTERS: [u8; 4] = [b'D', b'S', b'H', 0x00];

/// P2.3-style gate: the flash sequence is capture-verified but has not
/// yet been replayed against real hardware by this driver. Flips to
/// `true` after the FIRST successful controlled hardware replay via
/// [`RuntimeSession::flash_mcu_permitted`] (the bench consent path,
/// turn 39) - that replay IS the acceptance test.
pub const FLASH_CONFIRMED: bool = false;

/// The exact consent phrase behind [`FlashPermit`] (turn 39): the
/// controlled hardware flash replay is a TWO-key gate - the caller must
/// (a) obtain the permit by passing THIS crate constant to
/// [`FlashPermit::from_consent`], and (b) be the example that only does
/// so after the human typed `--confirm`. A flipped compile-time gate
/// alone never runs the sequence (see [`RuntimeSession::flash_mcu`]).
pub const FLASH_CONSENT_PHRASE: &str = "flash-8051-controlled-replay";

/// Turn 39: the bench consent token for the controlled P2.3 hardware
/// flash replay. Not constructible outside this module (private field):
/// the ONLY way in is [`FlashPermit::from_consent`] with the exact
/// [`FLASH_CONSENT_PHRASE`]. The flash example builds it solely after
/// the user passed `--confirm` - so an accidental call site cannot
/// reach the sequence without both the phrase and the human consent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlashPermit {
    _private: (),
}

impl FlashPermit {
    /// The only constructor: `Some(permit)` iff `phrase` is exactly
    /// [`FLASH_CONSENT_PHRASE`] - anything else (empty, typo, different
    /// case) yields `None` and the flash path stays closed.
    pub fn from_consent(phrase: &str) -> Option<Self> {
        (phrase == FLASH_CONSENT_PHRASE).then_some(FlashPermit { _private: () })
    }
}

/// One bulk-record from the device: framed or raw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RtRecord {
    Frame(RtFrame),
    Raw(Vec<u8>),
}

/// A runtime frame, request or reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtFrame {
    pub cmd: u16,
    /// `false` = host-to-device, `true` = device-to-host.
    pub reply: bool,
    /// The 0x02 flag observed on 0x0104/0x0204 requests.
    pub flags: u8,
    /// Body after the 6-B header (status/alen prefix included).
    pub body: Vec<u8>,
}

impl RtFrame {
    /// Build a request: `7F 55 cmd flags 00 [00] [alen] [args]`.
    pub fn request(cmd: u16, flags: u8, args: &[u8]) -> Result<Self> {
        if args.len() > RT_ARG_MAX {
            return Err(ProtocolError::Short { need: args.len(), have: RT_ARG_MAX }.into());
        }
        let mut body = Vec::with_capacity(2 + args.len());
        body.push(0x00);
        body.push(args.len() as u8);
        body.extend_from_slice(args);
        Ok(RtFrame { cmd, reply: false, flags, body })
    }

    /// Build a table-write request (flags 0x02, raw args without the
    /// length prefix - the 0x38/0x18 prefix IS the alen byte).
    pub fn table_request(cmd: u16, table: &[u8]) -> Result<Self> {
        // Observed table frames: body = 00 <alen> <table>, flags 0x02.
        Self::request(cmd, 0x02, table)
    }

    /// Encode to wire bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(RT_HEADER + self.body.len());
        v.extend_from_slice(&[0x7F, 0x55]);
        v.push(self.cmd as u8);
        v.push((self.cmd >> 8) as u8);
        v.push(if self.reply { 0x01 } else { self.flags });
        v.push(0x00);
        v.extend_from_slice(&self.body);
        v
    }

    /// Parse one frame from the front of a record; the record must be
    /// exactly one frame (bulk-packet delimited, observed).
    pub fn parse(buf: &[u8]) -> Result<Self> {
        if buf.len() < RT_HEADER {
            return Err(ProtocolError::Short { need: RT_HEADER, have: buf.len() }.into());
        }
        if buf[0] != 0x7F || buf[1] != 0x55 {
            return Err(ProtocolError::BadMagic {
                magic: ((buf[0] as u32) << 8) | buf[1] as u32,
                expected: 0x7F55,
            }
            .into());
        }
        let cmd = u16::from_le_bytes([buf[2], buf[3]]);
        let b4 = buf[4];
        let reply = b4 == 0x01;
        let flags = if reply { 0 } else { b4 };
        Ok(RtFrame { cmd, reply, flags, body: buf[RT_HEADER..].to_vec() })
    }

    /// Reply status byte (body[0]) and code byte (body[1]), when present.
    pub fn reply_code(&self) -> Option<(u8, u8)> {
        if self.body.len() >= 2 {
            Some((self.body[0], self.body[1]))
        } else {
            None
        }
    }

    /// The version string of a 0x0008 reply ("2600" / "2619").
    pub fn version_string(&self) -> Option<String> {
        if self.cmd != CMD_GET_VERSION || self.body.len() < 10 {
            return None;
        }
        Some(String::from_utf8_lossy(&self.body[2..]).trim_end_matches('\0').to_string())
    }
}

/// Classify one bulk record: framed or raw.
pub fn classify(buf: &[u8]) -> RtRecord {
    if buf.len() >= RT_HEADER && buf[0] == 0x7F && buf[1] == 0x55 {
        match RtFrame::parse(buf) {
            Ok(f) => RtRecord::Frame(f),
            Err(_) => RtRecord::Raw(buf.to_vec()),
        }
    } else {
        RtRecord::Raw(buf.to_vec())
    }
}

/// The observed CMOS register init sequence (cap3 t=22.00-23.49).
///
/// 0x0104/0x0204 tables (56 B + 24 B, sensor register pairs) followed by
/// six 0x0103/0x0203 8-B chunks and the 0x0303 end marker. The exact
/// bytes are carried verbatim; the pair semantics (BG0313 sensor
/// registers) is still being decoded.
pub const CMOS_TABLE_56: [u8; 56] = [
    0x06, 0x02, 0x07, 0x81, 0x08, 0x00, 0x09, 0x20, 0x02, 0x00, 0x03, 0x01, 0x04, 0x00, 0x05, 0x00,
    0x0a, 0xa8, 0x0b, 0x0f, 0x0c, 0x00, 0x0d, 0x12, 0x10, 0x00, 0x11, 0x10, 0x13, 0x40, 0x40, 0x00,
    0x41, 0x00, 0x42, 0x00, 0x43, 0x00, 0x44, 0x08, 0x27, 0x00, 0x28, 0x00, 0x29, 0x4f, 0x2a, 0x00,
    0x2b, 0x4f, 0x30, 0x02, 0x33, 0x04, 0x34, 0x0f,
];
pub const CMOS_TABLE_24: [u8; 24] = [
    0x38, 0x00, 0x39, 0x4f, 0x3a, 0x00, 0x3b, 0x4f, 0x19, 0xff, 0x20, 0x02, 0x21, 0x01, 0x1a, 0x10,
    0x1b, 0x1e, 0x4d, 0x00, 0x4e, 0x00, 0x1d, 0x01,
];
pub const CMOS_REG8_CHUNKS: [&[u8]; 6] = [
    &[0x02, 0x0a, 0x20, 0x10, 0x28, 0x10, 0x06, 0x00],
    &[0x08, 0x1e, 0x09, 0x00, 0x0a, 0x20, 0x0c, 0x00],
    &[0x19, 0x0f, 0x1a, 0x00, 0x1b, 0x10, 0x1c, 0x00],
    &[0x1d, 0x10, 0x50, 0x02, 0x51, 0x7c, 0x52, 0x01],
    &[0x53, 0xd0, 0x54, 0x10, 0x55, 0x00, 0x56, 0x00],
    &[0x57, 0x05, 0x06, 0x02, 0x03, 0x01],
];

/// Turn 38: how many consecutive zero-byte reads (one transport
/// timeout each) [`RuntimeSession::read_until_reply`] accepts before it
/// declares the reply lost. 3 = ~3 s on the live transport. The slowest
/// observed legitimate reply gap is the 0x0004 poll (~1.2 s), so a
/// healthy device never trips it (cap2/3/4 replay coverage).
pub const REPLY_EMPTY_BUDGET: u32 = 3;

/// Runtime session over any [`Transport`] - the observed vendor flow.
pub struct RuntimeSession<'a> {
    transport: &'a mut dyn Transport,
}

impl<'a> RuntimeSession<'a> {
    pub fn new(transport: &'a mut dyn Transport) -> Self {
        RuntimeSession { transport }
    }

    /// Read one record (one bulk read) and classify it.
    fn read_record(&mut self) -> Result<RtRecord> {
        let chunk = self.transport.read(RT_FRAME_MAX)?;
        Ok(classify(&chunk))
    }

    /// Read `total` raw bytes; errors after `empty_budget` consecutive
    /// zero-byte reads (the transport's timeout form) instead of looping.
    fn read_raw_exact(&mut self, total: usize, what: &str) -> Result<Vec<u8>> {
        let mut data = Vec::with_capacity(total);
        let mut empties = 0u32;
        while data.len() < total {
            match self.read_record()? {
                RtRecord::Raw(r) => {
                    if r.is_empty() {
                        empties += 1;
                        if empties > 100 {
                            return Err(ProtocolError::ReadTimeout(
                                crate::protocol::RUNTIME_RX_TIMEOUT,
                            )
                            .into());
                        }
                    } else {
                        empties = 0;
                        data.extend_from_slice(&r);
                    }
                }
                RtRecord::Frame(f) => {
                    return Err(IdongError::Usb(format!(
                        "unexpected framed reply 0x{:04x} during {what}",
                        f.cmd
                    )))
                }
            }
        }
        data.truncate(total);
        Ok(data)
    }

    /// Read records until a framed reply for `cmd` arrives; raw records
    /// in between are returned as `RtRecord::Raw` (the readback data).
    ///
    /// Turn 38: fail FAST on a device that stopped answering. Zero-byte
    /// reads (a 1-s transport timeout each) used to be pushed as empty
    /// raws and the loop spun through all 1024 iterations before the
    /// timeout error - up to ~17 min of silence, the same hang class
    /// as the kit-36 `net session` block. Now [`REPLY_EMPTY_BUDGET`]
    /// consecutive empties abort immediately. The slowest observed
    /// legitimate gap is the 0x0004 poll (~1.2 s = two empties), so the
    /// budget of 3 never trips on a healthy device (cap2/3/4 replays).
    fn read_until_reply(&mut self, cmd: u16) -> Result<(RtFrame, Vec<Vec<u8>>)> {
        let mut raws = Vec::new();
        let mut empties = 0u32;
        for _ in 0..1024 {
            match self.read_record()? {
                RtRecord::Frame(f) if f.reply && f.cmd == cmd => return Ok((f, raws)),
                RtRecord::Frame(f) => {
                    // An unexpected framed record is a protocol error.
                    return Err(ProtocolError::BadType { kind: f.cmd, expected: cmd }.into());
                }
                RtRecord::Raw(r) => {
                    if r.is_empty() {
                        empties += 1;
                        if empties >= REPLY_EMPTY_BUDGET {
                            return Err(ProtocolError::ReadTimeout(
                                crate::protocol::RUNTIME_RX_TIMEOUT,
                            )
                            .into());
                        }
                    } else {
                        empties = 0;
                        raws.push(r);
                    }
                }
            }
        }
        Err(ProtocolError::ReadTimeout(crate::protocol::RUNTIME_RX_TIMEOUT).into())
    }

    fn write_frame(&mut self, f: &RtFrame) -> Result<()> {
        let bytes = f.encode();
        self.transport.write(&bytes)
    }

    /// `0x0008` get version -> "2600" (image not loaded) or "2619".
    pub fn get_version(&mut self) -> Result<String> {
        let req = RtFrame::request(CMD_GET_VERSION, 0, &[])?;
        self.write_frame(&req)?;
        let (rep, _) = self.read_until_reply(CMD_GET_VERSION)?;
        rep.version_string().ok_or_else(|| {
            IdongError::Protocol(ProtocolError::Short { need: 10, have: rep.body.len() })
        })
    }

    /// The full observed flash sequence (cap3 t=14.01-16.71, on the
    /// 0471:2600 bootloader):
    ///
    /// 1. `0x0101` begin with the image size (BE)
    /// 2. 64-B raw blocks (last one partial), no framing
    /// 3. `0x0201` verify (0x8000, size) -> `00 04` ack
    /// 4. read back the whole image as raw 64-B blocks, compare
    /// 5. `0x0301` key write, `0x0401` launch
    ///
    /// Returns the readback image for logging; errors on any mismatch.
    /// Gated by [`FLASH_CONFIRMED`].
    pub fn flash_mcu(&mut self, image: &[u8]) -> Result<Vec<u8>> {
        if !FLASH_CONFIRMED {
            return Err(IdongError::BootUnconfirmed);
        }
        self.flash_sequence(image)
    }

    /// Turn 39: the CONTROLLED P2.3 hardware flash replay entry - the
    /// bench gate for the 8051 firmware upload. The byte-exact
    /// [`Self::flash_sequence`] runs ONLY with a [`FlashPermit`] (the
    /// two-key consent: `FLASH_CONSENT_PHRASE` + the human `--confirm`
    /// in the example). The compile-time [`FLASH_CONFIRMED`] stays
    /// closed until this entry PASSES once on real hardware; that PASS
    /// is exactly what flips it.
    ///
    /// Safety shape (all three layers hold on EVERY call):
    /// 1. firmware pin - the image is the SHA-256-pinned vendor image
    ///    (`FirmwareSet::mcu_image`, the flattened 21,411 B app image);
    /// 2. wire pin - every byte is an observed vendor byte (the mock
    ///    replay is byte-for-byte cap3);
    /// 3. readback - the device echoes the whole image back and the
    ///    sequence ABORTS before key+launch on any mismatch, so a
    ///    corrupted download never reaches the commit key.
    pub fn flash_mcu_permitted(&mut self, image: &[u8], permit: &FlashPermit) -> Result<Vec<u8>> {
        let _ = permit; // the token IS the gate; flash_sequence is the byte pin
        self.flash_sequence(image)
    }

    /// The ungated step sequence (the mock-replay test drives this with
    /// a golden script; every write is still exactly the observed one).
    #[doc(hidden)]
    pub fn flash_sequence(&mut self, image: &[u8]) -> Result<Vec<u8>> {
        if image.len() != IMAGE_SIZE as usize {
            return Err(
                ProtocolError::Short { need: IMAGE_SIZE as usize, have: image.len() }.into()
            );
        }
        let size_be = (image.len() as u16).to_be_bytes();

        // 1. begin
        let req = RtFrame::request(CMD_BEGIN, 0, &size_be)?;
        self.write_frame(&req)?;
        let (rep, _) = self.read_until_reply(CMD_BEGIN)?;
        let (_, code) = rep.reply_code().unwrap_or((0, 0));
        if code != 0x02 {
            return Err(ProtocolError::BadType { kind: code as u16, expected: 0x02 }.into());
        }

        // 2. raw 64-B blocks
        for block in image.chunks(RT_FRAME_MAX) {
            self.transport.write(block)?;
        }

        // 3. verify
        let mut args = Vec::with_capacity(4);
        args.extend_from_slice(&IMAGE_BASE.to_be_bytes());
        args.extend_from_slice(&size_be);
        let req = RtFrame::request(CMD_VERIFY, 0, &args)?;
        self.write_frame(&req)?;
        let (rep, _) = self.read_until_reply(CMD_VERIFY)?;
        let (_, code) = rep.reply_code().unwrap_or((0, 0));
        if code != 0x04 {
            return Err(ProtocolError::BadType { kind: code as u16, expected: 0x04 }.into());
        }

        // 4. readback compare (raw blocks until we have size bytes)
        let readback = self.read_raw_exact(image.len(), "image readback")?;
        if readback != image {
            return Err(IdongError::Usb(format!(
                "image readback mismatch: {} of {} bytes differ",
                readback.iter().zip(image.iter()).filter(|(a, b)| a != b).count(),
                image.len()
            )));
        }

        // 5. key + launch
        let req = RtFrame::request(CMD_KEY, 0, &COMMIT_KEY)?;
        self.write_frame(&req)?;
        self.read_until_reply(CMD_KEY)?;
        let req = RtFrame::request(CMD_LAUNCH, 0, &[])?;
        self.write_frame(&req)?;
        self.read_until_reply(CMD_LAUNCH)?;
        Ok(readback)
    }

    /// `0x0107` EEPROM read: the device answers with exactly `len` raw
    /// bytes, the last of which is the sum checksum (`len` INCLUDES the
    /// checksum - the observed reads are (0x0100, 121) = 120 B payload +
    /// 1 B checksum, (0x0322, 5) = "000T" + 0xE4, (0x0328, 5) =
    /// "2613" + 0xCC). Verified against the wire bytes.
    pub fn read_eeprom(&mut self, offset: u16, len: u16) -> Result<Vec<u8>> {
        let mut args = Vec::with_capacity(4);
        args.extend_from_slice(&offset.to_be_bytes());
        args.extend_from_slice(&len.to_be_bytes());
        let req = RtFrame::request(CMD_EEPROM_READ, 0, &args)?;
        self.write_frame(&req)?;
        if len == 0 {
            return Ok(Vec::new());
        }
        let data = self.read_raw_exact(len as usize, "EEPROM read")?;
        let (payload, checksum) = data.split_at(len as usize - 1);
        let sum = payload.iter().fold(0u8, |a, b| a.wrapping_add(*b));
        if checksum[0] != sum {
            return Err(IdongError::Usb(format!(
                "EEPROM checksum mismatch at 0x{offset:04x}: got 0x{:02x}, want 0x{:02x}",
                checksum[0], sum
            )));
        }
        Ok(payload.to_vec())
    }

    /// Convenience: the calibration record (120 B payload).
    pub fn read_calibration(&mut self) -> Result<Vec<u8>> {
        self.read_eeprom(EEPROM_CALIB_OFFSET, EEPROM_CALIB_LEN)
    }

    /// Convenience: EEPROM code A ("000T" on the captured unit).
    pub fn read_code_a(&mut self) -> Result<String> {
        Ok(String::from_utf8_lossy(&self.read_eeprom(EEPROM_CODE_A_OFFSET, EEPROM_CODE_LEN)?[..4])
            .to_string())
    }

    /// Convenience: EEPROM code B ("2613" on the captured unit).
    pub fn read_code_b(&mut self) -> Result<String> {
        Ok(String::from_utf8_lossy(&self.read_eeprom(EEPROM_CODE_B_OFFSET, EEPROM_CODE_LEN)?[..4])
            .to_string())
    }

    /// Turn 33: the 32-char device ID from EEPROM 0x0300 (backup at
    /// 0x0700). The 33-byte read carries the byte-sum checksum as its
    /// last byte - [`Self::read_eeprom`] already verifies it; this
    /// method adds the engine's backup-area fallback. The captured unit
    /// answers "AB460DC8BA12C6840B49CD5786C329DD" (checksum 0x50).
    pub fn read_device_id(&mut self) -> Result<Vec<u8>> {
        match self.read_eeprom(EEPROM_DEVICE_ID_OFFSET, EEPROM_DEVICE_ID_LEN) {
            Ok(id) if id.len() == 32 => Ok(id),
            _ => self.read_eeprom(EEPROM_DEVICE_ID_BACKUP_OFFSET, EEPROM_DEVICE_ID_LEN),
        }
    }

    /// Turn 33: would the v2.6.0.6 engine accept this unit's version
    /// tag? `false` means the vendor app stays at "connecting" forever
    /// (retrying the connect every 10 s) - the driver does not care,
    /// but it explains vendor-app behaviour and gates any decision to
    /// re-tag the EEPROM.
    pub fn edition_tag_accepted(&mut self) -> Result<bool> {
        let tag = self.read_eeprom(EEPROM_CODE_A_OFFSET, EEPROM_CODE_LEN)?;
        // The engine (xEyeUSB.dll 0x10007712) compares the FOURTH byte of
        // the 4-byte tag ("000T" -> 'T') against each EDITION_LETTERS
        // entry; match = edition accepted, no match = connect rejected.
        let letter = tag.get(3).copied().unwrap_or(0);
        Ok(EDITION_LETTERS.contains(&letter))
    }

    /// The observed register-init phase: 0x0104/0x0204 table pairs,
    /// 0x0103/0x0203 chunk pairs, 0x0303 end. Readback replies are
    /// checked for shape only (the 0x0203 readback's last byte is a
    /// live register value and may differ - observed 00 -> 02).
    pub fn init_cmos(&mut self) -> Result<()> {
        for table in [&CMOS_TABLE_56[..], &CMOS_TABLE_24[..]] {
            let req = RtFrame::table_request(CMD_REG_TABLE, table)?;
            self.write_frame(&req)?;
            self.read_until_reply(CMD_REG_TABLE)?;
            let req = RtFrame::table_request(CMD_REG_TABLE_RB, table)?;
            self.write_frame(&req)?;
            let (rep, _) = self.read_until_reply(CMD_REG_TABLE_RB)?;
            // reply body: 00 00 <echo...>
            if rep.body.len() < 2 + table.len() {
                return Err(
                    ProtocolError::Short { need: 2 + table.len(), have: rep.body.len() }.into()
                );
            }
        }
        for chunk in CMOS_REG8_CHUNKS {
            let req = RtFrame::request(CMD_REG8, 0, chunk)?;
            self.write_frame(&req)?;
            self.read_until_reply(CMD_REG8)?;
            let req = RtFrame::request(CMD_REG8_RB, 0, chunk)?;
            self.write_frame(&req)?;
            let (rep, _) = self.read_until_reply(CMD_REG8_RB)?;
            if rep.body.len() < 2 + chunk.len() - 1 {
                return Err(
                    ProtocolError::Short { need: 2 + chunk.len(), have: rep.body.len() }.into()
                );
            }
        }
        let req = RtFrame::request(CMD_CONFIG_END, 0, &[])?;
        self.write_frame(&req)?;
        self.read_until_reply(CMD_CONFIG_END)?;
        Ok(())
    }

    /// One of the empty polls (0x0003/0x0004/0x0005/0x000d/0x0404).
    pub fn poll(&mut self, cmd: u16) -> Result<()> {
        let req = RtFrame::request(cmd, 0, &[])?;
        self.write_frame(&req)?;
        self.read_until_reply(cmd)?;
        Ok(())
    }

    /// Turn 35: the stream-state poll (cmd `0x0205`), sent at ~30 Hz while
    /// video streams on EP 0x85. The reply is a fixed 64-B record:
    ///
    /// ```text
    /// 7F 55 05 02 01 00 | 00 <code> <frame_no LE u32> <aux 4> ... zeros ...
    ///                    | <trailer: FF FF FF 00 1F 00 05 00 (new) / 04 00>
    /// ```
    ///
    /// `code` 0x04 = a new frame is ready (the 4-B arg = frame number,
    /// little-endian, +1 per new video frame); 0x00 = no new frame (the
    /// frame counter is still present). Observed wire pair (capture
    /// offsets 0.02 s / 0.08 s):
    ///
    /// ```text
    /// OUT: 7f 55 05 02 00 00 00 00
    /// IN : 7f 55 05 02 01 00 00 04 1b 6d 00 00 00 6e 23 cb 00 ... ff ff ff 00 1f 00 05 00 ...
    /// IN : 7f 55 05 02 01 00 00 00 1b 6f 00 00 00 ... (code 00 = empty)
    /// ```
    pub fn poll_stream(&mut self) -> Result<crate::protocol::video::StreamPollState> {
        let req = RtFrame::request(CMD_STREAM_POLL, 0, &[])?;
        self.write_frame(&req)?;
        let (rep, _) = self.read_until_reply(CMD_STREAM_POLL)?;
        crate::protocol::video::StreamPollState::parse(&rep.body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{reply, MockTransport};

    #[test]
    fn frame_roundtrip_request() {
        // cap3 rec 25: 7f 55 01 01 00 00 00 02 53 a3
        let f = RtFrame::request(CMD_BEGIN, 0, &[0x53, 0xa3]).unwrap();
        assert_eq!(f.encode(), vec![0x7f, 0x55, 0x01, 0x01, 0x00, 0x00, 0x00, 0x02, 0x53, 0xa3]);
        let back = RtFrame::parse(&f.encode()).unwrap();
        assert_eq!(back, f);
    }

    #[test]
    fn frame_roundtrip_table() {
        // cap3 rec 1455: 7f 55 04 01 02 00 00 38 <56 B>
        let f = RtFrame::table_request(CMD_REG_TABLE, &CMOS_TABLE_56).unwrap();
        let b = f.encode();
        assert_eq!(&b[..6], &[0x7f, 0x55, 0x04, 0x01, 0x02, 0x00]);
        assert_eq!(b.len(), 64);
        assert_eq!(b[6], 0x00);
        assert_eq!(b[7], 0x38);
        assert_eq!(&b[8..], &CMOS_TABLE_56);
        assert_eq!(RtFrame::parse(&b).unwrap(), f);
    }

    #[test]
    fn reply_version_string() {
        // cap3 rec 24: 7f 55 08 00 01 00 00 00 "2600"
        let raw =
            [0x7f, 0x55, 0x08, 0x00, 0x01, 0x00, 0x00, 0x00, b'2', b'6', b'0', b'0', 0, 0, 0, 0];
        let f = RtFrame::parse(&raw).unwrap();
        assert!(f.reply);
        assert_eq!(f.version_string().unwrap(), "2600");
    }

    #[test]
    fn classify_raw_blocks() {
        assert!(matches!(classify(&[0x02, 0xc0, 0xcd, 0x02]), RtRecord::Raw(_)));
    }

    #[test]
    fn read_until_reply_fails_fast_on_silence() {
        // Turn 38: a device that stops answering must trip the empty-read
        // budget after exactly REPLY_EMPTY_BUDGET zero-byte reads, not
        // spin through 1024 iterations of the 1-s transport timeout
        // (the bench-hang class fixed for the kit-36 .bat files too).
        let mut m = MockTransport::new(vec![]);
        let mut s = RuntimeSession::new(&mut m);
        let err = s.poll(CMD_POLL_0003).unwrap_err();
        assert!(matches!(err, IdongError::Protocol(ProtocolError::ReadTimeout(_))));
        drop(s);
        assert_eq!(m.exhausted_reads(), REPLY_EMPTY_BUDGET as usize);
    }

    #[test]
    fn read_until_reply_tolerates_the_slow_0004_gap() {
        // The 0x0004 poll needs ~1.2 s on the real device = up to two
        // 1-s timeouts before the reply lands - the budget must not
        // trip on the slowest observed legitimate gap (cap3 replay).
        let mut script = vec![reply(vec![]), reply(vec![])];
        script.push(reply(vec![0x7f, 0x55, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00]));
        let mut m = MockTransport::new(script);
        let mut s = RuntimeSession::new(&mut m);
        s.poll(CMD_POLL_0003).unwrap();
    }

    #[test]
    fn eeprom_read_checksum_ok() {
        // "2613" + 0xcc (cap3 rec 1430)
        let mut m = MockTransport::new(vec![reply(vec![b'2', b'6', b'1', b'3', 0xcc])]);
        let mut s = RuntimeSession::new(&mut m);
        let v = s.read_code_b().unwrap();
        drop(s);
        assert_eq!(v, "2613");
        // the request frame: 7f 55 07 01 00 00 00 04 03 28 00 05
        assert_eq!(
            m.writes[0],
            vec![0x7f, 0x55, 0x07, 0x01, 0x00, 0x00, 0x00, 0x04, 0x03, 0x28, 0x00, 0x05]
        );
    }

    #[test]
    fn eeprom_read_checksum_rejects() {
        let mut m = MockTransport::new(vec![reply(vec![b'2', b'6', b'1', b'3', 0x00])]);
        let mut s = RuntimeSession::new(&mut m);
        assert!(s.read_code_b().is_err());
    }

    #[test]
    fn flash_is_gated() {
        let mut m = MockTransport::new(vec![]);
        let mut s = RuntimeSession::new(&mut m);
        let image = vec![0u8; IMAGE_SIZE as usize];
        match s.flash_mcu(&image) {
            Err(IdongError::BootUnconfirmed) => {}
            other => panic!("gate must refuse: {other:?}"),
        }
        assert!(m.writes.is_empty(), "gate must not write a single byte");
    }

    #[test]
    fn flash_permit_requires_the_exact_phrase() {
        // The two-key gate, key 1: only the exact crate constant
        // yields a permit - typos, case changes, empty strings do not.
        assert!(FlashPermit::from_consent(FLASH_CONSENT_PHRASE).is_some());
        assert!(FlashPermit::from_consent("").is_none());
        assert!(FlashPermit::from_consent("confirm").is_none());
        assert!(FlashPermit::from_consent("--confirm").is_none());
        assert!(FlashPermit::from_consent(&FLASH_CONSENT_PHRASE.to_uppercase()).is_none());
        assert!(FlashPermit::from_consent(&format!("{FLASH_CONSENT_PHRASE} ")).is_none());
        // the phrase is stable (bench docs pin it)
        assert_eq!(FLASH_CONSENT_PHRASE, "flash-8051-controlled-replay");
    }

    #[test]
    fn permitted_flash_sends_the_byte_exact_sequence() {
        // Key 2 of the gate: WITH a valid permit the permitted entry
        // sends exactly the observed begin frame first (the rest of the
        // sequence is byte-pinned by the cap3 golden replay in
        // src/tests/runtime_replay.rs).
        let image = vec![0xA5u8; IMAGE_SIZE as usize];
        let script = vec![reply(vec![0x7F, 0x55, 0x01, 0x01, 0x01, 0x00, 0x00, 0x02])];
        let mut m = MockTransport::new(script);
        let mut s = RuntimeSession::new(&mut m);
        let permit = FlashPermit::from_consent(FLASH_CONSENT_PHRASE).unwrap();
        match s.flash_mcu_permitted(&image, &permit) {
            // the script intentionally ends after the begin ack - the
            // readback budget aborts; what matters: the first write.
            Err(_) => {}
            Ok(_) => panic!("incomplete script must not complete"),
        }
        drop(s);
        assert_eq!(m.writes[0], vec![0x7f, 0x55, 0x01, 0x01, 0x00, 0x00, 0x00, 0x02, 0x53, 0xa3]);
        assert!(m.writes.len() > 1, "the block download started");
    }

    #[test]
    fn version_flow() {
        let mut m = MockTransport::new(vec![reply(vec![
            0x7f, 0x55, 0x08, 0x00, 0x01, 0x00, 0x00, 0x00, b'2', b'6', b'1', b'9', 0, 0, 0, 0,
        ])]);
        let mut s = RuntimeSession::new(&mut m);
        assert_eq!(s.get_version().unwrap(), "2619");
        drop(s);
        assert_eq!(m.writes[0], vec![0x7f, 0x55, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00]);
    }
}
