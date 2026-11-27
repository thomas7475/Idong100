//! Public data model for marker frames.
//!
//! Semantics follow the vendor stack: a record per 50 fps tick, up to
//! 10 tracked slots (chip documents 15 on idong100 / 20+ on idong200),
//! per-slot quality with a >= 50 tracking threshold, and a link-quality
//! classification driven by the disturbance counters (xEyeUsb.ini
//! thresholds Green=20 / Yellow=35 / Red=35).

use super::protocol::{QUALITY_GREEN, QUALITY_RED, QUALITY_YELLOW};

/// One tracked reflective marker in the device coordinate frame.
///
/// Coordinate system: X right, Y up, Z along the optical axis away from
/// the sensor (right-handed, device frame; orientation confirmed turn
/// 46: the vendor `GetADataFromUSB` best-marker loop selects the
/// smallest z - the NEAREST marker is the active one - and stores the
/// winning slot's x/y/z qwords as the manager's marker position).
///
/// The values are millimetres. NOTE (turn 46): these coordinates are
/// HOST-computed in the vendor stack (`classifying.dll!ClassifyAndTrack`
/// classifies the 640-B wire record of EP 0x83); the device itself
/// sends raw blob data ([`crate::protocol::ens::WireRecord`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Marker {
    /// Slot index inside the record (0..=9 observed; chip: up to 15).
    pub slot: u8,
    /// Lateral position, millimetres.
    pub x_mm: f32,
    /// Vertical position, millimetres.
    pub y_mm: f32,
    /// Depth from the sensor bar, millimetres (the nearest-marker
    /// selection key: smallest z wins).
    pub z_mm: f32,
    /// Vendor quality value (slot+0x24); >= 50 = tracked.
    pub quality: i32,
}

/// Link-quality classification (vendor traffic-light semantics).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Green,
    Yellow,
    Red,
}

impl Quality {
    /// Classify from the vendor disturbance counters (placeholder
    /// mapping on tracked-marker count until the counter payload layout
    /// is pinned by open item #3).
    pub fn from_count(tracked: usize) -> Quality {
        match tracked as i32 {
            t if t >= QUALITY_YELLOW => Quality::Green,
            t if t >= QUALITY_GREEN => Quality::Yellow,
            _ => Quality::Red,
        }
    }

    pub fn from_disturbance(d: i32) -> Quality {
        if d >= QUALITY_RED {
            Quality::Red
        } else if d >= QUALITY_YELLOW {
            Quality::Yellow
        } else {
            Quality::Green
        }
    }

    /// Traffic-light ordinal for diagnostics topics (0/1/2).
    pub fn level(self) -> u8 {
        match self {
            Quality::Green => 0,
            Quality::Yellow => 1,
            Quality::Red => 2,
        }
    }
}

/// One decoded marker record (50 fps).
#[derive(Debug, Clone, PartialEq)]
pub struct MarkerFrame {
    /// Record +0x00: vendor frame counter (continuity-checked).
    pub frame_counter: u32,
    /// Record +0x3B8: vendor mode flag (1 = skipped by the pipe writer).
    pub mode: u32,
    /// Slots that passed the >= 50 quality filter.
    pub markers: Vec<Marker>,
    /// Traffic-light link quality for this frame.
    pub link_quality: Quality,
    /// True when the frame counter jumped (vendor drop check).
    pub dropped: bool,
}

impl MarkerFrame {
    /// Number of tracked markers.
    pub fn tracked(&self) -> usize {
        self.markers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_levels_order() {
        // from_count placeholder mapping: >= 35 green, >= 20 yellow,
        // else red (counts below 20 -> red; thresholds really apply to
        // disturbance counters accumulated over RingSize frames)
        assert_eq!(Quality::from_count(36), Quality::Green);
        assert_eq!(Quality::from_count(21), Quality::Yellow);
        assert_eq!(Quality::from_count(4), Quality::Red, "4 < 20 -> red");
        // from_disturbance thresholds: >= 35 red, >= 20 yellow, else green
        assert_eq!(Quality::from_disturbance(QUALITY_RED), Quality::Red);
        assert_eq!(Quality::from_disturbance(0), Quality::Green);
    }
}
