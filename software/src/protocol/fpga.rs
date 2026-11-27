//! The FPGA bitstream download (turn 50) - the missing bring-up step
//! between the 7F 55 session init and the record plane.
//!
//! Source: xeyezHal.dll v2.6.0.6, disassembled this turn (scripts
//! `research/scripts/dis50_hal.py`, artifacts
//! `research/idong100/agents/22-turn50/`). The chain:
//!
//! ```text
//! xEyeUSB.dll McuManager25::connect (VA 0x10006500, turn 33)
//!   -> GetPrivateProfileStringA("Config", "BitFile", "", ... ".\\xEyeUsb.ini")
//!   -> xeyezHal_SetFpgaFile(name)            [dispatch impl 0xbd10]
//!   -> xeyezHal_StartFlow()                  [dispatch @0x16300 -> 0x13b70]
//!        step: Init StartFlow (0x103c0, the EasyUSB pipe open)
//!        step: SetCmosIndex / SetCmosTable / SetFpgaTable (dirty-flag gated)
//!        step 2: the 8051 flash update (0x13060 - observed on the wire)
//!        step 3: xeyezHal_DownLoadFpga  - UNCONDITIONAL, every StartFlow
//!        steps 4-8: ChipIDVerify / lens / CMOS_CFG / FPGA_CFG / LED tables
//!   -> session init -> GetMcuVersion -> the license gates
//! ```
//!
//! `DownLoadFpga` (impl @0x10040) is a five-step named flow on the
//! 7F 55 SESSION plane (EasyUSB write pipe 0 = EP 0x02, read pipe 1 =
//! EP 0x82 - the same pair the whole session uses):
//!
//! | step | frame (8 B) | u16 cmd | reply check |
//! |---|---|---|---|
//! | FPGA_DOWNLOAD_START | `7f 55 02 00 00 00 00 00` | 0x0002 | byte@+4 == 1 |
//! | FPGA_DOWNLOAD_QUERY_CFG | `7f 55 02 01 00 00 00 00` | 0x0102 | byte@+4 == 1 |
//! | SendFpgaCode announce | `7f 55 02 02 00 00 00 04` + size BE u32 (12 B) | 0x0202 | byte@+4 == 1 |
//! | the file, RAW | bit-reversed bytes in <=64 KiB bulk writes | - | no replies |
//! | FPGA_DOWNLOAD_END | `7f 55 02 03 00 00 00 00` | 0x0302 | byte@+4 == 1 |
//! | FPGA_DOWNLOAD_DONE | `7f 55 02 01 00 00 00 01` | 0x0102 word 1 | byte@+4 == 1 |
//!
//! The frame builder (`0xb2f0`): `7f 55` magic u16 LE, u8 cmd-lo @+2,
//! u8 cmd-hi @+3, u8 flags @+4, u8 @+5, u16 `swap_bytes(flag)` @+6.
//! `SendFpgaCode` (@0xd570) reads the WHOLE file, bit-reverses every
//! byte in place (`(b & 1) << 7 | ... ` = [`u8::reverse_bits`] - the
//! slave-serial load order; skipped only on platform flag == 1, and
//! this unit is not platform 1: the flash-update step 2 it gates ran in
//! every capture), announces the size big-endian, then streams the
//! file in <=0x10000-byte raw bulk writes (3 s each, no per-chunk
//! reply), logging progress every 100 chunks.
//!
//! Why this matters (the T13 startup-begin failure): the DSP lives
//! behind this Spartan-3A XC3S400A FPGA (`work_top.ncd;HW_TIMEOUT=FALSE`,
//! `3s400aft256`, 2010/04/06 - the .bit header). The FPGA configuration
//! is VOLATILE - the vendor re-downloads it on every device open - and
//! until it is loaded the record plane (EP 0x83/0x03) is dead: the ENS
//! probe stays silent and the cold machine's state 1 times out waiting
//! for the DEVICE ID frame (vendor -2, the T13 bench result). Our
//! driver never ran this download - the turn-32 "no FPGA upload on the
//! wire" capture conclusion was a false negative: every capture started
//! AFTER `McuManager25::connect` had already run `StartFlow`.

use std::time::Duration;

use crate::error::{IdongError, Result};
use crate::transport::Transport;

/// The magic the HAL packet builder writes (`0xb2f0`: `mov word [buf], 0x557f`).
pub const FPGA_MAGIC: u16 = 0x557f;

/// The reply frame the HAL accepts (`flow_open` @0xb3e6-0x45b: read `h`
/// bytes, check `u16@+0 == 0x557f` and `u8@+4 == f` with `f == 1`).
pub const FPGA_REPLY_ACK: u8 = 0x01;
pub const FPGA_REPLY_LEN: usize = 8;

/// `SendFpgaCode` chunk cap (`@0xd778: min(remaining, 0x10000)`).
pub const FPGA_CHUNK: usize = 0x1_0000;

/// Vendor timeouts: the announce write 1000 ms (`flow_open` @0xb3c4,
/// `push 0x3e8`), the raw chunk writes 3000 ms (`SendFpgaCode` @0xd7b9,
/// `push 0xbb8`), every reply read 3000 ms (`flow_open` @0xb3fd).
pub const FPGA_TIMEOUT_ANNOUNCE: Duration = Duration::from_millis(1000);
pub const FPGA_TIMEOUT_CHUNK: Duration = Duration::from_millis(3000);
pub const FPGA_TIMEOUT_REPLY: Duration = Duration::from_millis(3000);

/// Vendor negative return codes, cited verbatim in the errors:
/// `flow_open` write fail 0xffffd8f0, short reply 0xffffd8f1, frame
/// mismatch 0xffffd8f2; `SendFpgaCode` fopen fail 0xffffd8f3, chunk
/// write fail 0xffffd8f4.
pub const FPGA_CODE_TX: i32 = -10000;
pub const FPGA_CODE_REPLY_SHORT: i32 = -10015;
pub const FPGA_CODE_REPLY_FRAME: i32 = -10014;
pub const FPGA_CODE_FOPEN: i32 = -10013;
pub const FPGA_CODE_CHUNK_TX: i32 = -10012;

/// The 8-byte 7F 55 flow-step frame (`0xb2f0`, cited in the module doc).
/// `cmd` is the u16 the existing runtime layer reads at +2 (LE); the
/// trailing flag word is byte-swapped into +6/+7 exactly like the HAL.
pub fn fpga_step_frame(cmd: u16, flag_word: u16) -> [u8; 8] {
    let mut f = [0u8; 8];
    f[..2].copy_from_slice(&FPGA_MAGIC.to_le_bytes());
    f[2] = cmd as u8;
    f[3] = (cmd >> 8) as u8;
    f[6..8].copy_from_slice(&flag_word.swap_bytes().to_le_bytes());
    f
}

/// The 12-byte SendFpgaCode announce (`0xd732-0xd748`: the frame with
/// `e = 4` - the vendor's "a 4-byte field follows" marker - then the
/// file size as a BIG-endian u32 at +8).
pub fn fpga_send_announce(size: usize) -> [u8; 12] {
    let mut f = [0u8; 12];
    f[..8].copy_from_slice(&fpga_step_frame(0x0202, 4));
    f[8..12].copy_from_slice(&(size as u32).to_be_bytes());
    f
}

/// Bit-reverse every byte (`SendFpgaCode` @0xd605-0xd6dd, the literal
/// `(b & 1) << 7 | (b & 2) << 5 | (b & 4) << 3 | (b & 8) << 1 |
/// (b & 0x10) >> 1 | (b & 0x20) >> 3 | (b & 0x40) >> 5 | (b & 0x80) >> 7`
/// chain - [`u8::reverse_bits`]). The vendor reverses the ENTIRE file,
/// .bit header included, in place; the device-side loader skips the
/// header fields it does not need.
pub fn fpga_bit_reverse(data: &[u8]) -> Vec<u8> {
    data.iter().map(|b| b.reverse_bits()).collect()
}

/// One 8-byte reply (`flow_open` @0xb3e6-0x45b): exactly
/// [`FPGA_REPLY_LEN`] bytes, [`FPGA_MAGIC`] at +0, [`FPGA_REPLY_ACK`]
/// at +4. Empty read = timeout (the transport contract) and any
/// transport error both land in the vendor's short-reply branch (the
/// EasyUSB `ReadData` count != len path, -10015); a length/magic/ack
/// mismatch is the -10014 frame branch. The bytes beyond the check
/// fields are returned raw (QUERY_CFG likely carries the FPGA idcode
/// there).
fn read_reply(t: &mut dyn Transport) -> Result<[u8; FPGA_REPLY_LEN], (i32, String)> {
    let buf = match t.read(64) {
        Ok(b) => b,
        Err(e) => {
            return Err((
                FPGA_CODE_REPLY_SHORT,
                format!("read error: {e} (the EasyUSB count != len path)"),
            ))
        }
    };
    if buf.len() != FPGA_REPLY_LEN {
        return Err((
            FPGA_CODE_REPLY_SHORT,
            format!("reply {} B (need 8; empty = the device stayed silent)", buf.len()),
        ));
    }
    if u16::from_le_bytes([buf[0], buf[1]]) != FPGA_MAGIC || buf[4] != FPGA_REPLY_ACK {
        return Err((
            FPGA_CODE_REPLY_FRAME,
            format!("reply {} - magic/ack mismatch", hex_head(&buf)),
        ));
    }
    let mut out = [0u8; FPGA_REPLY_LEN];
    out.copy_from_slice(&buf);
    Ok(out)
}

fn hex_head(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect::<Vec<_>>().join(" ")
}

fn vendor_fpga(code: i32, detail: &str) -> IdongError {
    IdongError::Usb(format!("fpga download failed (vendor {code}: {detail})"))
}

/// Wrap a step failure with its vendor code WITHOUT losing the cause
/// (the T13 lesson: `vendor -2` alone hid whether the wire was silent
/// or answering garbage).
fn step_err(step: &'static str, code: i32, detail: &str) -> IdongError {
    IdongError::Usb(format!("fpga download failed (vendor {code}: {step} {detail})"))
}

/// What one download produced (for the bench output and the tests).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FpgaDownloadReport {
    /// The FPGA_DOWNLOAD_QUERY_CFG reply, raw (the check fields are
    /// magic@+0 and ack@+4; bytes 5..8 unpinned - kept for comparison).
    pub query_reply: [u8; FPGA_REPLY_LEN],
    /// The announced size (== the file length).
    pub size: usize,
    /// How many raw bulk writes the file took.
    pub chunks: usize,
}

/// The FPGA bitstream download over any [`Transport`] (EP 0x02 write /
/// EP 0x82 read - the session plane). `bit` is the RAW `.bit` file
/// (the caller passes the SHA-256-pinned `FirmwareSet::bit`); the
/// bit-reversal happens here, exactly like the vendor.
pub struct FpgaDownload<'a> {
    transport: &'a mut dyn Transport,
    /// Progress log (the vendor step names), for the bench output.
    pub milestones: Vec<&'static str>,
}

impl<'a> FpgaDownload<'a> {
    pub fn new(transport: &'a mut dyn Transport) -> Self {
        FpgaDownload { transport, milestones: Vec::new() }
    }

    /// Run the five-step flow. Fail-fast, one attempt per step (the
    /// vendor's own `flow_open` has no retry; its caller `connect`
    /// retries the whole open instead).
    pub fn run(&mut self, bit: &[u8]) -> Result<FpgaDownloadReport> {
        if bit.is_empty() {
            // SendFpgaCode fopen fail analog: nothing to send.
            return Err(vendor_fpga(FPGA_CODE_FOPEN, "no bitstream bytes"));
        }

        // 1. FPGA_DOWNLOAD_START (0x1005a: cmd 0x0002, flags 0).
        let f = fpga_step_frame(0x0002, 0);
        self.transport.write(&f).map_err(|e| {
            step_err("FPGA_DOWNLOAD_START", FPGA_CODE_TX, &format!("tx failed: {e}"))
        })?;
        read_reply(self.transport)
            .map_err(|(code, detail)| step_err("FPGA_DOWNLOAD_START", code, &detail))?;
        self.milestones.push("fpga-download-start");

        // 2. FPGA_DOWNLOAD_QUERY_CFG (0x10094: cmd 0x0102). The reply
        //    is kept raw (idcode/version candidates).
        let f = fpga_step_frame(0x0102, 0);
        self.transport.write(&f).map_err(|e| {
            step_err("FPGA_DOWNLOAD_QUERY_CFG", FPGA_CODE_TX, &format!("tx failed: {e}"))
        })?;
        let query_reply = read_reply(self.transport)
            .map_err(|(code, detail)| step_err("FPGA_DOWNLOAD_QUERY_CFG", code, &detail))?;
        self.milestones.push("fpga-query-cfg");

        // 3. SendFpgaCode announce (0xd732: cmd 0x0202, flag 4, size BE).
        let announce = fpga_send_announce(bit.len());
        self.transport.write(&announce).map_err(|e| {
            step_err("SendFpgaCode announce", FPGA_CODE_TX, &format!("tx failed: {e}"))
        })?;
        read_reply(self.transport)
            .map_err(|(code, detail)| step_err("SendFpgaCode announce", code, &detail))?;
        self.milestones.push("fpga-send-announce");

        // 4. the file, bit-reversed, raw <=64 KiB writes, no replies
        //    (0xd76f-0xd826). The reversed image is built once; the
        //    vendor reverses in place and streams from the same buffer.
        let image = fpga_bit_reverse(bit);
        let chunks = bit.len().div_ceil(FPGA_CHUNK);
        for (i, chunk) in image.chunks(FPGA_CHUNK).enumerate() {
            self.transport.write(chunk).map_err(|_| {
                vendor_fpga(FPGA_CODE_CHUNK_TX, &format!("SendFpgaCode chunk {}/{}", i + 1, chunks))
            })?;
        }
        self.milestones.push("fpga-code-sent");

        // 5. FPGA_DOWNLOAD_END (0x100f9: cmd 0x0302).
        let f = fpga_step_frame(0x0302, 0);
        self.transport
            .write(&f)
            .map_err(|e| step_err("FPGA_DOWNLOAD_END", FPGA_CODE_TX, &format!("tx failed: {e}")))?;
        read_reply(self.transport)
            .map_err(|(code, detail)| step_err("FPGA_DOWNLOAD_END", code, &detail))?;
        self.milestones.push("fpga-download-end");

        // 6. FPGA_DOWNLOAD_DONE (0x10130: cmd 0x0102 with flag word 1).
        let f = fpga_step_frame(0x0102, 1);
        self.transport.write(&f).map_err(|e| {
            step_err("FPGA_DOWNLOAD_DONE", FPGA_CODE_TX, &format!("tx failed: {e}"))
        })?;
        read_reply(self.transport)
            .map_err(|(code, detail)| step_err("FPGA_DOWNLOAD_DONE", code, &detail))?;
        self.milestones.push("fpga-download-done");

        Ok(FpgaDownloadReport { query_reply, size: bit.len(), chunks })
    }
}
