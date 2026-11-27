//! Runtime record plane - the PU/OD framing on EP 0x83/0x03 (turn 46),
//! plus the ENS startup that brings the plane up (turn 49: the cold
//! machine @RVA 0xf790 and the hot-start probe @0x10640, statically
//! disassembled from xEyeUSB.dll - no game capture needed).
//!
//! Source: xEyeUSB.dll v2.6.0.6 disassembly (the `usbCom` worker @RVA
//! 0x11770 and the `ens` subsystem @0xeb00-0x10900; artifacts in
//! `research/idong100/agents/20-turn46/` and `.../21-turn49/`). This
//! is the third protocol layer: the 7F 55 session stays on EP
//! 0x02/0x82, the record plane runs on the second MI_04 bulk pair.
//! EasyUSB pipe calibration: the 7F 55 engine writes pipe 0 / reads
//! pipe 1 (USBPcap-proven EP 0x02/0x82), so usbCom's read pipe 2 /
//! write pipe 3 are EP 0x83/0x03.
//!
//! The vendor exchange, per `usbCom` loop:
//!
//! 1. read a 64-B OD frame of type [`TYPE_RECORD_READY`] (0xF0) from
//!    EP 0x83 - one frame announces one record;
//! 2. read the 640-B raw record from EP 0x83 (it ends with the OD
//!    magic dword at +0x27C);
//! 3. write a 64-B PU frame of type [`TYPE_RECORD_CMD`] (7) to EP
//!    0x03 - the ACK for the NEXT record, whose payload the vendor
//!    stack fills from the PREVIOUS record's classification
//!    (`classifying.dll!ClassifyAndTrack`); the very first one is
//!    zero bytes;
//! 4. push the record into the host pipeline; the loop paces at
//!    USBComFPS = 35.
//!
//! The 640-B wire record carries RAW blob data (the classifier input,
//! `SVM_Class_ball/head_*.dat` training) - the marker x/y/z values of
//! the host-side xEye3Ddata are computed ON THE HOST. Our model keeps
//! the raw bytes and decodes the one byte that is understood on the
//! wire: the status byte at +0x25C (the f660 transform @0xf660).
//!
//! ## ENS startup (turn 49 - the wire format the captures never saw)
//!
//! `usbCom` brings the record plane up in one of two ways:
//!
//! * **Hot start** (`0x10640` + `0x10900`): when a type-0xF0 frame
//!   already answers, the stream is alive from a previous session.
//!   The vendor reads THREE 0xF0+record cycles, ACKing the first two
//!   with a `{PU magic, type 7}` frame, then finishes: PU 0x10 ->
//!   OD 0x10 (the 128-B lens frame, 500 ms), PU 0x9001 -> OD 0x9001
//!   (the 64-B device-ID frame), `setDeviceID`, and usbCom calls
//!   `SetLensParam` on the 0x78 B at ens+8.
//! * **Cold start** (`0xf790`, the 8-state machine, jmp table
//!   @0xf940): when the probe is silent - the T12 bench verdict - the
//!   vendor uploads `ens_code.ldr` to the DSP first. State order and
//!   every frame (all on EP 0x03/0x83):
//!
//!   | state | handler | milestone (vendor log) | exchange |
//!   |---|---|---|---|
//!   | 1 | `0xfa30` | 启动开始 startup-begin | PU 1 -> OD 1 (device ID) |
//!   | 2 | `0xfae0` | 代码加载 code-load | PU 2 {size@+8, crc@+0xC} -> OD 2 -> ceil(size/64) raw 64-B chunks -> OD 3, word 0xF @+8 |
//!   | 3 | skip | ……………… | none (returns 3) |
//!   | 4 | `0xfe20` | 手柄配对选择 pairing select | poll OD 5 (11x300 ms) -> PU 0x25/0x26 -> OD 5 |
//!   | 5 | `0xffb0` | ID及版本 ID & version | PU 0x8001 -> OD 1 (32-B ID @+8, 8-B ver @+0x28, `setDeviceID`) |
//!   | 6 | `0x10130` | 手柄配对 pairing | loop: OD 0xFFF -> PU 0xFFE; OD 4 sub 1/2/0xF/8 @+8 -> PU 4 word 0x11/3/0xFF/9; sub 8 exits |
//!   | 7 | `0x104e0` | 镜头参数 lens params | OD 6 (0x80 B) -> PU 6 |
//!   | 8 | exit | 启动流程结束 finished | returns the ens pointer (lens block at +8) |
//!
//!   The driver outer loop (`0xf790`) retries each state once and
//!   restarts the whole machine on a double failure (error mark at
//!   `0x1001cbb0` = -1; the per-state vendor return codes are the
//!   negatives cited in the error messages below). The code-load CRC
//!   is `crc.dll!GenerateCRC32` - [`crc32_ens`] below - a
//!   vendor-specific CRC-32 variant the device verifies.
//!
//! The Rust model: [`RecordIo`] is the record-plane wire seam
//! (implemented by `UsbTransport` behind `usb` and by `MockTransport`
//! for the replay tests), [`ColdStart`] / [`HotStart`] are the two
//! vendor paths, [`StartupOutcome`] is what both produce (device ID,
//! DSP version, lens params).

use std::time::Duration;

use super::{
    ENS_CODELOAD_OK_WORD, ENS_DEVICE_ID_LEN, ENS_DEVICE_VER_LEN, ENS_HANDLE_CFG_TIMEOUT,
    ENS_HANDLE_CFG_TRIES, ENS_LENS_FRAME_LEN, ENS_LENS_PARAM_LEN, ENS_LINK_FOUND,
    ENS_LINK_NOT_FOUND, ENS_LINK_OVER, ENS_LINK_REPLY_FOUND, ENS_LINK_REPLY_NOT_FOUND,
    ENS_LINK_REPLY_OVER, ENS_LINK_REPLY_TIMEOUT, ENS_LINK_TIMEOUT, ENS_PROBE_TIMEOUT,
    ENS_PROBE_TRIES, ENS_TIMEOUT_CHUNK, ENS_TIMEOUT_FRAME, ENS_TYPE_CODELOAD,
    ENS_TYPE_CODELOAD_DONE, ENS_TYPE_DEVICE_ID, ENS_TYPE_DID, ENS_TYPE_HANDLE_CFG,
    ENS_TYPE_HOT_START, ENS_TYPE_ID_REQ, ENS_TYPE_KEEPALIVE_RX, ENS_TYPE_KEEPALIVE_TX,
    ENS_TYPE_LENS, ENS_TYPE_LINK, ENS_TYPE_MATCH_OFF, ENS_TYPE_MATCH_ON, ENS_TYPE_START, MAGIC_D2H,
    MAGIC_H2D, RECORD_FRAME_LEN, RECORD_STATUS_OFF, RECORD_TRAILER_OFF, RECORD_WIRE_LEN,
    TYPE_RECORD_CMD, TYPE_RECORD_READY,
};

/// A 64-B device-to-host frame read from EP 0x83.
#[derive(Debug, Clone)]
pub struct StatusFrame {
    /// Full frame bytes (the payload semantics are unpinned - kept raw).
    pub bytes: [u8; RECORD_FRAME_LEN],
}

impl StatusFrame {
    /// Frame type (u16 @ +4).
    pub fn frame_type(&self) -> u16 {
        u16::from_le_bytes([self.bytes[4], self.bytes[5]])
    }

    /// Parse a 64-B read: it must carry the OD magic and announce a
    /// record (type [`TYPE_RECORD_READY`]).
    pub fn parse(buf: &[u8]) -> Option<StatusFrame> {
        if buf.len() < RECORD_FRAME_LEN {
            return None;
        }
        if u32::from_le_bytes(buf[0..4].try_into().ok()?) != MAGIC_D2H {
            return None;
        }
        let frame_type = u16::from_le_bytes([buf[4], buf[5]]);
        if frame_type != TYPE_RECORD_READY {
            return None;
        }
        let mut bytes = [0u8; RECORD_FRAME_LEN];
        bytes.copy_from_slice(&buf[..RECORD_FRAME_LEN]);
        Some(StatusFrame { bytes })
    }
}

/// The flags the vendor f660 transform (@RVA 0xf660) decodes from the
/// wire record's status byte at +0x25C.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RecordStatus {
    /// Raw status byte (record +0x25C).
    pub raw: u8,
    /// Status bit 0 -> the xEye3Ddata mode word (record +0x3B0);
    /// the pipe writer skips the frame when mode == 1.
    pub mode: u32,
    /// Extended flags (status bit 7 set): flag A = bit 4 == 0.
    pub flag_a: bool,
    /// Extended flags (status bit 7 set): flag B = bit 5 == 0.
    pub flag_b: bool,
    /// Extended flags (status bit 7 set): flag C = bit 3 == 0.
    pub flag_c: bool,
}

impl RecordStatus {
    /// Decode the status byte exactly like f660: bit 7 gates the three
    /// flag bits (4/5/3, inverted); bit 0 is the mode word.
    pub fn from_raw(raw: u8) -> RecordStatus {
        let ext = raw & 0x80 != 0;
        RecordStatus {
            raw,
            mode: u32::from(raw & 1),
            flag_a: ext && raw & 0x10 == 0,
            flag_b: ext && raw & 0x20 == 0,
            flag_c: ext && raw & 0x08 == 0,
        }
    }
}

/// One 640-B raw record read from EP 0x83.
///
/// The payload region +0x000..+0x25C is the raw blob data (the
/// classifier input - layout unpinned, kept for offline analysis);
/// the trailer magic and status byte are verified/decoded here.
#[derive(Debug, Clone)]
pub struct WireRecord {
    /// Full record bytes (64 B status frame header excluded - this is
    /// the 0x280-B payload read).
    pub bytes: [u8; RECORD_WIRE_LEN],
}

impl WireRecord {
    /// Parse the 0x280-B read: length exact, OD magic trailer at
    /// +0x27C (usbCom loop @0x11d13).
    pub fn parse(buf: &[u8]) -> Option<WireRecord> {
        if buf.len() != RECORD_WIRE_LEN {
            return None;
        }
        if u32::from_le_bytes(buf[RECORD_TRAILER_OFF..RECORD_TRAILER_OFF + 4].try_into().ok()?)
            != MAGIC_D2H
        {
            return None;
        }
        let mut bytes = [0u8; RECORD_WIRE_LEN];
        bytes.copy_from_slice(buf);
        Some(WireRecord { bytes })
    }

    /// The decoded status byte (f660 transform).
    pub fn status(&self) -> RecordStatus {
        RecordStatus::from_raw(self.bytes[RECORD_STATUS_OFF])
    }

    /// Non-zero byte count of the raw blob region (bench statistic:
    /// real sensor data vs. an all-zero record - the same idea as the
    /// video plane's PlaneStats contrast check).
    pub fn blob_nonzero(&self) -> usize {
        self.bytes[..RECORD_STATUS_OFF].iter().filter(|&&b| b != 0).count()
    }

    /// The raw blob region (the classifier input).
    pub fn blob(&self) -> &[u8] {
        &self.bytes[..RECORD_STATUS_OFF]
    }
}

/// A 64-B host-to-device frame of type [`TYPE_RECORD_CMD`] for EP 0x03.
#[derive(Debug, Clone)]
pub struct Type7Command {
    /// Full frame bytes.
    pub bytes: [u8; RECORD_FRAME_LEN],
}

impl Type7Command {
    /// Payload offset inside the 64-B frame (usbCom builds the frame at
    /// +0 and lets ClassifyAndTrack fill from +0xC).
    pub const PAYLOAD_OFF: usize = 0x0c;

    /// The zero ACK: the vendor loop's first write (a zeroed 64-B
    /// buffer, memset @0x119a6 before the loop starts).
    pub fn ack_zeros() -> Type7Command {
        Type7Command { bytes: [0u8; RECORD_FRAME_LEN] }
    }

    /// Build a type-7 command with a payload (the frame the vendor
    /// classifier fills: PU magic, type 7, byte 0 at +0xA, payload at
    /// +0xC - construction @0x11d91).
    pub fn with_payload(payload: &[u8]) -> Type7Command {
        let mut bytes = [0u8; RECORD_FRAME_LEN];
        bytes[..4].copy_from_slice(&MAGIC_H2D.to_le_bytes());
        bytes[4..6].copy_from_slice(&TYPE_RECORD_CMD.to_le_bytes());
        bytes[0x0a] = 0;
        let n = payload.len().min(RECORD_FRAME_LEN - Self::PAYLOAD_OFF);
        bytes[Self::PAYLOAD_OFF..Self::PAYLOAD_OFF + n].copy_from_slice(&payload[..n]);
        Type7Command { bytes }
    }

    /// Frame type (u16 @ +4).
    pub fn frame_type(&self) -> u16 {
        u16::from_le_bytes([self.bytes[4], self.bytes[5]])
    }
}

// ---------------------------------------------------------------------------
// ENS startup (turn 49). Everything below is the static decode of the
// vendor stack's record-plane bring-up - the PU/OD traffic no capture
// ever saw (the license gates kept every capture out of the game
// state), reconstructed from xEyeUSB.dll v2.6.0.6:
//   0xf790   the 8-state driver (jmp table @0xf940, per-state retry,
//            50 ms pacing, error mark at 0x1001cbb0)
//   0xfa30   state 1 - startup begin
//   0xfae0   state 2 - the ens_code.ldr code-load
//   (skip)   state 3
//   0xfe20   state 4 - handle-match (pairing) select
//   0xffb0   state 5 - ID & version
//   0x10130  state 6 - the pairing/link loop
//   0x104e0  state 7 - lens parameters
//   0x105f0  TX helper: {PU magic, u16 type @ +4} on pipe 3 (EP 0x03)
//   0x10590  RX helper: pipe 2 (EP 0x83), exact length + OD magic +
//            expected type
//   0x10640  the hot-start probe (also the ENS probe of turn 46)
//   0x10900  the hot-start finish
//   crc.dll  GenerateCRC32 - the code-load checksum the device verifies
// Full walkthrough: `research/idong100/agents/21-turn49/FINDINGS.md`.
// ---------------------------------------------------------------------------

/// The record-plane wire seam: raw bulk I/O on EP 0x83/0x03.
///
/// `UsbTransport` implements this behind the `usb` feature (its
/// `read_bulk_ep`/`write_bulk_ep`); `MockTransport` implements it for
/// the startup replay tests. A read that times out silently returns an
/// empty vec (the vendor's EasyUSB `ReadData` returns the byte count,
/// and every helper treats `count != len` as a failed try).
pub trait RecordIo {
    /// Read up to `len` bytes from EP 0x83. Empty = timeout.
    fn read_record_ep(&mut self, len: usize, timeout: Duration) -> crate::Result<Vec<u8>>;
    /// Write `buf` to EP 0x03; returns the byte count written.
    fn write_record_ep(&mut self, buf: &[u8], timeout: Duration) -> crate::Result<usize>;
}

/// What both startup paths produce: the device identity and the lens
/// parameters the vendor hands to `SetLensParam` (usbCom @0x118c2:
/// 0x78 B copied from the ens block at +8, after the state machine or
/// the hot-start finish filled it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupOutcome {
    /// The 32-B device ID (`setDeviceID` input; same string the 7F 55
    /// session reads from EEPROM).
    pub device_id: [u8; ENS_DEVICE_ID_LEN],
    /// The 8-B DSP version string (printed as "DSP_VER = %s").
    pub dsp_version: [u8; ENS_DEVICE_VER_LEN],
    /// The 0x78-B lens parameter block (OD type-6/0x10 frame at +8).
    pub lens_params: [u8; ENS_LENS_PARAM_LEN],
}

/// The vendor's CRC-32 (`crc.dll!GenerateCRC32`, v2.6.0.6).
///
/// Bit-exact port of the disassembly: the table is built by
/// `init_crc32_table` @0x1010 from `reverse8(i) << 24` through eight
/// MSB-first rounds of polynomial `0x04C11DB7` and a final 32-bit
/// reversal - numerically the standard reflected `0xEDB88320` table -
/// but the update loop @0x1110 is the vendor's hybrid form
/// `crc = ((crc << 8) | byte) ^ table[crc >> 24]` with init 0 and no
/// final XOR. It is NOT a catalog CRC ("123456789" -> 0x4D3A139E, not
/// CRC-32/MPEG-2's 0x0376E6E7 or zlib's 0xCBF43926); the device
/// verifies exactly this variant, so the port is literal.
pub fn crc32_ens(data: &[u8]) -> u32 {
    // The table as the vendor builds it. Const-proofed by the unit
    // tests (same construction, same check values); built lazily per
    // call is fine for the announce-once use, and keeps this pure.
    fn table() -> [u32; 256] {
        fn reverse8(i: u8) -> u8 {
            i.reverse_bits() // x86 `bswap`-equivalent of the vendor's shift-or chain
        }
        fn reverse32(x: u32) -> u32 {
            x.reverse_bits()
        }
        let mut tbl = [0u32; 256];
        for i in 0..256usize {
            let mut c = (reverse8(i as u8) as u32) << 24;
            for _ in 0..8 {
                let mut t = c << 1;
                if c & 0x8000_0000 != 0 {
                    t ^= 0x04C1_1DB7;
                }
                c = t;
            }
            tbl[i] = reverse32(c);
        }
        tbl
    }
    let tbl = table();
    let mut crc: u32 = 0;
    for &b in data {
        crc = ((crc << 8) | b as u32) ^ tbl[(crc >> 24) as usize];
    }
    crc
}

/// A zero-filled 64-B PU frame of the given type (the `0x105f0`
/// builder: magic dword at +0, u16 type at +4, everything else zero -
/// the vendor sends stack garbage in the unwritten bytes; zeros are
/// the deterministic, safe choice and the device only checks magic +
/// type in every disassembled path).
pub fn pu_frame(frame_type: u16) -> [u8; RECORD_FRAME_LEN] {
    let mut b = [0u8; RECORD_FRAME_LEN];
    b[..4].copy_from_slice(&MAGIC_H2D.to_le_bytes());
    b[4..6].copy_from_slice(&frame_type.to_le_bytes());
    b
}

/// A PU frame with an extra u16 word at +8 (the link-status replies of
/// the pairing loop: PU type 4 + word 0x11/3/0xFF/9 at +8).
pub fn pu_frame_word(frame_type: u16, word: u16) -> [u8; RECORD_FRAME_LEN] {
    let mut b = pu_frame(frame_type);
    b[8..10].copy_from_slice(&word.to_le_bytes());
    b
}

/// The code-load announce frame (state 2, `0xfae0` @0xfbf1: PU magic,
/// u16 type 2 @+4, u32 file size @+8, u32 [`crc32_ens`] @+0xC).
pub fn pu_codeload_announce(size: u32, crc: u32) -> [u8; RECORD_FRAME_LEN] {
    let mut b = pu_frame(ENS_TYPE_CODELOAD);
    b[8..12].copy_from_slice(&size.to_le_bytes());
    b[0x0c..0x10].copy_from_slice(&crc.to_le_bytes());
    b
}

/// The code-load chunks: the RAW file bytes in ceil(size/64) 64-B
/// slices (`0xfae0` @0xfcf6: `fread(frame, 1, 0x40, fp)` then a 64-B
/// write - no per-chunk framing). The final partial chunk is
/// zero-padded; the vendor writes stale stack bytes there, the device
/// knows the exact size from the announce and ignores the tail.
pub fn codeload_chunks(file: &[u8]) -> impl Iterator<Item = [u8; RECORD_FRAME_LEN]> + '_ {
    let n = file.len().div_ceil(RECORD_FRAME_LEN);
    (0..n).map(move |i| {
        let mut chunk = [0u8; RECORD_FRAME_LEN];
        let off = i * RECORD_FRAME_LEN;
        let end = (off + RECORD_FRAME_LEN).min(file.len());
        chunk[..end - off].copy_from_slice(&file[off..end]);
        chunk
    })
}

/// A generic OD frame read from EP 0x83 (the `0x10590` helper: exact
/// length, OD magic, one expected type). The payload beyond +6 is kept
/// raw; the accessors decode the fields the startup machine uses.
#[derive(Debug, Clone)]
pub struct OdFrame {
    pub bytes: Vec<u8>,
}

impl OdFrame {
    /// Parse a read of exactly `len` bytes: OD magic at +0 and the
    /// u16 type at +4 must match `expect`.
    pub fn parse(buf: &[u8], expect: u16) -> Option<OdFrame> {
        if buf.len() < 6 {
            return None;
        }
        if u32::from_le_bytes(buf[0..4].try_into().ok()?) != MAGIC_D2H {
            return None;
        }
        if u16::from_le_bytes(buf[4..6].try_into().ok()?) != expect {
            return None;
        }
        Some(OdFrame { bytes: buf.to_vec() })
    }

    /// The u16 word at +8 (link-status sub-code / code-load verdict).
    pub fn word8(&self) -> u16 {
        if self.bytes.len() < 10 {
            return 0;
        }
        u16::from_le_bytes([self.bytes[8], self.bytes[9]])
    }

    /// The device-ID view: 32 B at +8, 8 B version at +0x28 (`0xffb0`
    /// @0x100b5 and `0x10900` @0x10a4b copy exactly these fields).
    pub fn device_id(&self) -> Option<([u8; ENS_DEVICE_ID_LEN], [u8; ENS_DEVICE_VER_LEN])> {
        if self.bytes.len() < 0x30 {
            return None;
        }
        let mut id = [0u8; ENS_DEVICE_ID_LEN];
        id.copy_from_slice(&self.bytes[8..8 + ENS_DEVICE_ID_LEN]);
        let mut ver = [0u8; ENS_DEVICE_VER_LEN];
        ver.copy_from_slice(&self.bytes[0x28..0x28 + ENS_DEVICE_VER_LEN]);
        Some((id, ver))
    }
}

/// The cold-start state machine (`0xf790`): upload `ens_code.ldr` to
/// the DSP and bring the record plane up, states 1..=7 exactly as the
/// vendor runs them.
///
/// The per-state vendor return codes are cited in the error messages
/// (vendor -1..-30). The vendor's outer loop additionally retries each
/// state once and restarts the machine on a double failure (`0xf790`
/// @0xf8fa); the driver leaves that policy to the caller - a failed
/// bench run is simply rerun.
pub struct ColdStart<'a, IO: RecordIo> {
    io: &'a mut IO,
    ldr: &'a [u8],
    /// Progress log of the passed milestones (the vendor's own
    /// 启动 strings), for the bench output.
    pub milestones: Vec<&'static str>,
}

impl<IO: RecordIo> ColdStart<'_, IO> {
    /// New cold-start over a record-plane wire seam and the raw
    /// `ens_code.ldr` bytes (the caller loads and SHA-256-pins the
    /// file - `crate::firmware::load_set`).
    pub fn new<'a>(io: &'a mut IO, ldr: &'a [u8]) -> ColdStart<'a, IO> {
        ColdStart { io, ldr, milestones: Vec::new() }
    }

    fn step(&mut self, what: &'static str) -> crate::Result<()> {
        self.milestones.push(what);
        Ok(())
    }

    /// RX helper `0x10590`: read `len` bytes, expect the OD magic and
    /// `expect` type. Empty read = timeout. The error names what
    /// actually arrived (the vendor helper returns -1 read / -2 frame).
    fn rx(&mut self, len: usize, expect: u16, timeout: Duration) -> crate::Result<OdFrame> {
        let buf = self.io.read_record_ep(len, timeout)?;
        if buf.len() != len {
            return Err(crate::IdongError::Protocol(crate::ProtocolError::Short {
                need: len,
                have: buf.len(),
            }));
        }
        OdFrame::parse(&buf, expect).ok_or_else(|| {
            let got = if buf.len() >= 6 { u16::from_le_bytes([buf[4], buf[5]]) } else { 0 };
            crate::IdongError::Protocol(crate::ProtocolError::BadType {
                kind: got,
                expected: expect,
            })
        })
    }

    /// TX helper `0x105f0`: write a 64-B frame to EP 0x03.
    fn tx(&mut self, frame: &[u8; RECORD_FRAME_LEN], timeout: Duration) -> crate::Result<()> {
        let n = self.io.write_record_ep(frame, timeout)?;
        if n == frame.len() {
            Ok(())
        } else {
            Err(crate::IdongError::Usb(format!("short record write: {n} of {}", frame.len())))
        }
    }

    /// States 1..=7 in vendor order. `handle_match` mirrors the
    /// `NeedHandleMatch` config (shipped value 1; `false` sends type
    /// 0x26 in state 4 and skips state 6).
    pub fn run(&mut self, handle_match: bool) -> crate::Result<StartupOutcome> {
        // ---- state 1: startup begin (0xfa30) ----
        // "Tx DSP Start" (PU type 1) -> "Rx DEVICE ID" (OD type 1).
        self.tx(&pu_frame(ENS_TYPE_START), ENS_TIMEOUT_FRAME)
            .map_err(|_| vendor_ens("startup-begin", -1, "Tx DSP Start Err"))?;
        self.rx(RECORD_FRAME_LEN, ENS_TYPE_DEVICE_ID, ENS_TIMEOUT_FRAME)
            .map_err(|_| vendor_ens("startup-begin", -2, "Rx DEVICE ID Msg Err / illegal"))?;
        self.step("startup-begin")?;

        // ---- state 2: code-load (0xfae0) ----
        let crc = crc32_ens(self.ldr);
        let announce = pu_codeload_announce(self.ldr.len() as u32, crc);
        self.tx(&announce, ENS_TIMEOUT_FRAME)
            .map_err(|_| vendor_ens("code-load", -5, "Tx packet count & crc!"))?;
        self.rx(RECORD_FRAME_LEN, ENS_TYPE_CODELOAD, ENS_TIMEOUT_FRAME).map_err(|_| {
            vendor_ens("code-load", -6, "Failed to receive: Ready to load program!")
        })?;
        for (i, chunk) in codeload_chunks(self.ldr).enumerate() {
            self.tx(&chunk, ENS_TIMEOUT_CHUNK).map_err(|_| {
                vendor_ens("code-load", -8, "Error transfer: send program code error")
            })?;
            let _ = i;
        }
        let verdict = self
            .rx(RECORD_FRAME_LEN, ENS_TYPE_CODELOAD_DONE, ENS_TIMEOUT_CHUNK)
            .map_err(|_| vendor_ens("code-load", -9, "Failed to receive: CRC result!"))?;
        if verdict.word8() != ENS_CODELOAD_OK_WORD {
            return Err(vendor_ens("code-load", -11, "Error: crc result is not correct!"));
        }
        self.step("code-load")?;

        // ---- state 3: skipped by the vendor (returns 3) ----
        self.step("skip")?;

        // ---- state 4: handle-match select (0xfe20) ----
        // The vendor polls with a 300-ms read up to 11 times (100 ms
        // apart); a READ TIMEOUT retries, a frame that arrives with the
        // wrong magic/type is the immediate -13 "RCV illegal" error.
        let mut cfg = None;
        for _ in 0..ENS_HANDLE_CFG_TRIES {
            let buf = self.io.read_record_ep(RECORD_FRAME_LEN, ENS_HANDLE_CFG_TIMEOUT)?;
            if !buf.is_empty() {
                cfg = OdFrame::parse(&buf, ENS_TYPE_HANDLE_CFG);
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ =
            cfg.ok_or_else(|| vendor_ens("pairing-select", -13, "Rx READ Handle Cfg RCV illegal"))?;
        let match_type = if handle_match { ENS_TYPE_MATCH_ON } else { ENS_TYPE_MATCH_OFF };
        self.tx(&pu_frame(match_type), ENS_TIMEOUT_FRAME)
            .map_err(|_| vendor_ens("pairing-select", -14, "Tx Read Handle cfg ACK Err"))?;
        self.rx(RECORD_FRAME_LEN, ENS_TYPE_HANDLE_CFG, ENS_TIMEOUT_FRAME)
            .map_err(|_| vendor_ens("pairing-select", -29, "Rx Handle Cfg Ack Msg Err"))?;
        self.step("pairing-select")?;

        // ---- state 5: ID & version (0xffb0) ----
        self.tx(&pu_frame(ENS_TYPE_ID_REQ), ENS_TIMEOUT_FRAME)
            .map_err(|_| vendor_ens("id-version", -15, "Tx ID & Ver Resend Err"))?;
        let id_frame = self
            .rx(RECORD_FRAME_LEN, ENS_TYPE_DEVICE_ID, ENS_TIMEOUT_FRAME)
            .map_err(|_| vendor_ens("id-version", -16, "Rx ReSend Dev ID Err"))?;
        let (device_id, dsp_version) = id_frame
            .device_id()
            .ok_or_else(|| vendor_ens("id-version", -17, "Rx ReSend Dev ID illegal"))?;
        self.step("id-version")?;

        // ---- state 6: the pairing/link loop (0x10130) ----
        if handle_match {
            // Driver-side deviation: the vendor loop has no exit except
            // the found-over sub-code or an error; we bound it (64 frames)
            // so a chatty misbehaving device cannot hang the host.
            let mut guard = 0usize;
            while guard < 64 {
                guard += 1;
                let buf =
                    self.io.read_record_ep(RECORD_FRAME_LEN, ENS_TIMEOUT_FRAME).map_err(|_| {
                        vendor_ens("pairing", -18, "Failed to receive: Handle was found or not!")
                    })?;
                let f = OdFrame::parse(&buf, ENS_TYPE_KEEPALIVE_RX)
                    .or_else(|| OdFrame::parse(&buf, ENS_TYPE_LINK));
                let Some(f) = f else {
                    return Err(vendor_ens(
                        "pairing",
                        -20,
                        "No TSOL_DEVICE_MATCH_HANDLE msg header or type!",
                    ));
                };
                let ft = u16::from_le_bytes([f.bytes[4], f.bytes[5]]);
                if ft == ENS_TYPE_KEEPALIVE_RX {
                    // "IDLE msg" answer
                    self.tx(&pu_frame(ENS_TYPE_KEEPALIVE_TX), ENS_TIMEOUT_FRAME).map_err(|_| {
                        vendor_ens("pairing", -19, "Failed to Transfer : IDLE msg!")
                    })?;
                    continue;
                }
                match f.word8() {
                    ENS_LINK_FOUND => {
                        self.tx(
                            &pu_frame_word(ENS_TYPE_LINK, ENS_LINK_REPLY_FOUND),
                            ENS_TIMEOUT_FRAME,
                        )
                        .map_err(|_| vendor_ens("pairing", -21, "TSOL_DEVICE_HANDLE_FOUND_ACK"))?;
                    }
                    ENS_LINK_NOT_FOUND => {
                        self.tx(
                            &pu_frame_word(ENS_TYPE_LINK, ENS_LINK_REPLY_NOT_FOUND),
                            ENS_TIMEOUT_FRAME,
                        )
                        .map_err(|_| {
                            vendor_ens("pairing", -22, "TSOL_DEVICE_HANDLE_NOT_FOUND_ACK")
                        })?;
                    }
                    ENS_LINK_TIMEOUT => {
                        self.tx(
                            &pu_frame_word(ENS_TYPE_LINK, ENS_LINK_REPLY_TIMEOUT),
                            ENS_TIMEOUT_FRAME,
                        )
                        .map_err(|_| {
                            vendor_ens("pairing", -23, "TSOL_DEVICE_HANDLE_FOUND_TIMEOUT_ACK")
                        })?;
                    }
                    ENS_LINK_OVER => {
                        self.tx(
                            &pu_frame_word(ENS_TYPE_LINK, ENS_LINK_REPLY_OVER),
                            ENS_TIMEOUT_FRAME,
                        )
                        .map_err(|_| {
                            vendor_ens("pairing", -24, "TSOL_DEVICE_HANDLE_FOUND_OVER_ACK")
                        })?;
                        self.step("pairing")?;
                        break;
                    }
                    other => {
                        return Err(vendor_ens(
                            "pairing",
                            -25,
                            &format!("ERROR: Handle status (word 0x{other:04X})"),
                        ));
                    }
                }
            }
        } else {
            self.step("pairing-skipped")?;
        }

        // ---- state 7: lens parameters (0x104e0) ----
        let lens_frame = self
            .rx(ENS_LENS_FRAME_LEN, ENS_TYPE_LENS, ENS_TIMEOUT_FRAME)
            .map_err(|_| vendor_ens("lens-params", -26, "Rx Lens Parameter Err"))?;
        let mut lens_params = [0u8; ENS_LENS_PARAM_LEN];
        lens_params.copy_from_slice(&lens_frame.bytes[8..8 + ENS_LENS_PARAM_LEN]);
        self.tx(&pu_frame(ENS_TYPE_LENS), ENS_TIMEOUT_FRAME)
            .map_err(|_| vendor_ens("lens-params", -28, "Tx Lens Parameter ack Err"))?;
        self.step("lens-params")?;

        Ok(StartupOutcome { device_id, dsp_version, lens_params })
    }
}

/// The hot-start path (`0x10640` + `0x10900`): the record stream is
/// already alive. The vendor reads three 0xF0+record cycles (ACKing
/// the first two with `{PU magic, type 7}`), then finishes with the
/// lens + device-ID exchange.
pub struct HotStart<'a, IO: RecordIo> {
    io: &'a mut IO,
    /// Progress log of the passed milestones.
    pub milestones: Vec<&'static str>,
}

impl<IO: RecordIo> HotStart<'_, IO> {
    pub fn new(io: &mut IO) -> HotStart<'_, IO> {
        HotStart { io, milestones: Vec::new() }
    }

    /// The probe half (`0x10640`): up to [`ENS_PROBE_TRIES`] silent
    /// 100-ms reads. `None` = silence (the caller runs [`ColdStart`]).
    pub fn probe(io: &mut IO) -> Option<StatusFrame> {
        for _ in 0..ENS_PROBE_TRIES {
            if let Ok(buf) = io.read_record_ep(RECORD_FRAME_LEN, ENS_PROBE_TIMEOUT) {
                if let Some(f) = StatusFrame::parse(&buf) {
                    return Some(f);
                }
            }
        }
        None
    }

    /// The full hot start: probe + three record cycles + the finish.
    /// Returns `Ok(None)` when the probe finds no live stream (cold
    /// start needed).
    pub fn run(&mut self) -> crate::Result<Option<StartupOutcome>> {
        let Some(first) = Self::probe(self.io) else {
            return Ok(None);
        };
        self.milestones.push("probe-alive");
        // The probe's 0xF0 frame is the FIRST cycle's announcement
        // (0x10640 keeps the counter logic: three 0xF0+record cycles,
        // the first two ACKed with {PU magic, type 7}, then finish).
        let mut status = first;
        let mut lens_params = [0u8; ENS_LENS_PARAM_LEN];
        for cycle in 0..3 {
            let _ = status;
            let wire = self
                .io
                .read_record_ep(RECORD_WIRE_LEN, Duration::from_secs(1))
                .map_err(|_| vendor_ens("hot-start", 0, "Hot Start packet err"))?;
            let record = WireRecord::parse(&wire)
                .ok_or_else(|| vendor_ens("hot-start", 0, "Hot Start chk err"))?;
            let _ = record;
            if cycle < 2 {
                let ack = pu_frame(TYPE_RECORD_CMD);
                self.io
                    .write_record_ep(&ack, Duration::from_millis(300))
                    .map_err(|_| vendor_ens("hot-start", 0, "Hot Start ack err"))?;
                // the next cycle's announcement
                let next = self
                    .io
                    .read_record_ep(RECORD_FRAME_LEN, ENS_TIMEOUT_FRAME)
                    .map_err(|_| vendor_ens("hot-start", 0, "Hot Start no rcv / chk err"))?;
                status = StatusFrame::parse(&next)
                    .ok_or_else(|| vendor_ens("hot-start", 0, "Hot Start no rcv / chk err"))?;
            }
        }
        // finish (0x10900): PU 0x10 -> OD 0x10 (lens, 0x80 B) ->
        // PU 0x9001 -> OD 0x9001 (device ID)
        self.io
            .write_record_ep(&pu_frame(ENS_TYPE_HOT_START), ENS_TIMEOUT_FRAME)
            .map_err(|_| vendor_ens("hot-start", -1, "Tx HotStart Cmd"))?;
        let lens_frame = self
            .io
            .read_record_ep(ENS_LENS_FRAME_LEN, Duration::from_millis(500))
            .and_then(|b| {
                OdFrame::parse(&b, ENS_TYPE_HOT_START).ok_or_else(|| {
                    crate::IdongError::Protocol(crate::ProtocolError::BadType {
                        kind: 0,
                        expected: ENS_TYPE_HOT_START,
                    })
                })
            })
            .map_err(|_| vendor_ens("hot-start", -2, "Rx HotStart Lens Ack Err"))?;
        lens_params.copy_from_slice(&lens_frame.bytes[8..8 + ENS_LENS_PARAM_LEN]);
        self.io
            .write_record_ep(&pu_frame(ENS_TYPE_DID), ENS_TIMEOUT_FRAME)
            .map_err(|_| vendor_ens("hot-start", -4, "Tx HotStart DID Err"))?;
        let did = self
            .io
            .read_record_ep(RECORD_FRAME_LEN, ENS_TIMEOUT_FRAME)
            .and_then(|b| {
                OdFrame::parse(&b, ENS_TYPE_DID).ok_or_else(|| {
                    crate::IdongError::Protocol(crate::ProtocolError::BadType {
                        kind: 0,
                        expected: ENS_TYPE_DID,
                    })
                })
            })
            .map_err(|_| vendor_ens("hot-start", -5, "Rx HotStart DID Err"))?;
        let (device_id, dsp_version) = did
            .device_id()
            .ok_or_else(|| vendor_ens("hot-start", -6, "Rx HotStart DID illegal"))?;
        self.milestones.push("hot-start-ok");
        Ok(Some(StartupOutcome { device_id, dsp_version, lens_params }))
    }
}

/// Wrap a startup failure with the vendor's own state name and return
/// code (the negative numbers the state handlers return to `0xf790`).
fn vendor_ens(state: &str, code: i32, log: &str) -> crate::IdongError {
    crate::IdongError::Usb(format!("ens {state} failed (vendor {code}: {log})"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status_frame_bytes(frame_type: u16) -> [u8; RECORD_FRAME_LEN] {
        let mut b = [0u8; RECORD_FRAME_LEN];
        b[..4].copy_from_slice(&MAGIC_D2H.to_le_bytes());
        b[4..6].copy_from_slice(&frame_type.to_le_bytes());
        b
    }

    #[test]
    fn status_frame_accepts_only_od_type_f0() {
        let f = StatusFrame::parse(&status_frame_bytes(TYPE_RECORD_READY)).unwrap();
        assert_eq!(f.frame_type(), TYPE_RECORD_READY);

        // wrong type: the 7F 55 session lives on EP 0x82, but a link
        // status frame (type 4) read from 0x83 must NOT pass as a
        // record announcement
        assert!(StatusFrame::parse(&status_frame_bytes(4)).is_none());
        // PU magic (an echo of our own command) must not pass either
        let mut pu = status_frame_bytes(TYPE_RECORD_READY);
        pu[..4].copy_from_slice(&MAGIC_H2D.to_le_bytes());
        assert!(StatusFrame::parse(&pu).is_none());
        // short reads reject
        assert!(StatusFrame::parse(&status_frame_bytes(0xf0)[..63]).is_none());
    }

    #[test]
    fn wire_record_needs_exact_len_and_trailer() {
        let mut rec = [0u8; RECORD_WIRE_LEN];
        // no trailer -> reject
        assert!(WireRecord::parse(&rec).is_none());
        rec[RECORD_TRAILER_OFF..RECORD_TRAILER_OFF + 4].copy_from_slice(&MAGIC_D2H.to_le_bytes());
        let r = WireRecord::parse(&rec).unwrap();
        assert_eq!(r.blob_nonzero(), 0);
        assert_eq!(r.status().mode, 0);
        // 639 or 641 bytes: reject (the usbCom loop demands exactly 0x280)
        assert!(WireRecord::parse(&rec[..RECORD_WIRE_LEN - 1]).is_none());
        assert!(WireRecord::parse(&[&rec[..], rec[..1].as_ref()].concat()).is_none());
    }

    #[test]
    fn status_byte_decodes_like_the_f660_transform() {
        // bit 0 -> mode 1 (pipe-writer skip flag)
        let s = RecordStatus::from_raw(0x01);
        assert_eq!(s.mode, 1);
        assert!(!s.flag_a && !s.flag_b && !s.flag_c, "bit 7 clear -> flags stay 0");
        // bit 7 set, bits 4/5/3 clear -> all three flags true (f660: the
        // flag is (status & bit) == 0, gated on 0x80)
        let s = RecordStatus::from_raw(0x80);
        assert_eq!(s.mode, 0);
        assert!(s.flag_a && s.flag_b && s.flag_c);
        // bit 7 set, bits 4/5/3 set -> all three flags false
        let s = RecordStatus::from_raw(0xB8);
        assert!(!s.flag_a && !s.flag_b && !s.flag_c);
        // mixed: bit 5 set only -> flag B false, others true
        let s = RecordStatus::from_raw(0xA0);
        assert!(s.flag_a && s.flag_c);
        assert!(!s.flag_b);
    }

    #[test]
    fn wire_record_status_and_blob_reported() {
        let mut rec = [0u8; RECORD_WIRE_LEN];
        rec[RECORD_TRAILER_OFF..RECORD_TRAILER_OFF + 4].copy_from_slice(&MAGIC_D2H.to_le_bytes());
        rec[0] = 0xAA;
        rec[1] = 0xBB;
        rec[RECORD_STATUS_OFF] = 0x81;
        let r = WireRecord::parse(&rec).unwrap();
        assert_eq!(r.blob_nonzero(), 2);
        assert_eq!(r.blob().len(), RECORD_STATUS_OFF);
        assert_eq!(r.status().raw, 0x81);
        assert_eq!(r.status().mode, 1);
        assert!(r.status().flag_a, "0x81: bit7 set, bit4 clear -> flag_a");
    }

    #[test]
    fn type7_command_layout_matches_the_vendor_builder() {
        // zero ACK: 64 zero bytes (the vendor's first write)
        let ack = Type7Command::ack_zeros();
        assert_eq!(ack.bytes, [0u8; RECORD_FRAME_LEN]);

        // payload command: PU magic + type 7 + payload @ +0xC
        let cmd = Type7Command::with_payload(&[0x11, 0x22, 0x33]);
        assert_eq!(u32::from_le_bytes(cmd.bytes[0..4].try_into().unwrap()), MAGIC_H2D);
        assert_eq!(cmd.frame_type(), TYPE_RECORD_CMD);
        assert_eq!(cmd.frame_type(), 7, "the vendor builder writes word 7 @ +4");
        assert_eq!(cmd.bytes[0x0a], 0, "byte 0 at +0xA (usbCom @0x11da3)");
        assert_eq!(&cmd.bytes[0x0c..0x0f], &[0x11, 0x22, 0x33]);
        // payload longer than the frame is clipped, never overflows
        let long = Type7Command::with_payload(&[0xFFu8; 256]);
        assert_eq!(long.bytes[RECORD_FRAME_LEN - 1], 0xFF);
    }

    #[test]
    fn constants_cite_the_disassembly() {
        // the sizes the usbCom loop and the ens states actually use
        assert_eq!(RECORD_WIRE_LEN, 0x280, "read len @0x11c80");
        assert_eq!(RECORD_TRAILER_OFF, 0x27c, "trailer check @0x11d13");
        assert_eq!(RECORD_STATUS_OFF, 0x25c, "f660 reads byte @ +0x25C");
        assert_eq!(TYPE_RECORD_READY, 0xF0, "type check @0x11c59");
        assert_eq!(TYPE_RECORD_CMD, 7, "word 7 @0x11d97");
        assert_eq!(RECORD_FRAME_LEN, 64, "64-B frames everywhere in usbCom/ens");
    }

    // ---- turn 49: the startup machine ----

    fn od_frame(frame_type: u16) -> [u8; RECORD_FRAME_LEN] {
        let mut b = [0u8; RECORD_FRAME_LEN];
        b[..4].copy_from_slice(&MAGIC_D2H.to_le_bytes());
        b[4..6].copy_from_slice(&frame_type.to_le_bytes());
        b
    }

    #[test]
    fn crc32_ens_matches_the_vendor_dll() {
        // vectors computed by the literal crc.dll model
        // (research/idong100/agents/21-turn49/, crc_model.py)
        assert_eq!(crc32_ens(b""), 0);
        assert_eq!(crc32_ens(b"\x00"), 0);
        assert_eq!(crc32_ens(b"a"), 0x61, "short inputs degenerate - the table index stays 0");
        assert_eq!(
            crc32_ens(b"123456789"),
            0x4d3a139e,
            "NOT MPEG-2 (0376e6e7), NOT zlib (cbf43926)"
        );
        assert_eq!(crc32_ens(b"hello world"), 0xa7491886);
        // the shipped ens_code.ldr (54,420 B, sha256-pinned by
        // FirmwareSet): the announce CRC the device expects
        if let Ok(set) = crate::firmware::load_set() {
            assert_eq!(set.ldr.len(), 54420, "the vendor loader image size");
            assert_eq!(crc32_ens(&set.ldr), 0x63eaa8ca, "the disassembly-pinned ldr CRC");
        }
    }

    #[test]
    fn pu_frame_builders_match_the_105f0_layout() {
        let f = pu_frame(ENS_TYPE_START);
        assert_eq!(&f[..4], &[0x50, 0x55, 0x7f, 0x7f], "PU magic LE");
        assert_eq!(&f[4..6], &[0x01, 0x00], "u16 type @ +4");
        assert!(f[6..].iter().all(|&b| b == 0), "deterministic zeros (vendor sends stack garbage)");

        let link = pu_frame_word(ENS_TYPE_LINK, ENS_LINK_REPLY_FOUND);
        assert_eq!(&link[8..10], &[0x11, 0x00], "reply word @ +8 (@0x10291)");
        assert_eq!(&link[4..6], &[0x04, 0x00], "type 4");

        let ann = pu_codeload_announce(0xD494, 0x63eaa8ca);
        assert_eq!(&ann[4..6], &[0x02, 0x00], "type 2 (@0xfbfd)");
        assert_eq!(&ann[8..12], &[0x94, 0xd4, 0x00, 0x00], "file size u32 @ +8 (@0xfc0f)");
        assert_eq!(&ann[0x0c..0x10], &[0xca, 0xa8, 0xea, 0x63], "CRC u32 @ +0xC (@0xfc1b)");
    }

    #[test]
    fn codeload_chunks_are_raw_64b_slices() {
        // 130 B -> 3 chunks: 64 + 64 + 2 (zero-padded tail)
        let file: Vec<u8> = (0..130u8).collect();
        let chunks: Vec<[u8; RECORD_FRAME_LEN]> = codeload_chunks(&file).collect();
        assert_eq!(chunks.len(), 3, "ceil(130/64) - the vendor's nchunks (@0xfb60)");
        assert_eq!(&chunks[0][..], &file[..64]);
        assert_eq!(&chunks[1][..], &file[64..128]);
        assert_eq!(&chunks[2][..2], &file[128..]);
        assert!(chunks[2][2..].iter().all(|&b| b == 0), "tail zero-padded, not stale stack bytes");

        // empty file: no chunks (the vendor would still announce size 0)
        assert_eq!(codeload_chunks(&[]).count(), 0);
    }

    #[test]
    fn od_frame_parse_and_fields() {
        let mut f = od_frame(ENS_TYPE_DEVICE_ID);
        // the device-ID layout: 32 B ID @ +8, 8 B version @ +0x28
        for (i, b) in f.iter_mut().enumerate().skip(8).take(32) {
            *b = (i - 8) as u8;
        }
        f[0x28..0x30].copy_from_slice(b"V2.6.0.6");
        let od = OdFrame::parse(&f, ENS_TYPE_DEVICE_ID).unwrap();
        let (id, ver) = od.device_id().unwrap();
        assert_eq!(id[0], 0);
        assert_eq!(id[31], 31);
        assert_eq!(&ver, b"V2.6.0.6");

        // wrong expected type / wrong magic / short buffer reject
        assert!(OdFrame::parse(&f, ENS_TYPE_CODELOAD).is_none());
        let mut pu = f;
        pu[..4].copy_from_slice(&MAGIC_H2D.to_le_bytes());
        assert!(OdFrame::parse(&pu, ENS_TYPE_DEVICE_ID).is_none());
        assert!(OdFrame::parse(&f[..4], ENS_TYPE_DEVICE_ID).is_none());

        // the word @ +8 (code-load verdict / link sub-code)
        let mut v = od_frame(ENS_TYPE_CODELOAD_DONE);
        v[8..10].copy_from_slice(&ENS_CODELOAD_OK_WORD.to_le_bytes());
        assert_eq!(OdFrame::parse(&v, ENS_TYPE_CODELOAD_DONE).unwrap().word8(), 0x000F);
    }
}
