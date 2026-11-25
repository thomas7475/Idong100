//! open bootloader (0471:2600 iface 0) / runtime (0471:2500 MI_04),
//! bulk read/write.  Bootloader maxpacket is 64 (quirk: full-speed
//! value at high speed - accepted).  Runtime path reads 4096 B chunks.

pub enum DeviceState { Bootloader, Runtime }
// TODO(P2.1): list_devices() -> Vec<UsbDeviceInfo>; UsbTransport impl
// Transport; detach kernel drivers defensively (Windows: WinUSB).
