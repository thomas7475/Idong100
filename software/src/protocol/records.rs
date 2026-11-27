//! xEye3Ddata record parser (xEyeClass.dll, GetADataFromUSB @RVA 0x20f0).
//!
//! Layout (turn 46: every offset now byte-pinned by disassembly - open
//! item #4 RESOLVED):
//! 0x3BC B minimum: frame counter u32 @ +0 (overlays slot 0's first u32;
//! the usbCom worker assigns it host-side, one per record); 10 slots x
//! 88 B at +0 with **x f64 @ slot+0x08, y f64 @ slot+0x10, z f64 @
//! slot+0x18, quality i32 @ slot+0x24 (>= 50 tracked)** - pinned by
//! `TransUSB2Pipe` @RVA 0x25e0 (the slot packer) and the best-marker
//! loop of `GetADataFromUSB` (smallest z wins - the NEAREST marker is
//! the active one; its slot+0x08/0x10/0x18 qwords become the manager's
//! x/y/z floats @ +0x770/+0x774/+0x778). Pipe form: 0x208 B.
//!
//! IMPORTANT (turn 46 finding): this record is a HOST-side structure.
//! On the wire the device sends 640-B raw records (blob data for the
//! host classifier `classifying.dll!ClassifyAndTrack`) on EP 0x83 - see
//! [`super::ens`]. The x/y/z values live in the record only after the
//! vendor host stack classified it. Our parser covers the record model
//! (for replay/analysis and a future host-side classifier), the wire
//! plane lives in `ens`.

use super::{MARKER_RECORD_MIN, MARKER_SLOTS, MARKER_SLOT_SIZE, MARKER_TRAILER, QUALITY_GATE};
use crate::marker::{Marker, MarkerFrame, Quality};

pub const SLOT_LEN: usize = MARKER_SLOT_SIZE; // 0x58
pub const SLOTS: usize = MARKER_SLOTS; // 10
/// Slots occupy +0x000..+0x370 (10 x 88 = 880).
pub const SLOTS_END: usize = SLOTS * SLOT_LEN; // 0x370
/// Mode flag u32 at +0x3B8.
pub const MODE_OFFSET: usize = SLOTS_END + MARKER_TRAILER; // 0x3B8
/// Minimum record size (mode flag is a u32). The full host-side queue
/// element is 0x3F0 B (`TransUSB2Pipe` reads up to +0x3C8; the
/// "Data2Class" queue carries 0x3F0-B elements).
pub const RECORD_MIN: usize = MARKER_RECORD_MIN; // 0x3BC
/// Slot x f64 offset (also y @ +0x10, z @ +0x18 - TransUSB2Pipe).
pub const SLOT_XYZ_OFF: usize = 0x08;
/// Slot quality i32 offset.
pub const SLOT_QUALITY_OFF: usize = 0x24;

/// One raw 88-byte slot with the fields we decode.
#[derive(Debug, Clone)]
pub struct RawSlot {
    /// Full slot bytes for offline analysis.
    pub bytes: [u8; SLOT_LEN],
    /// slot+0x08 (marker X, millimetres - TransUSB2Pipe @0x2627).
    pub x_mm: f64,
    /// slot+0x10 (marker Y, millimetres - TransUSB2Pipe @0x2641).
    pub y_mm: f64,
    /// slot+0x18 (marker Z, millimetres - the nearest-marker selection
    /// key of GetADataFromUSB: smallest z wins).
    pub z_mm: f64,
    /// slot+0x24 (vendor quality; >= 50 = tracked).
    pub quality: i32,
}

impl RawSlot {
    fn parse(bytes: &[u8]) -> Option<RawSlot> {
        if bytes.len() < SLOT_LEN {
            return None;
        }
        let mut b = [0u8; SLOT_LEN];
        b.copy_from_slice(&bytes[..SLOT_LEN]);
        let x_mm = f64::from_le_bytes(bytes[0x08..0x10].try_into().ok()?);
        let y_mm = f64::from_le_bytes(bytes[0x10..0x18].try_into().ok()?);
        let z_mm = f64::from_le_bytes(bytes[0x18..0x20].try_into().ok()?);
        let quality = i32::from_le_bytes(bytes[0x24..0x28].try_into().ok()?);
        Some(RawSlot { bytes: b, x_mm, y_mm, z_mm, quality })
    }
}

/// Parse a host-side marker record payload (the xEye3Ddata).
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
            markers.push(Marker {
                slot: i as u8,
                x_mm: raw.x_mm as f32,
                y_mm: raw.y_mm as f32,
                z_mm: raw.z_mm as f32,
                quality: raw.quality,
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

/// Nearest-marker selection exactly like `GetADataFromUSB` @0x22a8: the
/// tracked slot (quality >= 50) with the smallest z; `None` when no
/// slot is tracked (the vendor fills the "invalid" constant there).
pub fn nearest_marker(frame: &MarkerFrame) -> Option<&Marker> {
    frame.markers.iter().min_by(|a, b| a.z_mm.total_cmp(&b.z_mm))
}

/// Raw slots of a record (for offline analysis).
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
            let off = i * SLOT_LEN;
            // real x/y/z decode fields (TransUSB2Pipe offsets)
            v[off + 0x08..off + 0x10].copy_from_slice(&((i as f64) * 10.0).to_le_bytes());
            v[off + 0x10..off + 0x18].copy_from_slice(&(100.0 + i as f64).to_le_bytes());
            v[off + 0x18..off + 0x20].copy_from_slice(&(500.0 - 10.0 * i as f64).to_le_bytes());
            v[off + 0x24..off + 0x28].copy_from_slice(&q.to_le_bytes());
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
        // open item #4 resolution: x/y/z/quality offsets pinned by
        // TransUSB2Pipe @RVA 0x25e0 (packer) + GetADataFromUSB @0x22a8
        // (selection): x +0x08, y +0x10, z +0x18, quality +0x24.
        assert_eq!(SLOT_XYZ_OFF, 0x08);
        assert_eq!(SLOT_QUALITY_OFF, 0x24);
    }

    #[test]
    fn xyz_decoded_from_the_pinned_offsets() {
        let r = record_with_slots(100, &[80, 80, 10, 80]);
        let f1 = parse_record(&r, None).unwrap();
        assert_eq!(f1.frame_counter, 100);
        assert_eq!(f1.markers.len(), 3, "the quality-10 slot is filtered");
        let m0 = &f1.markers[0];
        assert_eq!(m0.slot, 0);
        assert_eq!(m0.quality, 80);
        assert!((m0.x_mm - 0.0).abs() < f32::EPSILON, "slot 0 x = 0");
        assert!((m0.y_mm - 100.0).abs() < 1e-3, "slot 0 y = 100");
        assert!((m0.z_mm - 500.0).abs() < 1e-3, "slot 0 z = 500");
        let m1 = &f1.markers[1];
        assert!((m1.x_mm - 10.0).abs() < 1e-3);
        assert!((m1.y_mm - 101.0).abs() < 1e-3);
        assert!((m1.z_mm - 490.0).abs() < 1e-3);
    }

    #[test]
    fn nearest_marker_is_the_smallest_z() {
        // GetADataFromUSB selects the tracked slot with the SMALLEST
        // slot+0x18 (z): slots with z 500/490/480 -> slot 2 wins
        let r = record_with_slots(1, &[80, 80, 80]);
        let f = parse_record(&r, None).unwrap();
        let n = nearest_marker(&f).unwrap();
        assert_eq!(n.slot, 2, "slot 2 has z=480, the smallest");
        assert!((n.z_mm - 480.0).abs() < 1e-3);

        // no tracked slot -> no nearest (the vendor "no marker" branch)
        let r2 = record_with_slots(2, &[10, 20]);
        let f2 = parse_record(&r2, None).unwrap();
        assert!(f2.markers.is_empty());
        assert!(nearest_marker(&f2).is_none());
    }

    #[test]
    fn quality_filter_and_drop_detection() {
        let r1 = record_with_slots(100, &[80, 80, 10, 80]);
        let f1 = parse_record(&r1, None).unwrap();
        assert_eq!(f1.markers.len(), 3, "the quality-10 slot is filtered");
        assert_eq!(f1.markers[0].quality, 80);
        assert_eq!(f1.markers[0].slot, 0);

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
        assert!((slots[2].z_mm - 480.0).abs() < 1e-3);
    }
}
