//! Wire constants - one home, every value cited.
//!
//! Sources: xeyez.inf, xEyeUSB.dll (`_InitBlobUpload`, disassembly in
//! `research/idong100/agents/09-xeyez-analysis/artifacts/
//! xeyez_boot_protocol.txt`), xeyezHal.dll v2.6.0.6 (packet builder
//! @RVA 0x10003010, command engine @0x100030a0), xEyeClass.dll
//! (`GetADataFromUSB` @RVA 0x20f0), xEyeUsb.ini / xEye.ini, and the
//! T24C16A dump decode. Full table with per-row citations:
//! `docs/PROTOCOL.md`.

pub mod boot;
pub mod cmd;
pub mod ens;
pub mod frames;
pub mod records;
pub mod runtime;
pub mod video;

use std::time::Duration;

// ---- USB identity (xeyez.inf, both driver generations) ----
pub const VID: u16 = 0x0471;
/// Bootloader PID ("TOLTech xeyez Bootloader" in xeyez.inf).
pub const PID_BOOTLOADER: u16 = 0x2600;
/// Runtime PID - composite device; the vendor bulk interface is MI_04
/// ("TOLTech xeyez v2.5"). Interfaces 0-3 are standard UVC/UAC.
pub const PID_RUNTIME: u16 = 0x2500;
/// Older generations, accepted for identification only (xeyez.inf).
pub const PID_V21: u16 = 0x2100;
pub const PID_V20: u16 = 0x0668;
pub const PID_V10: u16 = 0x0666;
pub const IFACE_VENDOR: u8 = 4;
/// Bulk endpoints (bootloader descriptors + EasyUSB.dll).
pub const EP_OUT: u8 = 0x02;
pub const EP_IN: u8 = 0x82;

// ---- Endpoints / chunking (EasyUSB.dll + xeyez.sys) ----
/// Bootloader bulk maxpacket (device declares 64 even at high speed -
/// spec violation, logged and accepted; CONTEXT.md).
pub const BOOT_PACKET: usize = 64;
/// Runtime read chunk: xeyez.sys `MaximumTransferSize=4096`;
/// EasyUSB accumulates up to 16 KiB per read burst.
pub const RUNTIME_CHUNK: usize = 4096;
pub const RUNTIME_ACCUM: usize = 16 * 1024;
/// EasyUSB retry policy: 3 attempts, 100 ms apart.
pub const IO_RETRIES: u32 = 3;
pub const IO_RETRY_DELAY: Duration = Duration::from_millis(100);

// ---- Boot framing (xEyeUSB.dll; PU/OD magics, little-endian u32 @ +0) ----
/// Host-to-device magic (LE bytes `50 55 7f 7f` = "PU\x7f\x7f").
pub const MAGIC_H2D: u32 = 0x7f7f_5550;
/// Device-to-host magic (LE bytes `4f 44 7f 7f` = "OD\x7f\x7f").
pub const MAGIC_D2H: u32 = 0x7f7f_444f;
/// "Ready to load program!" / CRC-result / device-ID outer timeout (3 s).
pub const TIMEOUT_READY: Duration = Duration::from_secs(3);
/// Per 64-byte firmware packet (6 s).
pub const TIMEOUT_PACKET: Duration = Duration::from_secs(6);
/// DSP-reset ACK and keep-alive answer budget (1 s).
pub const TIMEOUT_ACK: Duration = Duration::from_secs(1);
/// Device-ID re-poll interval; up to [`DEVICE_ID_TRIES`] attempts.
pub const DEVICE_ID_POLL: Duration = Duration::from_millis(300);
pub const DEVICE_ID_TRIES: u8 = 10;

// ---- Boot frame types (u16 @ frame offset +4; xEyeUSB.dll) ----
/// Boot: device ready for program load ("Ready to load program!").
pub const TYPE_READY_TO_LOAD: u16 = 2;
/// Boot: packet count + CRC announce (TX) / CRC verdict (RX).
pub const TYPE_COUNT_CRC: u16 = 2;
/// Boot: firmware data packet (64 B) / CRC verdict (RX).
pub const TYPE_FIRMWARE_PACKET: u16 = 3;
/// Runtime: link status; host replies with word 0x11.
pub const TYPE_LINK_STATUS: u16 = 4;
/// Runtime: DSP reset (sent after "Get DSP Reset ACK OK").
pub const TYPE_DSP_RESET: u16 = 7;
/// CRC-failure status word at RX frame offset +8 (boot_protocol @0x1000fdee).
pub const CRC_FAIL_WORD: u16 = 0x000F;
/// Link-status reply word (tx_builder @0x10010291).
pub const LINK_STATUS_REPLY_WORD: u16 = 0x0011;

// ---- Runtime framing (observed, turn 32: cap2/cap3/cap4) ----
/// Runtime frame header: 7F 55 <cmd u16 LE> <flags> <rsv> (6 B).
/// Full observed command set: `protocol::runtime`.
pub const RUNTIME_MAGIC: [u8; 2] = [0x7f, 0x55];
/// Runtime TX timeout (1 s).
pub const RUNTIME_TX_TIMEOUT: Duration = Duration::from_secs(1);
/// Runtime RX timeout (5 s).
pub const RUNTIME_RX_TIMEOUT: Duration = Duration::from_secs(5);
/// Runtime command payload maximum (56 B, the largest observed body).
pub const RUNTIME_PAYLOAD_MAX: usize = 56;
/// MCU image load base (0x0201 verify args) and observed size
/// (0x0101 begin args): 0x8000..0xD3A3 = 21,411 B.
pub const MCU_IMAGE_BASE: u16 = 0x8000;
pub const MCU_IMAGE_SIZE: u16 = 0x53A3;

// ---- Keep-alive (xEyeClass reader thread) ----
/// Device polls with 0xFFF; the host must answer 0xFFE within 1 s.
pub const KEEPALIVE_RX: u16 = 0x0fff;
pub const KEEPALIVE_TX: u16 = 0x0ffe;

// ---- Marker record (xEyeClass.dll GetADataFromUSB @RVA 0x20f0) ----
/// Slots per record (loop bound 0xA in the disassembly).
pub const MARKER_SLOTS: usize = 10;
/// Slot stride (88 B); slots occupy +0x000..+0x370.
pub const MARKER_SLOT_SIZE: usize = 88;
/// Trailer between the slots and the mode flag (72 B).
pub const MARKER_TRAILER: usize = 0x48;
/// Mode flag u32 at +0x3B8 (== 1 -> frame skipped by the pipe writer).
pub const MARKER_RECORD_MIN: usize = 0x3bc;
/// Slot quality gate: slot+0x24 (i32) >= 50 -> tracked.
pub const QUALITY_GATE: i32 = 50;

// ---- Runtime record plane (turn 46: xEyeUSB.dll usbCom worker @RVA 0x11770,
//      ens subsystem @0xeb00-0x10900, all disassembled - see docs/PROTOCOL.md
//      "Runtime record plane") ----
/// Vendor bulk pair the record plane runs on: the 7F 55 session stays on
/// EP 0x02/0x82 (calibrated: EasyUSB write pipe 0 / read pipe 1, the
/// USBPcap-proven pair), and usbCom + the ens loader use read pipe 2 /
/// write pipe 3 -> the second MI_04 bulk pair (512 B maxpacket).
pub const EP_RECORD_IN: u8 = 0x83;
pub const EP_RECORD_OUT: u8 = 0x03;
/// OD frame type announcing one record is ready (u16 @ status +4).
pub const TYPE_RECORD_READY: u16 = 0x00f0;
/// PU frame type of the per-record ACK/command the host sends
/// (built in the usbCom loop, payload filled by classifying.dll).
pub const TYPE_RECORD_CMD: u16 = 0x0007;
/// Length of the OD status frame and the PU type-7 command frame (64 B).
pub const RECORD_FRAME_LEN: usize = 64;
/// Length of one raw wire record (0x280 B, usbCom loop @0x11c7b).
pub const RECORD_WIRE_LEN: usize = 0x280;
/// Wire-record trailer: the OD magic dword ends every record
/// (usbCom loop @0x11d13, checked at record +0x27C).
pub const RECORD_TRAILER_OFF: usize = 0x27c;
/// Status byte inside the wire record (f660 transform @0xf667).
pub const RECORD_STATUS_OFF: usize = 0x25c;
/// Startup probe: read RECORD_FRAME_LEN from EP 0x83 expecting an OD
/// type-0xF0 frame, 0x64 ms timeout, up to 10 tries (probe @RVA 0x10640,
/// called from usbCom @0x11886). No frame within budget -> the runtime
/// ens_code.ldr upload state machine (@RVA 0xf790) would run first.
pub const ENS_PROBE_TRIES: u8 = 10;
pub const ENS_PROBE_TIMEOUT: Duration = Duration::from_millis(100);
/// The xEye3Ddata host element the wire record eventually becomes in
/// the vendor stack (queue "Data2Class", element size 0x3F0; the 0x280-B
/// wire record itself carries raw blob data, the marker x/y/z are
/// host-computed by classifying.dll - turn 46 finding).
pub const RECORD_ELEMENT_LEN: usize = 0x3f0;

// ---- ENS startup: the record-plane bring-up (turn 49: the 8-state
//      cold machine @RVA 0xf790 + the hot-start probe @0x10640 +
//      hot-start finish @0x10900, all statically disassembled from
//      xEyeUSB.dll - no capture needed; see docs/PROTOCOL.md "ENS
//      startup" and research/idong100/agents/21-turn49/) ----
/// PU type: "DSP Start", the cold machine's opening command (state 1,
/// `0x105f0(buf, 1, 0x40)` from `0xfa30`; log "Tx DSP Start").
pub const ENS_TYPE_START: u16 = 0x0001;
/// PU/OD type: the code-load announce (TX: file size @+8, CRC32 @+0xC)
/// and its go-ahead answer (state 2, `0xfae0`; logs "Tx packet count &
/// crc!" / "Ready to load program!").
pub const ENS_TYPE_CODELOAD: u16 = 0x0002;
/// OD type: the code-load final verdict, status word 0xF at +8 (state 2;
/// log "Error: crc result is not correct!" when it differs).
pub const ENS_TYPE_CODELOAD_DONE: u16 = 0x0003;
/// PU/OD type: the handle-match (gamepad pairing) select + link-status
/// frames; the reply word at +8 selects the sub-code (states 4/6).
pub const ENS_TYPE_LINK: u16 = 0x0004;
/// OD type: the handle-match config prompt (state 4, `0xfe20`).
pub const ENS_TYPE_HANDLE_CFG: u16 = 0x0005;
/// PU/OD type: the lens-parameter frame (128 B on the wire; state 7,
/// `0x104e0` reads 0x80 B, usbCom copies 0x78 B from +8 for
/// `SetLensParam`).
pub const ENS_TYPE_LENS: u16 = 0x0006;
/// OD type 0xFFF: the TSOL keep-alive poll; the host answers PU 0xFFE
/// (the "IDLE msg") within 1 s (pairing loop @0x10130; also the
/// xEyeClass keep-alive).
pub const ENS_TYPE_KEEPALIVE_RX: u16 = 0x0fff;
pub const ENS_TYPE_KEEPALIVE_TX: u16 = 0x0ffe;
/// PU type 0x10: the hot-start command (hot-start finish @0x10900;
/// answered by OD 0x10 with the 128-B lens frame).
pub const ENS_TYPE_HOT_START: u16 = 0x0010;
/// PU type 0x25/0x26: handle-match on/off (state 4; `NeedHandleMatch`
/// in xEyeUsb.ini, shipped value 1 -> 0x25).
pub const ENS_TYPE_MATCH_ON: u16 = 0x0025;
pub const ENS_TYPE_MATCH_OFF: u16 = 0x0026;
/// OD type 0x9001/PU 0x8001: the device-ID exchange of the cold
/// machine's state 5 ("ID & Ver Resend") and the hot-start finish.
pub const ENS_TYPE_ID_REQ: u16 = 0x8001;
pub const ENS_TYPE_DID: u16 = 0x9001;
/// OD type 1: the "DEVICE ID" frame - 32 ID bytes at +8, 8 version
/// bytes at +0x28 (states 1 and 5, and the boot plane's device-ID step).
pub const ENS_TYPE_DEVICE_ID: u16 = 0x0001;
/// Device-ID frame: ID length @+8 and DSP-version length @+0x28
/// (`0xffb0` copies 0x20 then 8 B; `0x10900` the same).
pub const ENS_DEVICE_ID_LEN: usize = 32;
pub const ENS_DEVICE_VER_LEN: usize = 8;
/// Lens-parameter frame length (the OD type-6 read is 0x80 B; the
/// params start at +8, so the `SetLensParam` block is 0x78 B).
pub const ENS_LENS_FRAME_LEN: usize = 0x80;
pub const ENS_LENS_PARAM_LEN: usize = 0x78;
/// Link-status reply words at +8 of the PU type-4 answer (pairing loop
/// `0x10130`: found -> 0x11, not-found -> 0xFF, found-timeout -> 3,
/// found-over -> 9 - the exit sub-code).
pub const ENS_LINK_REPLY_FOUND: u16 = 0x0011;
pub const ENS_LINK_REPLY_NOT_FOUND: u16 = 0x00ff;
pub const ENS_LINK_REPLY_TIMEOUT: u16 = 0x0003;
pub const ENS_LINK_REPLY_OVER: u16 = 0x0009;
/// Link-status sub-codes at +8 of the OD type-4 frame (1 = found with
/// UID pair, 2 = found-timeout, 0xF = not found, 8 = found-over: the
/// loop-exit code).
pub const ENS_LINK_FOUND: u16 = 0x0001;
pub const ENS_LINK_TIMEOUT: u16 = 0x0002;
pub const ENS_LINK_NOT_FOUND: u16 = 0x000f;
pub const ENS_LINK_OVER: u16 = 0x0008;
/// Code-load verdict: status word 0xF at +8 of the OD type-3 answer
/// means CRC OK (`0xfae0` @0x10e0 region; log "crc result is not
/// correct!" when it differs).
pub const ENS_CODELOAD_OK_WORD: u16 = 0x000f;
/// State-4 prompt budget: the vendor polls the OD type-5 frame with a
/// 300 ms timeout up to 11 times (`0xfe20`, tries 0..=10 with 100 ms
/// sleeps - about 4 s of quiet tolerated).
pub const ENS_HANDLE_CFG_TRIES: u8 = 11;
pub const ENS_HANDLE_CFG_TIMEOUT: Duration = Duration::from_millis(300);
/// Cold-machine outer timeouts: 3 s for the 64-B exchanges, 6 s per
/// code-load chunk (`0xfae0`: 0xBB8 announce/answers, 0x1770 chunks).
pub const ENS_TIMEOUT_FRAME: Duration = Duration::from_millis(3000);
pub const ENS_TIMEOUT_CHUNK: Duration = Duration::from_millis(6000);
/// Hot-start probe: 100 ms per 64-B try, 10 silent tries before the
/// cold machine runs (probe `0x10640`; a wrong-type frame resets the
/// budget, three record cycles with two ACKs then finish).
pub const ENS_HOT_PROBE_TRIES: u8 = 10;
/// Vendor pacing between state-machine iterations (`0xf790` @0xf922:
/// `Sleep(0x32)`).
pub const ENS_STATE_PAUSE: Duration = Duration::from_millis(50);

// ---- Stream config defaults (xEye.ini / xEyeUsb.ini) ----
/// xEyeFrame=50 (USBComFPS=35 transport polling).
pub const DEFAULT_FPS: u32 = 50;
/// Link-quality thresholds Green/Yellow/Red (xEyeUsb.ini).
pub const QUALITY_GREEN: i32 = 20;
pub const QUALITY_YELLOW: i32 = 35;
pub const QUALITY_RED: i32 = 35;
/// Vendor noise-filter ring (xEyeUsb.ini `RingSize=200`).
pub const QUALITY_WINDOW: usize = 200;

// ---- EEPROM T24C16A (verified dump decode, MD5 dc02fce8...) ----
/// ASCII header tag at 0x0000 (16 B).
pub const EEPROM_HEADER_TAG: &[u8; 16] = b"ef0123456789abcd";
/// EEPROM size in bytes.
pub const EEPROM_SIZE: usize = 2048;
/// Mirror stride: every record is duplicated at +0x400.
pub const EEPROM_MIRROR_STRIDE: usize = 0x400;
