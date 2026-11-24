//! Transport abstraction - the seam that makes CI possible.
//!
//! `idong-transport` implements this with rusb; `mock.rs` implements it
//! with recorded sessions.  The core never touches rusb directly (CI
//! dependency audit: the `idong` crate has no rusb / rclrs deps).
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
