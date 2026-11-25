//! Wire constants - one home, every value cited.
//!
//! Sources: xeyez.inf, xEyeUSB.dll (_InitBlobUpload), xeyezHal.dll v2.6.0.6
//! (packet builder @VA 0x10003010, engine @0x100030a0), xEyeClass.dll
//! (GetADataFromUSB @RVA 0x20f0), xEyeUsb.ini, T24C16A dump decode.
//! Full table: docs/PROTOCOL.md.

pub mod boot;
pub mod cmd;
pub mod frames;
pub mod records;

use std::time::Duration;

// ---- USB identity (xeyez.inf) ----
pub const VID: u16 = 0x0471;
pub const PID_BOOTLOADER: u16 = 0x2600;
pub const PID_RUNTIME: u16 = 0x2500; // composite; vendor interface = 4
pub const IFACE_VENDOR: u8 = 4;

// ---- Boot framing (xEyeUSB.dll) ----
pub const MAGIC_H2D: u32 = 0x7f7f_5550; // bytes "PU" + 0x7f 0x7f (LE)
pub const MAGIC_D2H: u32 = 0x7f7f_444f; // bytes "OD" + 0x7f 0x7f (LE)
pub const BOOT_PACKET: usize = 64;
pub const TIMEOUT_READY: Duration = Duration::from_secs(3);
pub const TIMEOUT_PACKET: Duration = Duration::from_secs(6);
pub const TIMEOUT_RESET: Duration = Duration::from_secs(1);
pub const DEVICE_ID_POLL: Duration = Duration::from_millis(300);
pub const DEVICE_ID_TRIES: u8 = 10;

// ---- Runtime framing (xeyezHal.dll) ----
pub const RUNTIME_MAGIC: [u8; 2] = [0x7f, 0x55];
pub const RUNTIME_TX_TIMEOUT: Duration = Duration::from_secs(1);
pub const RUNTIME_RX_TIMEOUT: Duration = Duration::from_secs(5);
pub const RUNTIME_PAYLOAD_MAX: usize = 56;

// ---- Keep-alive / link (xEyeClass reader thread) ----
pub const KEEPALIVE_RX: u16 = 0x0fff;
pub const KEEPALIVE_TX: u16 = 0x0ffe;

// ---- Marker record (xEyeClass.dll GetADataFromUSB) ----
pub const MARKER_SLOTS: usize = 10;
pub const MARKER_SLOT_SIZE: usize = 88;
pub const MARKER_RECORD_MIN: usize = 0x3bc;
pub const QUALITY_GATE: i32 = 50; // slot+0x24 >= 50 -> tracked

// ---- Stream config defaults (xEyeUsb.ini) ----
pub const DEFAULT_FPS: u32 = 50; // xEyeFrame=50 (USBComFPS=35 transport)
pub const QUALITY_WINDOW: usize = 200; // RingSize
