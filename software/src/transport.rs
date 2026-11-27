//! Transport abstraction - the seam that makes CI possible.
//!
//! `transport::usb` (behind the `usb` feature) implements this with
//! rusb/libusb; `mock.rs` implements it with recorded sessions.  The
//! core never touches rusb directly (CI dependency audit: plain
//! `cargo test` runs without libusb installed).
//!
//! Read/write are blocking with the caller's timeout policy; the
//! implementor maps its native errors into [`IdongError::Usb`] strings.

/// Bulk transport to the device's vendor interface.
///
/// `write` is the ONLY write path in the whole driver (EP 0x02); every
/// write-capable caller is safety-gated (boot uploader + runtime replies).
pub trait Transport {
    /// Bulk write to EP 0x02 (commands/upload). The ONLY write path.
    fn write(&mut self, buf: &[u8]) -> crate::error::Result<()>;
    /// Bulk read from EP 0x82 (data). An empty vec means "no data within
    /// the read budget" (timeout), not an error.
    fn read(&mut self, max: usize) -> crate::error::Result<Vec<u8>>;
}

impl Transport for Box<dyn Transport + Send> {
    fn write(&mut self, buf: &[u8]) -> crate::error::Result<()> {
        (**self).write(buf)
    }
    fn read(&mut self, max: usize) -> crate::error::Result<Vec<u8>> {
        (**self).read(max)
    }
}

/// rusb/libusb implementation (rusb): discovery + bulk I/O.
/// Feature `usb` - absent from a hardware-free CI build.
#[cfg(feature = "usb")]
pub mod usb;

/// Re-enumeration handling: the 2600 -> 2500 swap after a launch or a
/// self-reboot. Pure decision core here; the rusb poll loop behind the
/// `usb` feature.
pub mod reenum;

/// udev rule for unprivileged access on Linux (no root requirement).
pub mod udev;

/// V4L2 video-node discovery - the pure sysfs matcher for the P2.5
/// Linux video backend (which /dev/videoN is the sensor).
pub mod v4l2;

/// usbfs (the /dev/bus/usb device node) - the pure half of the P2.5b
/// raw ISO backend: config-descriptor walk, wMaxPacketSize decode, UVC
/// control builders. The ioctl engine lives in src/examples/video.rs
/// (`--iso`) until its first bench PASS promotes it into the crate.
pub mod usbfs;
