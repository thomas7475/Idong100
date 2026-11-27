//! open bootloader (0471:2600 iface 0) / runtime (0471:2500 MI_04),
//! bulk read/write.  Bootloader maxpacket is 64 (quirk: full-speed
//! value at high speed - accepted).  Runtime path reads 4096 B chunks.
//!
//! Discovery is read-only (device descriptors only); opening claims ONLY
//! the vendor interface.  Kernel drivers are detached from OUR interface
//! alone - the UVC/UAC interfaces are never touched.  Write retry
//! policy: 3 attempts, 100 ms apart (EasyUSB.dll).

use std::time::Duration;

use rusb::{DeviceHandle, UsbContext};

use crate::protocol::{
    EP_IN, EP_OUT, IFACE_VENDOR, IO_RETRIES, IO_RETRY_DELAY, PID_BOOTLOADER, PID_RUNTIME, PID_V10,
    PID_V20, PID_V21, VID,
};
use crate::transport::Transport;
use crate::IdongError;

/// Older generations, accepted for identification only.
pub const PID_LEGACY: [u16; 3] = [PID_V21, PID_V20, PID_V10];

/// Which state a discovered device is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceState {
    /// 0471:2600 - waiting for firmware upload.
    Bootloader,
    /// 0471:2500 - firmware running, vendor interface available.
    Runtime,
    /// 0471:2100 / 0668 / 0666 - older generation, identification only.
    Legacy(u16),
}

/// A discovered device summary.
#[derive(Debug, Clone)]
pub struct UsbDeviceInfo {
    pub bus: u8,
    pub address: u8,
    pub pid: u16,
    pub state: DeviceState,
    pub serial: Option<String>,
}

impl UsbDeviceInfo {
    /// `0471:2600`-style string.
    pub fn vid_pid(&self) -> String {
        format!("{:04x}:{:04x}", VID, self.pid)
    }
}

/// Enumerate every i-Dong sensor on the bus (bootloader, runtime,
/// legacy). READ-ONLY: device descriptors only, no interface claims.
pub fn list_devices() -> Vec<UsbDeviceInfo> {
    let mut out = Vec::new();
    let Ok(ctx) = rusb::Context::new() else { return out };
    let Ok(devices) = ctx.devices() else { return out };
    for d in devices.iter() {
        let Ok(desc) = d.device_descriptor() else { continue };
        if desc.vendor_id() != VID {
            continue;
        }
        let pid = desc.product_id();
        let state = match pid {
            PID_BOOTLOADER => DeviceState::Bootloader,
            PID_RUNTIME => DeviceState::Runtime,
            PID_V21 | PID_V20 | PID_V10 => DeviceState::Legacy(pid),
            _ => continue,
        };
        let serial = desc
            .serial_number_string_index()
            .and_then(|i| d.open().ok().and_then(|h| h.read_string_descriptor_ascii(i).ok()));
        out.push(UsbDeviceInfo { bus: d.bus_number(), address: d.address(), pid, state, serial });
    }
    out
}

/// Open bulk transport to a device face.  The handle releases the
/// claimed interface on drop.
pub struct UsbTransport {
    handle: DeviceHandle<rusb::Context>,
    iface: u8,
    mode: DeviceState,
}

impl UsbTransport {
    /// Open the bootloader (0471:2600, interface 0).
    pub fn open_bootloader() -> crate::Result<Self> {
        Self::open(PID_BOOTLOADER, 0, DeviceState::Bootloader)
    }

    /// Open the runtime vendor interface (0471:2500, MI_04).
    ///
    /// **Do not** detach the UVC/UAC interfaces' drivers - only the
    /// vendor interface is claimed, leaving video/audio to the OS.
    pub fn open_runtime() -> crate::Result<Self> {
        Self::open(PID_RUNTIME, IFACE_VENDOR, DeviceState::Runtime)
    }

    fn open(pid: u16, iface: u8, mode: DeviceState) -> crate::Result<Self> {
        let ctx = rusb::Context::new().map_err(|e| IdongError::Usb(e.to_string()))?;
        let devices = ctx.devices().map_err(|e| IdongError::Usb(e.to_string()))?;
        let mut found: Option<rusb::Device<rusb::Context>> = None;
        for d in devices.iter() {
            let desc = d.device_descriptor().map_err(|e| IdongError::Usb(e.to_string()))?;
            if desc.vendor_id() == VID && desc.product_id() == pid {
                found = Some(d);
                break;
            }
        }
        let dev = found.ok_or(IdongError::NotFound)?;
        let handle = dev.open().map_err(|e| IdongError::Usb(e.to_string()))?;
        // Detach a kernel driver from OUR interface only (defensive; on
        // Linux nothing binds to the vendor interface).
        if matches!(handle.kernel_driver_active(iface), Ok(true)) {
            handle.detach_kernel_driver(iface).map_err(|e| IdongError::Usb(e.to_string()))?;
        }
        handle.claim_interface(iface).map_err(|e| IdongError::Usb(e.to_string()))?;
        Ok(UsbTransport { handle, iface, mode })
    }

    /// Legacy PIDs are identification-only: no transport is opened.
    pub fn state(&self) -> DeviceState {
        self.mode
    }

    // ---- record plane (turn 46): raw MI_04 bulk I/O on arbitrary
    //      endpoints. The 7F 55 session keeps EP 0x02/0x82 through the
    //      `Transport` trait; the runtime record plane runs on the
    //      second bulk pair EP 0x83 IN / EP 0x03 OUT (EasyUSB pipe
    //      calibration - see protocol::ens). ----

    /// Raw bulk read from an MI_04 IN endpoint (e.g.
    /// [`crate::protocol::EP_RECORD_IN`]). A read that times out with
    /// zero bytes returns an empty vec - the ens probe treats that as
    /// "no record frame yet".
    pub fn read_bulk_ep(
        &mut self,
        ep: u8,
        len: usize,
        timeout: Duration,
    ) -> crate::Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        match self.handle.read_bulk(ep, &mut buf, timeout) {
            Ok(n) => {
                buf.truncate(n);
                Ok(buf)
            }
            Err(rusb::Error::Timeout) => Ok(Vec::new()),
            Err(e) => Err(IdongError::Usb(e.to_string())),
        }
    }

    /// Raw bulk write to an MI_04 OUT endpoint (e.g.
    /// [`crate::protocol::EP_RECORD_OUT`]) - the type-7 record ACKs.
    /// Single attempt, no retry: the usbCom loop writes its command
    /// once per record.
    pub fn write_bulk_ep(&mut self, ep: u8, buf: &[u8], timeout: Duration) -> crate::Result<usize> {
        self.handle.write_bulk(ep, buf, timeout).map_err(|e| IdongError::Usb(e.to_string()))
    }
}

// The ENS startup machine's wire seam (turn 49): EP 0x83 IN reads /
// EP 0x03 OUT writes through the same raw bulk pair the usbCom worker
// uses (EasyUSB read pipe 2 / write pipe 3).
impl crate::protocol::ens::RecordIo for UsbTransport {
    fn read_record_ep(&mut self, len: usize, timeout: Duration) -> crate::Result<Vec<u8>> {
        self.read_bulk_ep(crate::protocol::EP_RECORD_IN, len, timeout)
    }

    fn write_record_ep(&mut self, buf: &[u8], timeout: Duration) -> crate::Result<usize> {
        self.write_bulk_ep(crate::protocol::EP_RECORD_OUT, buf, timeout)
    }
}

impl Transport for UsbTransport {
    /// Write to EP 0x02 with the EasyUSB retry policy.
    fn write(&mut self, buf: &[u8]) -> crate::Result<()> {
        let mut last = None;
        for attempt in 0..IO_RETRIES {
            match self.handle.write_bulk(EP_OUT, buf, Duration::from_secs(1)) {
                Ok(_) => return Ok(()),
                Err(e) => {
                    log::warn!("bulk write attempt {attempt} failed: {e}");
                    last = Some(e);
                    std::thread::sleep(IO_RETRY_DELAY);
                }
            }
        }
        Err(IdongError::Usb(
            last.map(|e| e.to_string()).unwrap_or_else(|| "no write attempts".into()),
        ))
    }

    /// Raw bulk read from EP 0x82. A read that times out with zero bytes
    /// returns an empty vec (the reader loop treats that as "no data").
    fn read(&mut self, max: usize) -> crate::Result<Vec<u8>> {
        let mut buf = vec![0u8; max];
        match self.handle.read_bulk(EP_IN, &mut buf, Duration::from_secs(1)) {
            Ok(n) => {
                buf.truncate(n);
                Ok(buf)
            }
            Err(rusb::Error::Timeout) => Ok(Vec::new()),
            Err(e) => Err(IdongError::Usb(e.to_string())),
        }
    }
}

impl Drop for UsbTransport {
    fn drop(&mut self) {
        let _ = self.handle.release_interface(self.iface);
    }
}
