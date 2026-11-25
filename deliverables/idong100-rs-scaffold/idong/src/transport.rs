//! Transport abstraction - the seam that makes CI possible.
//!
//! `idong-transport` implements this with rusb; `mock.rs` implements it
//! with recorded sessions.  The core never touches rusb directly.

pub trait Transport {
    /// Bulk write to EP 0x02 (commands/upload).  The ONLY write path.
    fn write(&mut self, buf: &[u8]) -> Result<(), crate::IdongError>;
    /// Bulk read from EP 0x82 (data).
    fn read(&mut self, max: usize) -> Result<Vec<u8>, crate::IdongError>;
}
