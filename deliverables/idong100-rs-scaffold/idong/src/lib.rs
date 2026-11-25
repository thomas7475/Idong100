//! # idong - pure protocol core for the i-Dong Sensor V2.6
//!
//! Zero USB and zero ROS dependencies: everything here is testable with
//! `cargo test` alone, against golden vectors under `tests/data/`.
//! The crate facade is [`Idong`]; see the implementation draft (turn 13)
//! section 4 for the file map and section 7 for the phase plan.
//!
//! Facts in this crate are cited to their decompiled source in
//! `docs/PROTOCOL.md` - the single source of truth for constants.

#![deny(unsafe_code)]

pub mod calib;
pub mod error;
pub mod handle;
pub mod marker;
pub mod mock;
pub mod protocol;
pub mod stream;
pub mod transport;

pub use calib::{DeviceIdentity, StereoCalib};
pub use error::{BootStep, IdongError};
pub use handle::HandleEvent;
pub use marker::{Marker, MarkerFrame, Quality};
pub use stream::{Idong, StreamStats};

/// Firmware set the user must supply (never bundled).
/// Hashes pinned from the turn-12 manifest.
pub struct FirmwareSet {
    pub ldr: Vec<u8>,  // ens_code.ldr       Blackfin BF527
    pub bit: Vec<u8>,  // A_a_v4.4_BG0313.bit  Spartan-3
    pub hex: Vec<u8>,  // app_m8051_2611.hex   8051
}

// TODO(P1): verify_firmware() pins sha256 before any upload (safety #3).
