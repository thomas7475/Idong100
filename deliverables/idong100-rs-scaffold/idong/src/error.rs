//! Error model: one variant per failing state, no panics.
//!
//! `BootStep` mirrors the vendor `_InitBlobUpload` sequence one step per
//! variant so a failure pins the exact state (draft section 6, table 6).

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum IdongError {
    #[error("no i-Dong sensor found (looked for 0471:2600 or 0471:2500 MI_04)")]
    NotFound,
    #[error("device is in bootloader state; firmware upload required first")]
    BootloaderState,
    #[error("boot failed at {step:?}: {reason}")]
    Boot { step: BootStep, reason: String },
    #[error("protocol error: {0}")]
    Protocol(#[from] ProtocolError),
    #[error("bad frame magic {magic:#010x}, expected {expected:#010x}")]
    BadMagic { magic: u32, expected: u32 },
    #[error("CRC mismatch after upload: announced {announced:#010x}, device reported {device:#010x}")]
    CrcMismatch { announced: u32, device: u32 },
    #[error("USB error: {0}")]
    Usb(#[from] rusb::Error),
    #[error("firmware file {path} failed integrity check")]
    FirmwareIntegrity { path: PathBuf },
    #[error("write-capable operation refused: pass --i-know-the-write-risks")]
    WriteRiskGate,
}

/// Boot state-machine steps, in vendor sequence order.
#[derive(Debug, Clone, Copy)]
pub enum BootStep {
    DspStart,
    ReadyToLoad,
    CountAndCrc,
    PacketStream,
    CrcAck,
    DeviceId,
    DspReset,
    HandleConfig,
}

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("short buffer: need {need} bytes, have {have}")]
    Short { need: usize, have: usize },
    #[error("unexpected frame type {0:#06x}")]
    BadType(u16),
    #[error("status byte {0:#04x} not in expected set")]
    BadStatus(u8),
}
