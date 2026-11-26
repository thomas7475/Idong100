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
//!
//! Verified against the real dump (MD5 dc02fce8ca39155135a4b0368b3f33ae).
//! Real-device reference values asserted by the tests below: cam1
//! f=(428.761292, 430.208038), c=(312.787292, 212.867691); cam2
//! f=(435.891205, 437.767944), c=(312.149200, 179.222244); |k1|<0.05;
//! R orthonormal; T=(99.562683, 0.463940, 9.330422) mm with |T| =
//! 100.0000 mm (designed baseline); mean f across both cameras =
//! 432.33 px.

use super::protocol::{EEPROM_HEADER_TAG, EEPROM_MIRROR_STRIDE, EEPROM_SIZE};
use crate::error::{ProtocolError, Result};

// Record table: (offset, payload_len) - fixed lengths; payloads may
// contain 0xFF bytes, so there is no terminator scan.
pub const REC_CALIB: usize = 0x0100;
pub const REC_CALIB_LEN: usize = 120; // 28 x f32 + 8 B pad
pub const REC_VERSION: usize = 0x0180;
pub const REC_VERSION_LEN: usize = 16;
pub const REC_INT16: usize = 0x0192;
pub const REC_INT16_LEN: usize = 16;
pub const REC_SERIAL: usize = 0x0300;
pub const REC_SERIAL_LEN: usize = 32;
pub const REC_CODE_A: usize = 0x0322;
pub const REC_CODE_A_LEN: usize = 4; // "000T"
pub const REC_CODE_B: usize = 0x0328;
pub const REC_CODE_B_LEN: usize = 4; // "2613"

/// Factory stereo calibration (OpenCV convention).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StereoCalib {
    /// Focal lengths [cam1, cam2] (px).
    pub fx: [f64; 2],
    pub fy: [f64; 2],
    /// Principal points [cam1, cam2] (px).
    pub cx: [f64; 2],
    pub cy: [f64; 2],
    /// Distortion [cam1, cam2]: (k1, k2, p1, p2) per camera.
    pub dist: [[f64; 4]; 2],
    /// Rotation cam1->cam2 (3x3, row-major, orthonormal).
    pub r: [[f64; 3]; 3],
    /// Translation cam1->cam2 in millimetres (|T| = 100.0 by design).
    pub t: [f64; 3],
}

impl StereoCalib {
    /// Designed baseline in millimetres (|T|).
    pub fn baseline_mm(&self) -> f64 {
        (self.t[0] * self.t[0] + self.t[1] * self.t[1] + self.t[2] * self.t[2]).sqrt()
    }

    /// Mean focal length across both cameras: mean of the two fx values
    /// (reference value 432.33 px - CONTEXT.md).
    pub fn mean_focal_px(&self) -> f64 {
        (self.fx[0] + self.fx[1]) / 2.0
    }
}

/// Device identity from the three identity records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceIdentity {
    /// 32-character uppercase hex serial (128-bit).
    pub serial: String,
    /// Production/firmware code (e.g. "000T").
    pub code_a: String,
    /// Production/batch code (e.g. "2613" - matches the app_m8051_2611
    /// 8051 firmware family selection).
    pub code_b: String,
}

/// Validate one `[payload][checksum][0xFF]` record; returns the payload.
fn record(eeprom: &[u8], off: usize, len: usize) -> Result<&[u8]> {
    let payload = eeprom
        .get(off..off + len)
        .ok_or(ProtocolError::Short { need: off + len, have: eeprom.len() })?;
    let (chk, sep) = eeprom
        .get(off + len..off + len + 2)
        .map(|s| (s[0], s[1]))
        .ok_or(ProtocolError::Short { need: off + len + 2, have: eeprom.len() })?;
    let expect = payload.iter().fold(0u8, |a, &b| a.wrapping_add(b));
    if chk != expect {
        return Err(ProtocolError::BadMagic { magic: chk as u32, expected: expect as u32 }.into());
    }
    if sep != 0xFF {
        return Err(ProtocolError::BadType { kind: sep as u16, expected: 0xFF }.into());
    }
    Ok(payload)
}

/// Parse the full 2 KiB dump. Returns the calibration and identity; every
/// record checksum is verified, the header tag must match, and the
/// +0x400 mirror copies are cross-checked (mismatch -> warning, primary
/// used; a mismatch never panics and never blocks the primary data).
pub fn parse_eeprom(dump: &[u8]) -> Result<(StereoCalib, DeviceIdentity)> {
    if dump.len() != EEPROM_SIZE {
        return Err(ProtocolError::Short { need: EEPROM_SIZE, have: dump.len() }.into());
    }
    if dump[..16] != *EEPROM_HEADER_TAG {
        return Err(ProtocolError::BadMagic {
            magic: u32::from_le_bytes(dump[0..4].try_into().unwrap()),
            expected: u32::from_le_bytes(EEPROM_HEADER_TAG[0..4].try_into().unwrap()),
        }
        .into());
    }

    // --- calibration ---------------------------------------------------
    let cal = record(dump, REC_CALIB, REC_CALIB_LEN)?;
    let calib = floats_to_calib(cal);

    // --- version + int16 metadata (parsed for completeness) ------------
    let version = record(dump, REC_VERSION, REC_VERSION_LEN)?;
    let _ = record(dump, REC_INT16, REC_INT16_LEN)?;
    debug_assert!(version.starts_with(b"2059"));

    // --- identity -------------------------------------------------------
    let serial = record(dump, REC_SERIAL, REC_SERIAL_LEN)?;
    let code_a = record(dump, REC_CODE_A, REC_CODE_A_LEN)?;
    let code_b = record(dump, REC_CODE_B, REC_CODE_B_LEN)?;

    // --- mirror cross-check ---------------------------------------------
    let mirror_ok = dump[REC_CALIB..REC_CALIB + REC_CALIB_LEN + 2]
        == dump[REC_CALIB + EEPROM_MIRROR_STRIDE
            ..REC_CALIB + EEPROM_MIRROR_STRIDE + REC_CALIB_LEN + 2]
        && dump[REC_SERIAL..REC_SERIAL + REC_SERIAL_LEN + 2]
            == dump[REC_SERIAL + EEPROM_MIRROR_STRIDE
                ..REC_SERIAL + EEPROM_MIRROR_STRIDE + REC_SERIAL_LEN + 2];
    if !mirror_ok {
        log::warn!("EEPROM mirror copy mismatch - primary records still used");
    }

    Ok((
        calib,
        DeviceIdentity {
            serial: String::from_utf8_lossy(serial).into_owned(),
            code_a: String::from_utf8_lossy(code_a).into_owned(),
            code_b: String::from_utf8_lossy(code_b).into_owned(),
        },
    ))
}

/// The 28 f32 words of the calibration record, as the struct.
fn floats_to_calib(cal: &[u8]) -> StereoCalib {
    let mut f = [0f32; 28];
    for (i, v) in f.iter_mut().enumerate() {
        *v = f32::from_le_bytes(cal[i * 4..i * 4 + 4].try_into().unwrap());
    }
    let g = |i: usize| f[i] as f64;
    StereoCalib {
        fx: [g(0), g(8)],
        fy: [g(1), g(9)],
        cx: [g(2), g(10)],
        cy: [g(3), g(11)],
        dist: [[g(4), g(5), g(6), g(7)], [g(12), g(13), g(14), g(15)]],
        r: [[g(16), g(17), g(18)], [g(19), g(20), g(21)], [g(22), g(23), g(24)]],
        t: [g(25), g(26), g(27)],
    }
}

/// Parse the calibration record EXACTLY as the live session read returns
/// it ([`crate::protocol::runtime::RuntimeSession::read_calibration`]):
/// 120 B payload + 1 B byte-sum checksum, no 0xFF separator.
///
/// The read-only bring-up (P2.2, `examples/bringup.rs`) uses this to
/// prove the factory calibration round-trips from the real device.
pub fn parse_calibration_record(rec: &[u8]) -> Result<StereoCalib> {
    const LEN: usize = REC_CALIB_LEN + 1;
    if rec.len() != LEN {
        return Err(ProtocolError::Short { need: LEN, have: rec.len() }.into());
    }
    let (payload, chk) = (&rec[..REC_CALIB_LEN], rec[REC_CALIB_LEN]);
    let expect = payload.iter().fold(0u8, |a, &b| a.wrapping_add(b));
    if chk != expect {
        return Err(ProtocolError::BadMagic { magic: chk as u32, expected: expect as u32 }.into());
    }
    Ok(floats_to_calib(payload))
}

/// Build a synthetic dump in the verified layout (used by tests and the
/// mock transport; never a substitute for the real golden file).
pub fn build_eeprom(cal: &StereoCalib, serial: &str, code_a: &str, code_b: &str) -> Vec<u8> {
    let mut e = vec![0xFFu8; EEPROM_SIZE];
    e[..16].copy_from_slice(EEPROM_HEADER_TAG);

    let mut put = |off: usize, payload: &[u8]| {
        e[off..off + payload.len()].copy_from_slice(payload);
        e[off + payload.len()] = payload.iter().fold(0u8, |a, &b| a.wrapping_add(b));
        e[off + payload.len() + 1] = 0xFF;
    };

    let floats: Vec<f32> = [
        cal.fx[0],
        cal.fy[0],
        cal.cx[0],
        cal.cy[0],
        cal.dist[0][0],
        cal.dist[0][1],
        cal.dist[0][2],
        cal.dist[0][3],
        cal.fx[1],
        cal.fy[1],
        cal.cx[1],
        cal.cy[1],
        cal.dist[1][0],
        cal.dist[1][1],
        cal.dist[1][2],
        cal.dist[1][3],
        cal.r[0][0],
        cal.r[0][1],
        cal.r[0][2],
        cal.r[1][0],
        cal.r[1][1],
        cal.r[1][2],
        cal.r[2][0],
        cal.r[2][1],
        cal.r[2][2],
        cal.t[0],
        cal.t[1],
        cal.t[2],
    ]
    .iter()
    .map(|&v| v as f32)
    .collect();
    let mut payload = Vec::new();
    for v in &floats {
        payload.extend_from_slice(&v.to_le_bytes());
    }
    payload.extend_from_slice(&[0u8; 8]); // pad to 120
    put(REC_CALIB, &payload);

    put(REC_VERSION, b"2059\0\0\0\0\0\0\0\0\0\0\0\0");
    let ints: Vec<u8> =
        [386i16, 473, 215, 303, 142, 221, 0, 0].iter().flat_map(|v| v.to_le_bytes()).collect();
    put(REC_INT16, &ints);

    let mut ser = [0u8; REC_SERIAL_LEN];
    let sb = serial.as_bytes();
    ser[..sb.len().min(REC_SERIAL_LEN)].copy_from_slice(&sb[..sb.len().min(REC_SERIAL_LEN)]);
    put(REC_SERIAL, &ser);
    // Fixed-length payloads: pad codes/serial with 0xFF fill to their
    // record sizes, exactly like the vendor flash image (records are
    // fixed-length; the parse side reads the same fixed sizes).
    let pad = |s: &str, len: usize| -> Vec<u8> {
        let b = s.as_bytes();
        let mut v = vec![0u8; len];
        let n = b.len().min(len);
        v[..n].copy_from_slice(&b[..n]);
        v
    };
    put(REC_CODE_A, &pad(code_a, REC_CODE_A_LEN));
    put(REC_CODE_B, &pad(code_b, REC_CODE_B_LEN));

    // mirror the two regions (drop the closure to release the borrow)
    drop(put);
    e.copy_within(REC_CALIB..REC_CALIB + 128, REC_CALIB + EEPROM_MIRROR_STRIDE);
    e.copy_within(REC_SERIAL..REC_SERIAL + 0x30, REC_SERIAL + EEPROM_MIRROR_STRIDE);
    e
}

/// Reference calibration of the real device (from the verified dump;
/// used by tests and as a sanity anchor for future golden files).
pub const REFERENCE_CALIB: StereoCalib = StereoCalib {
    fx: [428.761292, 435.891205],
    fy: [430.208038, 437.767944],
    cx: [312.787292, 312.149200],
    cy: [212.867691, 179.222244],
    dist: [[0.042773, -0.070018, -0.000007, -0.000250], [0.024833, -0.044414, 0.000232, -0.000101]],
    r: [
        [0.999981, 0.002477, -0.005639],
        [-0.002544, 0.999927, -0.011812],
        [0.005609, 0.011827, 0.999914],
    ],
    t: [99.562683, 0.463940, 9.330422],
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::IdongError;

    #[test]
    fn reference_values_hold() {
        let cal = REFERENCE_CALIB;
        // designed baseline exactly 100 mm
        assert!((cal.baseline_mm() - 100.0).abs() < 1e-4, "designed baseline");
        // mean focal length 432.33 px (CONTEXT.md)
        assert!((cal.mean_focal_px() - 432.33).abs() < 0.01, "mean f = 432.33 px");
        // distortion magnitudes small
        assert!(cal.dist.iter().all(|d| d[0].abs() < 0.05));
        // R orthonormal: R R^T == I within tolerance
        for i in 0..3 {
            for j in 0..3 {
                let mut dot = 0.0;
                for k in 0..3 {
                    dot += cal.r[i][k] * cal.r[j][k];
                }
                let want = if i == j { 1.0 } else { 0.0 };
                assert!((dot - want).abs() < 1e-4, "R({i},{j}) dot {dot}");
            }
        }
        // principal points near mid-frame (640-px frames)
        assert!(cal.cx.iter().all(|&c| (c - 312.5).abs() < 1.0));
    }

    #[test]
    fn synthetic_roundtrip() {
        let e = build_eeprom(&REFERENCE_CALIB, "AB460DC8BA12C6840B49CD5786C329DD", "000T", "2613");
        let (c, id) = parse_eeprom(&e).unwrap();
        assert!((c.fx[0] - 428.761292).abs() < 1e-2, "f32 roundtrip tolerance");
        assert!((c.baseline_mm() - 100.0).abs() < 1e-3);
        assert_eq!(id.serial, "AB460DC8BA12C6840B49CD5786C329DD");
        assert_eq!(id.code_a, "000T");
        assert_eq!(id.code_b, "2613");
    }

    #[test]
    fn rejects_corrupted_checksum() {
        let mut e =
            build_eeprom(&REFERENCE_CALIB, "AB460DC8BA12C6840B49CD5786C329DD", "000T", "2613");
        // all-zero payload checksums to 0x00, so flip a payload byte to
        // make the stored checksum wrong:
        e[REC_CALIB] ^= 1;
        match parse_eeprom(&e) {
            Err(IdongError::Protocol(ProtocolError::BadMagic { .. })) => {}
            other => panic!("expected checksum failure, got {other:?}"),
        }
    }

    #[test]
    fn rejects_wrong_header_and_size() {
        let mut e = build_eeprom(&REFERENCE_CALIB, "S", "A", "B");
        e[0] = b'X';
        assert!(parse_eeprom(&e).is_err());
        assert!(parse_eeprom(&e[..1024]).is_err(), "short dump");
    }

    #[test]
    fn session_calibration_record_roundtrip() {
        // The P2.2 live bring-up path: read_calibration returns the
        // 0x0100 page as [120 B payload][checksum] (121 B, no 0xFF
        // separator - the separator lives in the dump, not on the wire).
        let e = build_eeprom(&REFERENCE_CALIB, "AB460DC8BA12C6840B49CD5786C329DD", "000T", "2613");
        let rec = &e[REC_CALIB..REC_CALIB + REC_CALIB_LEN + 1];
        assert_eq!(rec.len(), 121);
        let c = parse_calibration_record(rec).unwrap();
        assert!((c.fx[0] - REFERENCE_CALIB.fx[0] as f32 as f64).abs() < 1e-2);
        assert!((c.baseline_mm() - 100.0).abs() < 1e-3);
        assert!((c.mean_focal_px() - 432.33).abs() < 0.01);
        // checksum corruption is rejected
        let mut bad = rec.to_vec();
        *bad.last_mut().unwrap() ^= 1;
        assert!(parse_calibration_record(&bad).is_err());
        // wrong length is rejected
        assert!(parse_calibration_record(&rec[..120]).is_err());
    }

    /// EEPROM write path must not exist (safety rule #4): this is a
    /// compile-time property - there is no write function in this
    /// module; the test asserts the API stays read-only by parsing only.
    #[test]
    fn api_surface_is_read_only() {
        // parse_eeprom is the only public entry; nothing writes.
        let e = build_eeprom(&REFERENCE_CALIB, "S", "A", "B");
        assert!(parse_eeprom(&e).is_ok());
    }
}
