//! Topics + QoS + the message shapes (draft 5.1 / table 4, P4 turn 48).
//!
//! The QoS split follows the sensor-data guidance: the marker stream
//! is best-effort keep-last 5 (a late frame is worthless), while
//! handle button events are reliable keep-last 10 (a lost click is a
//! lost user action). Diagnostics and time_ref are reliable.
//!
//! The structs below mirror the `.msg`/`.srv` files in
//! `software/idong_msgs` field-for-field; the rclrs glue copies them
//! into the generated types - the only place message types leak into
//! our code. Headers (stamp + frame_id) are stamped by the glue with
//! receive time (the device gives a frame counter, not a clock), so
//! the pure structs carry the payload fields plus the frame_id.

use super::params::TrackingConfig;
use crate::handle::HandleEvent;
use crate::marker::{Marker, MarkerFrame};

/// Publisher reliability for one topic (ROS 2 QoS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reliability {
    Reliable,
    BestEffort,
}

/// One interface of the node surface (draft table 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interface {
    /// Topic or service name.
    pub name: &'static str,
    /// Type string (idong_msgs/... or the standard types).
    pub ty: &'static str,
    /// Publisher reliability.
    pub reliability: Reliability,
    /// Keep-last history depth.
    pub depth: u8,
}

impl Interface {
    /// The main data topic: best-effort keep-last 5.
    pub const MARKERS: Interface = Interface {
        name: "/idong/markers",
        ty: "idong_msgs/MarkerArray",
        reliability: Reliability::BestEffort,
        depth: 5,
    };
    /// Handle events: reliable keep-last 10.
    pub const EVENTS: Interface = Interface {
        name: "/idong/events",
        ty: "idong_msgs/HandleEvent",
        reliability: Reliability::Reliable,
        depth: 10,
    };
    /// Diagnostics: reliable.
    pub const DIAGNOSTICS: Interface = Interface {
        name: "/idong/diagnostics",
        ty: "diagnostic_msgs/DiagnosticArray",
        reliability: Reliability::Reliable,
        depth: 10,
    };
    /// Device frame counter -> host clock pairs: reliable.
    pub const TIME_REF: Interface = Interface {
        name: "/idong/time_ref",
        ty: "sensor_msgs/TimeReference",
        reliability: Reliability::Reliable,
        depth: 10,
    };
    /// The device identity service.
    pub const DEVICE_INFO: Interface = Interface {
        name: "get_device_info",
        ty: "idong_msgs/DeviceInfo",
        reliability: Reliability::Reliable,
        depth: 1,
    };
    /// The lifecycle transition service (draft 5.3).
    pub const TRANSITION: Interface = Interface {
        name: "~/transition",
        ty: "lifecycle_msgs/ChangeState",
        reliability: Reliability::Reliable,
        depth: 1,
    };
}

/// The whole node surface in one table (draft table 4; the glue
/// creates exactly these and nothing else).
pub const INTERFACES: [Interface; 6] = [
    Interface::MARKERS,
    Interface::EVENTS,
    Interface::DIAGNOSTICS,
    Interface::TIME_REF,
    Interface::DEVICE_INFO,
    Interface::TRANSITION,
];

/// idong_msgs/Marker (field-for-field): one tracked marker.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkerMsg {
    /// Device frame counter (continuity: delta <= 1).
    pub seq: u32,
    /// Slot index (0..=9 observed; chip documents up to 15).
    pub slot: u8,
    /// mm, device frame.
    pub x: f32,
    pub y: f32,
    /// Depth from the sensor bar - the nearest-marker selection key
    /// (smallest z wins, `GetADataFromUSB`).
    pub z: f32,
    /// Vendor quality (slot+0x24); >= 50 means tracked.
    pub quality: i32,
}

impl MarkerMsg {
    /// Convert one core marker of a frame (the frame supplies seq).
    pub fn from_core(seq: u32, m: &Marker) -> MarkerMsg {
        MarkerMsg { seq, slot: m.slot, x: m.x_mm, y: m.y_mm, z: m.z_mm, quality: m.quality }
    }
}

/// idong_msgs/MarkerArray (field-for-field): one 50 fps frame.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkerArrayMsg {
    /// `header.frame_id` (stamped by the glue with receive time).
    pub frame_id: String,
    /// Device frame counter.
    pub seq: u32,
    /// The tracked markers, slot order.
    pub markers: Vec<MarkerMsg>,
    /// 0 green / 1 yellow / 2 red (vendor thresholds).
    pub link_quality: u8,
    /// Record +0x3B8 mode flag.
    pub mode: u32,
    /// Frame-counter gap detected.
    pub dropped: bool,
}

impl MarkerArrayMsg {
    /// Convert one core [`MarkerFrame`], applying the configured
    /// quality gate (the draft's live-reconfigurable threshold; the
    /// core already filters at the vendor's own gate 50 - a user
    /// threshold ABOVE that only narrows further, below it cannot
    /// resurrect slots the core never emitted).
    pub fn from_frame(f: &MarkerFrame, cfg: &TrackingConfig) -> MarkerArrayMsg {
        MarkerArrayMsg {
            frame_id: cfg.frame_id.clone(),
            seq: f.frame_counter,
            markers: f
                .markers
                .iter()
                .filter(|m| m.quality >= cfg.quality_threshold)
                .map(|m| MarkerMsg::from_core(f.frame_counter, m))
                .collect(),
            link_quality: f.link_quality.level(),
            mode: f.mode,
            dropped: f.dropped,
        }
    }
}

/// idong_msgs/HandleEvent (field-for-field). The event codes are the
/// vendor ack sequence (TSOL_DEVICE_HANDLE_*); `handle_id` is 0 until
/// open item #6 (the handle-config blob layout) pins the pairing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandleEventMsg {
    pub seq: u32,
    /// 0 FOUND, 1 NOT_FOUND, 2 TIMEOUT, 3 FOUND_OVER,
    /// 4 BUTTON_DOWN, 5 BUTTON_UP.
    pub event: u8,
    pub handle_id: u8,
    /// Bitmask when event is BUTTON_*.
    pub button: u8,
}

impl HandleEventMsg {
    /// Convert one core event (event codes pinned by test).
    pub fn from_event(seq: u32, e: &HandleEvent) -> HandleEventMsg {
        let (event, button) = match e {
            HandleEvent::Found => (0u8, 0u8),
            HandleEvent::NotFound => (1, 0),
            HandleEvent::Timeout => (2, 0),
            HandleEvent::FoundOver => (3, 0),
            HandleEvent::ButtonDown { button } => (4, *button),
            HandleEvent::ButtonUp { button } => (5, *button),
        };
        HandleEventMsg { seq, event, handle_id: 0, button }
    }
}

/// idong_msgs/DeviceInfo response (field-for-field).
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceInfoMsg {
    /// 32-char device serial (EEPROM).
    pub serial: String,
    /// Device-ID exchange string at boot.
    pub dsp_version: String,
    /// sha256 prefix of the pinned firmware set.
    pub firmware_hash: String,
    /// EEPROM T magnitude (100.00 expected).
    pub baseline_mm: f32,
    /// Mean EEPROM focal length (432.33 expected).
    pub focal_px: f32,
}

impl DeviceInfoMsg {
    /// Build from the session's identity facts.
    pub fn new(
        serial: impl Into<String>,
        dsp_version: impl Into<String>,
        firmware_hash: impl Into<String>,
        baseline_mm: f32,
        focal_px: f32,
    ) -> DeviceInfoMsg {
        DeviceInfoMsg {
            serial: serial.into(),
            dsp_version: dsp_version.into(),
            firmware_hash: firmware_hash.into(),
            baseline_mm,
            focal_px,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marker::Quality;
    use crate::protocol::records;

    fn frame_with(counter: u32, dropped: bool) -> MarkerFrame {
        let mut f = records::parse_record(&record_bytes(counter), Some(counter.wrapping_sub(2)))
            .expect("record parses");
        f.dropped = dropped;
        f
    }

    /// A 0x3BC record with two tracked slots (0 and 2) and one below
    /// the gate (1).
    fn record_bytes(counter: u32) -> Vec<u8> {
        use crate::protocol::records::{MODE_OFFSET, SLOT_LEN};
        let mut b = vec![0u8; crate::protocol::records::RECORD_MIN];
        b[0..4].copy_from_slice(&counter.to_le_bytes());
        b[MODE_OFFSET..MODE_OFFSET + 4].copy_from_slice(&7u32.to_le_bytes());
        // slot 0: tracked, z = 1500 mm (nearest)
        b[0x08..0x10].copy_from_slice(&10.0f64.to_le_bytes());
        b[0x10..0x18].copy_from_slice(&20.0f64.to_le_bytes());
        b[0x18..0x20].copy_from_slice(&1500.0f64.to_le_bytes());
        b[0x24..0x28].copy_from_slice(&90i32.to_le_bytes());
        // slot 1: below the vendor gate - filtered by the core parser
        let s1 = SLOT_LEN;
        b[s1 + 0x24..s1 + 0x28].copy_from_slice(&10i32.to_le_bytes());
        // slot 2: tracked, z = 2000 mm
        let s2 = 2 * SLOT_LEN;
        b[s2 + 0x08..s2 + 0x10].copy_from_slice(&(-5.0f64).to_le_bytes());
        b[s2 + 0x10..s2 + 0x18].copy_from_slice(&6.0f64.to_le_bytes());
        b[s2 + 0x18..s2 + 0x20].copy_from_slice(&2000.0f64.to_le_bytes());
        b[s2 + 0x24..s2 + 0x28].copy_from_slice(&60i32.to_le_bytes());
        b
    }

    #[test]
    fn the_interface_surface_is_the_draft_table() {
        // names, types, QoS - exactly the draft table 4, no more.
        assert_eq!(Interface::MARKERS.name, "/idong/markers");
        assert_eq!(Interface::MARKERS.reliability, Reliability::BestEffort);
        assert_eq!(Interface::MARKERS.depth, 5);
        assert_eq!(Interface::EVENTS.name, "/idong/events");
        assert_eq!(Interface::EVENTS.reliability, Reliability::Reliable);
        assert_eq!(Interface::EVENTS.depth, 10);
        assert_eq!(Interface::DIAGNOSTICS.name, "/idong/diagnostics");
        assert_eq!(Interface::DIAGNOSTICS.reliability, Reliability::Reliable);
        assert_eq!(Interface::TIME_REF.name, "/idong/time_ref");
        assert_eq!(Interface::DEVICE_INFO.name, "get_device_info");
        assert_eq!(Interface::TRANSITION.name, "~/transition");
        assert_eq!(INTERFACES.len(), 6);
    }

    #[test]
    fn marker_array_conversion_applies_the_gate() {
        let cfg = TrackingConfig {
            frame_id: "idong".into(),
            quality_threshold: 50,
            link_quality_window: 200,
            enable_handle_events: true,
        };
        let f = frame_with(41, true);
        let m = MarkerArrayMsg::from_frame(&f, &cfg);
        assert_eq!(m.seq, 41);
        assert_eq!(m.frame_id, "idong");
        assert_eq!(m.mode, 7);
        assert!(m.dropped);
        assert_eq!(m.markers.len(), 2); // slot 1 was below the gate
        assert_eq!(m.markers[0].slot, 0);
        assert!((m.markers[0].x - 10.0).abs() < 1e-4);
        assert!((m.markers[0].z - 1500.0).abs() < 1e-4);
        assert_eq!(m.markers[0].quality, 90);
        assert_eq!(m.markers[1].slot, 2);
        assert_eq!(m.markers[1].seq, 41); // every marker carries the frame's seq
                                          // a stricter threshold narrows further (live reconfigure)
        let stricter = TrackingConfig { quality_threshold: 80, ..cfg.clone() };
        assert_eq!(MarkerArrayMsg::from_frame(&f, &stricter).markers.len(), 1);
    }

    #[test]
    fn handle_event_codes_match_the_msg_definition() {
        // 0 FOUND, 1 NOT_FOUND, 2 TIMEOUT, 3 FOUND_OVER,
        // 4 BUTTON_DOWN, 5 BUTTON_UP (idong_msgs/HandleEvent.msg)
        let cases = [
            (&HandleEvent::Found, 0u8, 0u8),
            (&HandleEvent::NotFound, 1, 0),
            (&HandleEvent::Timeout, 2, 0),
            (&HandleEvent::FoundOver, 3, 0),
            (&HandleEvent::ButtonDown { button: 3 }, 4, 3),
            (&HandleEvent::ButtonUp { button: 1 }, 5, 1),
        ];
        for (e, code, button) in cases {
            let m = HandleEventMsg::from_event(9, e);
            assert_eq!(m.seq, 9);
            assert_eq!(m.event, code);
            assert_eq!(m.button, button);
            assert_eq!(m.handle_id, 0); // until open item #6
        }
    }

    #[test]
    fn link_quality_ordinals_match_the_msg_comment() {
        assert_eq!(Quality::Green.level(), 0);
        assert_eq!(Quality::Yellow.level(), 1);
        assert_eq!(Quality::Red.level(), 2);
    }

    #[test]
    fn device_info_carries_the_identity_facts() {
        let d = DeviceInfoMsg::new(
            "ab460dc8ba12c6840b49cd5786c329dd",
            "3.5.2.6",
            "5da1565c",
            100.0,
            432.33,
        );
        assert_eq!(d.serial.len(), 32);
        assert!((d.baseline_mm - 100.0).abs() < 1e-4);
        assert!((d.focal_px - 432.33).abs() < 1e-4);
    }
}
