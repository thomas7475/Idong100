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
pub mod frames;
pub mod records;

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

// ---- Runtime framing (xeyezHal.dll v2.6.0.6) ----
/// Runtime frame header: 7F 55 at byte offset 0-1.
pub const RUNTIME_MAGIC: [u8; 2] = [0x7f, 0x55];
/// Runtime TX timeout (1 s).
pub const RUNTIME_TX_TIMEOUT: Duration = Duration::from_secs(1);
/// Runtime RX timeout (5 s).
pub const RUNTIME_RX_TIMEOUT: Duration = Duration::from_secs(5);
/// Runtime command payload maximum (56 B).
pub const RUNTIME_PAYLOAD_MAX: usize = 56;

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
