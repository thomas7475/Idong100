//! MockTransport - record/replay for CI.
//!
//! Replays a recorded session (bytes + expected reads) and records
//! everything the state machines write, so tests/mock_replay.rs can
//! assert the exact vendor boot sequence byte-for-byte, including
//! failure injections (timeout at ready-to-load, CRC mismatch,
//! device-ID poll exhaustion).

// TODO(P1.4): implement Transport; session files under tests/data/.
