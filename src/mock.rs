//! MockTransport - record/replay for CI.
//!
//! Replays a scripted session (a queue of read results) and records
//! everything the state machines write, so tests/mock_replay.rs can
//! assert the exact vendor boot sequence byte-for-byte, including
//! failure injections (timeout at ready-to-load, CRC mismatch,
//! device-ID poll exhaustion). Read results may be bytes (Ok) or
//! errors (Err) - an Err result injects a timeout/IO failure at that
//! point of the session.

use std::collections::VecDeque;

use crate::error::IdongError;
use crate::transport::Transport;

/// A scripted read result: data or an injected failure.
pub type Scripted = Result<Vec<u8>, IdongError>;

/// Script builder for the common case: bytes to return on read.
pub fn reply(bytes: Vec<u8>) -> Scripted {
    Ok(bytes)
}

/// Scripted read failure (timeout injection).
pub fn timeout() -> Scripted {
    Err(IdongError::Usb("mock: read timeout".into()))
}

/// MockTransport: replays `reads`, records `writes`.
#[derive(Debug, Default)]
pub struct MockTransport {
    reads: VecDeque<Scripted>,
    /// Every write, in order, for byte-for-byte assertions.
    pub writes: Vec<Vec<u8>>,
    exhausted: usize,
}

impl MockTransport {
    /// New mock from a list of scripted read results.
    pub fn new(reads: Vec<Scripted>) -> Self {
        MockTransport { reads: reads.into(), writes: Vec::new(), exhausted: 0 }
    }

    /// Number of scripted reads consumed past the end of the script
    /// (each returns an empty buffer, like a bulk read that timed out
    /// with zero bytes).
    pub fn exhausted_reads(&self) -> usize {
        self.exhausted
    }
}

impl Transport for MockTransport {
    fn write(&mut self, buf: &[u8]) -> Result<(), IdongError> {
        self.writes.push(buf.to_vec());
        Ok(())
    }

    fn read(&mut self, _max: usize) -> Result<Vec<u8>, IdongError> {
        match self.reads.pop_front() {
            Some(r) => r,
            None => {
                self.exhausted += 1;
                Ok(Vec::new())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replays_and_records() {
        let mut m = MockTransport::new(vec![reply(vec![1, 2, 3]), reply(vec![])]);
        assert_eq!(m.read(64).unwrap(), vec![1, 2, 3]);
        assert!(m.write(&[9]).is_ok());
        assert_eq!(m.read(64).unwrap(), Vec::<u8>::new());
        assert_eq!(m.read(64).unwrap(), Vec::<u8>::new()); // exhausted
        assert_eq!(m.exhausted_reads(), 1);
        assert_eq!(m.writes, vec![vec![9]]);
    }

    #[test]
    fn failure_injection() {
        let mut m = MockTransport::new(vec![timeout()]);
        assert!(m.read(64).is_err());
    }
}
