//! Error model: one variant per failing state, no panics.
//!
//! `BootStep` mirrors the vendor `_InitBlobUpload` sequence one step per
//! variant so a failure pins the exact state (draft section 6, table 6).
//! The core crate never depends on `rusb` (CI dependency audit), so the
//! USB variant is a plain display string; `idong-transport` wraps rusb
//! errors into it at the boundary.

use std::path::PathBuf;

/// Top-level crate error.
#[derive(Debug, thiserror::Error)]
pub enum IdongError {
    #[error("no i-Dong sensor found (looked for 0471:2600 or 0471:2500 MI_04)")]
    NotFound,
    #[error("device is in bootloader state; firmware upload required first")]
    BootloaderState,
    #[error("boot failed at {step:?} ({step_str}): {reason}", step_str = .step.as_str())]
    Boot { step: BootStep, reason: String },
    #[error("protocol error: {0}")]
    Protocol(#[from] ProtocolError),
    #[error(
        "CRC mismatch after upload: announced {announced:#010x}, device reported {device:#010x}"
    )]
    CrcMismatch { announced: u32, device: u32 },
    /// Transport-layer failure, rendered as a display string so the pure
    /// core carries no rusb dependency (mock and rusb transports share it).
    #[error("USB error: {0}")]
    Usb(String),
    #[error("firmware file {path} failed integrity check (sha256 {got}, expected {want})")]
    FirmwareIntegrity { path: PathBuf, got: String, want: String },
    #[error("write-capable operation refused: pass --i-know-the-write-risks")]
    WriteRiskGate,
    /// Safety gate: boot upload not yet byte-confirmed against a USBPcap
    /// transcript of the vendor stack (draft P2.3).
    #[error("boot sequence unconfirmed (USBPcap gate P2.3): refusing to write")]
    BootUnconfirmed,
    /// The launch command was accepted; the device re-enumerates as
    /// 0471:2500. Drop this transport, open the new device, reconnect
    /// (the self-reboot recovery path uses the same signal).
    #[error("device launched: re-enumerating as 0471:2500 - reopen and reconnect")]
    NeedsReenumeration,
}

impl IdongError {
    /// Attach the boot step to an error, for `BootUploader` internals.
    pub(crate) fn boot(step: BootStep, reason: impl Into<String>) -> Self {
        IdongError::Boot { step, reason: reason.into() }
    }
}

/// Boot state-machine steps, in vendor sequence order
/// (`_InitBlobUpload`, xEyeUSB.dll).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootStep {
    /// TX DSP-start; expect RX type 2 "Ready to load program!" (3 s).
    DspStart,
    /// TX type 2: packet count + CRC ("Tx packet count & crc!").
    CountAndCrc,
    /// TX type 3 x 64-byte firmware packets (6 s per packet).
    PacketStream,
    /// RX type 3 CRC result; abort on status word 0xF at frame offset +8.
    CrcAck,
    /// RX type 2 device-ID / DSP version (300 ms poll x <= 10).
    DeviceId,
    /// "Get DSP Reset ACK OK" -> TX type 7 (1 s).
    DspReset,
    /// Handle-config read/write + lens parameters.
    HandleConfig,
}

impl BootStep {
    pub fn as_str(self) -> &'static str {
        match self {
            BootStep::DspStart => "dsp-start",
            BootStep::CountAndCrc => "count-and-crc",
            BootStep::PacketStream => "packet-stream",
            BootStep::CrcAck => "crc-ack",
            BootStep::DeviceId => "device-id",
            BootStep::DspReset => "dsp-reset",
            BootStep::HandleConfig => "handle-config",
        }
    }
}

/// Wire-protocol violations that are not boot failures.
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("short buffer: need {need} bytes, have {have}")]
    Short { need: usize, have: usize },
    #[error("bad magic {magic:#010x} (expected {expected:#010x})")]
    BadMagic { magic: u32, expected: u32 },
    #[error("unexpected frame type {kind} (expected {expected})")]
    BadType { kind: u16, expected: u16 },
    #[error("status byte {0:#04x} not in expected set")]
    BadStatus(u8),
    /// Runtime command not decoded from the vendor stack (safety rule
    /// "no speculative writes" - `cmd.rs` rejects it).
    #[error("command {0} is not a vendor-decoded runtime command")]
    UnknownCommand(u8),
    #[error("keep-alive not answered within {0:?}")]
    KeepaliveTimeout(std::time::Duration),
    #[error("read timed out after {0:?}")]
    ReadTimeout(std::time::Duration),
}

pub type Result<T, E = IdongError> = std::result::Result<T, E>;
