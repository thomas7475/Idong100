//! USB transport: discovery (0471:2600 / 0471:2500 MI_04), bulk I/O on
//! EP 0x02 / 0x82, re-enumeration wait after firmware upload.
//! Implements `idong::Transport`.  All hardware tests are #[ignore].
//!
//! Read-only by construction: the only write path is `Transport::write`,
//! whose callers are the safety-gated boot uploader and the documented
//! runtime replies. Opening a device claims ONLY the vendor interface
//! (MI_04 on the runtime composite - UVC/UAC interfaces 0-3 are left to
//! the OS drivers).

pub mod reenum;
pub mod udev;
pub mod usb;

pub use usb::{list_devices, DeviceState, UsbDeviceInfo, UsbTransport};
