//! xEye3Ddata record parser (xEyeClass.dll, GetADataFromUSB @RVA 0x20f0).
//!
//! Layout (verified structurally, field semantics partially open):
//! 0x3BC B minimum: frame counter u32 @ +0 (overlays slot 0's first u32);
//! 10 slots x 88 B at +0: gate f64 @ slot+0x18, quality i32 @ slot+0x24
//! (>= 50 tracked); 72 B trailer; mode flag u32 @ +0x3B8 (==1 skip).
//! Pipe form: 0x208 B after the RVA 0x25e0 transform.
//!
//! The 88-byte slots certainly contain the marker 3D position; the exact
//! x/y/z offsets are **open item #4** (resolution: disassembly of the
//! record transform at RVA 0x25e0 + `iDongTTCamera.dll`). Until pinned,
//! valid slots surface as tracked markers with placeholder coordinates
//! read from the first three f32-sized words (slot+0x00..0x0C) - the
//! region the transform feeds from - and the raw bytes are kept for
//! offline analysis.

use super::{MARKER_RECORD_MIN, MARKER_SLOTS, MARKER_SLOT_SIZE, MARKER_TRAILER, QUALITY_GATE};
use crate::marker::{Marker, MarkerFrame, Quality};

pub const SLOT_LEN: usize = MARKER_SLOT_SIZE; // 0x58
pub const SLOTS: usize = MARKER_SLOTS; // 10
/// Slots occupy +0x000..+0x370 (10 x 88 = 880).
pub const SLOTS_END: usize = SLOTS * SLOT_LEN; // 0x370
/// Mode flag u32 at +0x3B8.
pub const MODE_OFFSET: usize = SLOTS_END + MARKER_TRAILER; // 0x3B8
/// Minimum record size (mode flag is a u32).
pub const RECORD_MIN: usize = MARKER_RECORD_MIN; // 0x3BC

/// One raw 88-byte slot with the fields we can already decode.
#[derive(Debug, Clone)]
pub struct RawSlot {
    /// Full slot bytes for offline analysis (open item #4).
    pub bytes: [u8; SLOT_LEN],
    /// slot+0x18 (timestamp or distance gate; open item #4).
    pub gate: f64,
    /// slot+0x24 (vendor quality/age; >= 50 = tracked).
    pub quality: i32,
}

impl RawSlot {
    fn parse(bytes: &[u8]) -> Option<RawSlot> {
        if bytes.len() < SLOT_LEN {
            return None;
        }
        let mut b = [0u8; SLOT_LEN];
        b.copy_from_slice(&bytes[..SLOT_LEN]);
        let gate = f64::from_le_bytes(bytes[0x18..0x20].try_into().ok()?);
        let quality = i32::from_le_bytes(bytes[0x24..0x28].try_into().ok()?);
        Some(RawSlot { bytes: b, gate, quality })
    }
}

/// Parse a device-to-host marker record payload.
///
/// `prev_counter` enables the vendor's drop check: a counter delta > 1
/// marks the frame as dropped (the lost-frame path in GetADataFromUSB).
pub fn parse_record(payload: &[u8], prev_counter: Option<u32>) -> Option<MarkerFrame> {
    if payload.len() < RECORD_MIN {
        return None;
    }
    let frame_counter = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let mode = u32::from_le_bytes([
        payload[MODE_OFFSET],
        payload[MODE_OFFSET + 1],
        payload[MODE_OFFSET + 2],
        payload[MODE_OFFSET + 3],
    ]);

    let mut markers = Vec::with_capacity(SLOTS);
    for (i, slot) in payload[..SLOTS_END].chunks(SLOT_LEN).take(SLOTS).enumerate() {
        let raw = RawSlot::parse(slot)?;
        if raw.quality >= QUALITY_GATE {
            // Open item #4: x/y/z offsets inside the slot. Until pinned,
            // placeholder coordinates come from slot+0x00..0x0C.
            markers.push(Marker {
                slot: i as u8,
                x_mm: f32::from_le_bytes(slot[0x00..0x04].try_into().ok()?),
                y_mm: f32::from_le_bytes(slot[0x04..0x08].try_into().ok()?),
                z_mm: f32::from_le_bytes(slot[0x08..0x0C].try_into().ok()?),
                quality: raw.quality,
                age: raw.gate,
            });
        }
    }

    let dropped = match prev_counter {
        Some(p) => frame_counter.wrapping_sub(p) > 1,
        None => false,
    };
    let link_quality = Quality::from_count(markers.len());

    Some(MarkerFrame { frame_counter, mode, markers, link_quality, dropped })
}

/// Raw slots of a record (for offline analysis of open item #4).
pub fn raw_slots(payload: &[u8]) -> Option<Vec<RawSlot>> {
    if payload.len() < RECORD_MIN {
        return None;
    }
    payload[..SLOTS_END].chunks(SLOT_LEN).take(SLOTS).map(RawSlot::parse).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_with_slots(counter: u32, qualities: &[i32]) -> Vec<u8> {
        let mut v = vec![0u8; RECORD_MIN];
        v[0..4].copy_from_slice(&counter.to_le_bytes()); // overlays slot 0
        v[MODE_OFFSET..MODE_OFFSET + 4].copy_from_slice(&0u32.to_le_bytes());
        for (i, &q) in qualities.iter().enumerate().take(SLOTS) {
            let off = i * SLOT_LEN + 0x24;
            v[off..off + 4].copy_from_slice(&q.to_le_bytes());
            let gate = i * SLOT_LEN + 0x18;
            v[gate..gate + 8].copy_from_slice(&(i as f64).to_le_bytes());
        }
        v
    }

    #[test]
    fn sizes_match_disassembly() {
        assert_eq!(SLOT_LEN, 0x58);
        assert_eq!(SLOTS_END, 0x370);
        assert_eq!(MARKER_TRAILER, 0x48);
        assert_eq!(MODE_OFFSET, 0x3B8);
        assert_eq!(RECORD_MIN, 0x3BC);
        // mode sits exactly 72 bytes past the last slot - the layout the
        // GetADataFromUSB addresses imply (slots at +0, no header).
        assert_eq!(MODE_OFFSET, SLOTS * SLOT_LEN + MARKER_TRAILER);
    }

    #[test]
    fn quality_filter_and_drop_detection() {
        let r1 = record_with_slots(100, &[80, 80, 10, 80]);
        let f1 = parse_record(&r1, None).unwrap();
        assert_eq!(f1.frame_counter, 100);
        assert_eq!(f1.markers.len(), 3, "the quality-10 slot is filtered");
        assert_eq!(f1.markers[0].quality, 80);
        assert_eq!(f1.markers[0].slot, 0);
        assert!((f1.markers[0].age - 0.0).abs() < f64::EPSILON);

        let r2 = record_with_slots(103, &[80, 80, 80, 80]);
        let f2 = parse_record(&r2, Some(100)).unwrap();
        assert!(f2.dropped, "counter jumped 100 -> 103");
    }

    #[test]
    fn mode_flag_read() {
        let mut r = record_with_slots(7, &[80]);
        r[MODE_OFFSET..MODE_OFFSET + 4].copy_from_slice(&1u32.to_le_bytes());
        let f = parse_record(&r, None).unwrap();
        assert_eq!(f.mode, 1, "pipe-skip flag");
    }

    #[test]
    fn short_payload_rejected_not_panicking() {
        assert!(parse_record(&[0u8; 0x100], None).is_none());
        assert!(parse_record(&[], Some(5)).is_none());
        assert!(raw_slots(&[0u8; 10]).is_none());
    }

    #[test]
    fn raw_slots_expose_every_slot() {
        let r = record_with_slots(1, &[80, 10, 80]);
        let slots = raw_slots(&r).unwrap();
        assert_eq!(slots.len(), SLOTS);
        assert_eq!(slots[1].quality, 10);
        assert_eq!(slots[0].bytes.len(), SLOT_LEN);
    }
}
