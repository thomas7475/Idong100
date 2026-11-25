//! USB transport: discovery (0471:2600 / 0471:2500 MI_04), bulk I/O on
//! EP 0x02 / 0x82, re-enumeration wait after firmware upload.
//! Implements `idong::Transport`.  All hardware tests are #[ignore].

pub mod reenum;
pub mod udev;
pub mod usb;

pub use usb::{list_devices, DeviceState, UsbTransport};
