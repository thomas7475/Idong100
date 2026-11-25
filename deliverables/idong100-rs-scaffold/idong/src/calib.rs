//! T24C16A EEPROM layout (2 KiB, dumped and decoded in earlier turns).
//!
//! Tag 0xef0123456789abcd; records at 0x0100/0x0180/0x0192/0x0300 with
//! +0x400 mirror; record = [payload][checksum = sum&0xFF][0xFF]; the
//! calibration record is 28 x f32 in OpenCV convention (fx fy cx cy x2,
//! dist[5] x2, R[3x3], T[3]).  Known-good values: |T| = 100.00 mm
//! baseline, mean f = 432.33 px, |k1| < 0.05, cx ~312-313 px (640-px
//! frames).  Serial: AB460DC8BA12C6840B49CD5786C329DD.
//!
//! Read-only: EpromRead only; the write path is deliberately NOT
//! implemented (safety rule #4).

#[derive(Debug, Clone)]
pub struct StereoCalib { /* 28 x f32 LE, OpenCV convention */ }

#[derive(Debug, Clone)]
pub struct DeviceIdentity { pub serial: [u8; 32] }

// TODO(P1.3): parse_eeprom(dump: &[u8; 2048]) -> (StereoCalib, DeviceIdentity)
// with checksum verification; tests/eeprom_decode.rs asserts the
// known-good values above.
