//! `_InitBlobUpload` state machine (decoded from xEyeUSB.dll).
//!
//! Sequence: TX DSP-start -> RX type 2 "Ready to load program!" ->
//! TX type 2 packet-count+CRC -> TX type-3 64 B packets (6 s each) ->
//! RX type 3 CRC result (abort on mismatch) -> device-ID exchange
//! (300 ms x 10) -> TX type 7 DSP reset. Staging order of the three
//! firmware images is open item #1 - resolved at the P2.3 USBPcap gate.
//!
//! Safety: refuses to run unless the risk gate is passed (error.rs
//! `WriteRiskGate`); firmware sha256-verified first.

use crate::error::BootStep;

pub struct BootUploader<'a> { pub blob: &'a [u8] }
impl<'a> BootUploader<'a> {
    /// Runs the full vendor sequence over any `Transport`.
    // TODO(P1.4): implement against MockTransport; replay test asserts
    // the exact byte sequence (tests/mock_replay.rs).
    pub fn run(&mut self) -> Result<crate::calib::DeviceIdentity, crate::IdongError> {
        Err(crate::IdongError::Boot { step: BootStep::DspStart, reason: "P1.4 not implemented".into() })
    }
}
