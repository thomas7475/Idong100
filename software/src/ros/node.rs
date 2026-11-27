//! The node core: the executor side of the reader-thread channel
//! (draft 5.2, P4 turn 48).
//!
//! The rclrs glue owns the executor thread and the clock; this struct
//! owns everything that thread does BETWEEN rclrs calls, so it is all
//! testable with plain `cargo test`:
//!
//! * **drain** - one call per delivered [`MarkerFrame`] /
//!   [`HandleEvent`];
//! * **convert** - to the pure message shapes of [`super::pubsub`]
//!   (which the glue copies into the generated `idong_msgs` types);
//! * **stamp** - the glue's receive clock comes IN as `recv_ns`; the
//!   device gives a frame counter, not a clock, so every frame emits a
//!   [`TimeRefPair`] for `/idong/time_ref` (downstream drift
//!   estimation, the sensor-package pattern when PTP is unavailable);
//! * **aggregate** - the diagnostics windows (draft table 4) update in
//!   the same pass.
//!
//! Parameters are applied through the glue's shared
//! `Arc<Mutex<TrackingConfig>>` - [`NodeCore::set_config`] is the
//! snapshot refresh (no locks on the hot path: the conversion below
//! only reads the snapshot).

use super::diagnostics::{Diagnostics, DiagnosticsReport};
use super::params::TrackingConfig;
use super::pubsub::{HandleEventMsg, MarkerArrayMsg};
use crate::handle::HandleEvent;
use crate::marker::MarkerFrame;

/// Device frame counter -> host receive time (the `/idong/time_ref`
/// content; `stamp_ns` is the glue's node-clock nanoseconds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeRefPair {
    pub seq: u32,
    pub stamp_ns: u64,
}

/// What one drained marker frame produces (the glue stamps the
/// `std_msgs/Header`s with the same receive time and publishes).
#[derive(Debug, Clone, PartialEq)]
pub struct FrameOutput {
    /// `/idong/markers` payload.
    pub markers: MarkerArrayMsg,
    /// `/idong/time_ref` payload for this frame.
    pub time_ref: TimeRefPair,
}

/// The pure executor core (draft 5.2). One per node; the glue calls
/// it from its spin loop only.
#[derive(Debug)]
pub struct NodeCore {
    /// Hot parameter snapshot (the reader thread's gate + frame_id).
    cfg: TrackingConfig,
    /// Link-quality/rate/stall aggregation (draft table 4).
    diag: Diagnostics,
    /// Host-assigned event sequence (the vendor events carry no
    /// counter - same posture as the host-assigned frame counter,
    /// turn 46).
    event_seq: u32,
}

impl NodeCore {
    /// Build from the configure-time parameter snapshot.
    pub fn new(cfg: &TrackingConfig, publish_hz_min: u32) -> NodeCore {
        NodeCore { cfg: cfg.clone(), diag: Diagnostics::new(cfg, publish_hz_min), event_seq: 0 }
    }

    /// Live parameter reapply: the glue snapshots its shared config
    /// under the mutex and hands the copy here (never mid-frame - the
    /// executor calls between drains).
    pub fn set_config(&mut self, cfg: &TrackingConfig, publish_hz_min: u32) {
        self.cfg = cfg.clone();
        self.diag.set_config(cfg, publish_hz_min);
    }

    /// The live snapshot (the glue reads this for logging/`status`).
    pub fn config(&self) -> &TrackingConfig {
        &self.cfg
    }

    /// Drain one marker frame: convert (the quality gate of the live
    /// snapshot), stamp, and feed the diagnostics windows.
    pub fn on_frame(&mut self, f: &MarkerFrame, recv_ns: u64) -> FrameOutput {
        self.diag.observe_frame(f.link_quality, f.dropped, recv_ns);
        FrameOutput {
            markers: MarkerArrayMsg::from_frame(f, &self.cfg),
            time_ref: TimeRefPair { seq: f.frame_counter, stamp_ns: recv_ns },
        }
    }

    /// Drain one handle event: `None` when `enable_handle_events` is
    /// off (the glue then drops it instead of publishing).
    pub fn on_event(&mut self, e: &HandleEvent) -> Option<HandleEventMsg> {
        if !self.cfg.enable_handle_events {
            return None;
        }
        let msg = HandleEventMsg::from_event(self.event_seq, e);
        self.event_seq = self.event_seq.wrapping_add(1);
        Some(msg)
    }

    /// The `/idong/diagnostics` report (the glue publishes on its
    /// diagnostics period - not per frame).
    pub fn diagnostics(&self, now_ns: u64) -> DiagnosticsReport {
        self.diag.report(now_ns)
    }

    /// The stream layer's resync counter (the glue feeds the
    /// `StatsSnapshot` deltas).
    pub fn set_resyncs(&mut self, resyncs: u64) {
        self.diag.set_resyncs(resyncs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marker::Quality;
    use crate::protocol::records;

    fn cfg() -> TrackingConfig {
        TrackingConfig {
            frame_id: "idong".into(),
            quality_threshold: 50,
            link_quality_window: 200,
            enable_handle_events: true,
        }
    }

    /// A record with two tracked slots (0 and 2) and one below the
    /// gate (1) - the pubsub fixture shape.
    fn frame_with(counter: u32) -> MarkerFrame {
        let mut b = vec![0u8; records::RECORD_MIN];
        b[0..4].copy_from_slice(&counter.to_le_bytes());
        b[records::MODE_OFFSET..records::MODE_OFFSET + 4].copy_from_slice(&7u32.to_le_bytes());
        b[0x08..0x10].copy_from_slice(&10.0f64.to_le_bytes());
        b[0x10..0x18].copy_from_slice(&20.0f64.to_le_bytes());
        b[0x18..0x20].copy_from_slice(&1500.0f64.to_le_bytes());
        b[0x24..0x28].copy_from_slice(&90i32.to_le_bytes());
        let s1 = records::SLOT_LEN;
        b[s1 + 0x24..s1 + 0x28].copy_from_slice(&10i32.to_le_bytes());
        let s2 = 2 * records::SLOT_LEN;
        b[s2 + 0x08..s2 + 0x10].copy_from_slice(&(-5.0f64).to_le_bytes());
        b[s2 + 0x10..s2 + 0x18].copy_from_slice(&6.0f64.to_le_bytes());
        b[s2 + 0x18..s2 + 0x20].copy_from_slice(&2000.0f64.to_le_bytes());
        b[s2 + 0x24..s2 + 0x28].copy_from_slice(&60i32.to_le_bytes());
        records::parse_record(&b, Some(counter.wrapping_sub(1))).expect("record parses")
    }

    #[test]
    fn a_frame_drains_through_convert_stamp_and_aggregate() {
        let mut core = NodeCore::new(&cfg(), 30);
        let t0 = 5_000_000_000u64;
        let mut f = frame_with(100);
        // the placeholder link classification (from_count on 2
        // markers) reads Red until open item #3 pins the disturbance
        // counters - this test targets the plumbing, not the
        // placeholder, so pin the frame green.
        f.link_quality = Quality::Green;
        let out = core.on_frame(&f, t0);
        assert_eq!(out.markers.seq, 100);
        assert_eq!(out.markers.frame_id, "idong");
        assert_eq!(out.markers.markers.len(), 2); // slot 1 below the gate
        assert_eq!(out.markers.link_quality, Quality::Green.level());
        assert_eq!(out.time_ref, TimeRefPair { seq: 100, stamp_ns: t0 });
        // the diagnostics window saw it (one stamp = no rate yet, so
        // not degraded either)
        let r = core.diagnostics(t0);
        assert_eq!(r.frames, 1);
        assert_eq!(r.link, Quality::Green);
        assert_eq!(r.level, super::super::diagnostics::DiagnosticsLevel::Ok);
        assert!(!r.degraded);
    }

    #[test]
    fn the_gate_is_the_live_snapshot() {
        let mut core = NodeCore::new(&cfg(), 30);
        let t0 = 1_000_000_000u64;
        let f = frame_with(7);
        assert_eq!(core.on_frame(&f, t0).markers.markers.len(), 2);
        // reapply: stricter gate - slot 2 (quality 60) drops out,
        // slot 0 (quality 90) stays
        let stricter = TrackingConfig { quality_threshold: 80, ..cfg() };
        core.set_config(&stricter, 30);
        let out = core.on_frame(&f, t0 + 20_000_000);
        assert_eq!(out.markers.markers.len(), 1);
        assert_eq!(out.markers.markers[0].slot, 0);
        assert_eq!(core.config().quality_threshold, 80);
    }

    #[test]
    fn events_sequence_and_the_switch() {
        let mut core = NodeCore::new(&cfg(), 30);
        let a = core.on_event(&HandleEvent::Found).expect("enabled");
        let b = core.on_event(&HandleEvent::ButtonDown { button: 2 }).expect("enabled");
        assert_eq!(a.seq, 0);
        assert_eq!(b.seq, 1);
        assert_eq!(b.event, 4);
        assert_eq!(b.button, 2);
        // off: no message, and the sequence does not advance
        let off = TrackingConfig { enable_handle_events: false, ..cfg() };
        core.set_config(&off, 30);
        assert!(core.on_event(&HandleEvent::Found).is_none());
        core.set_config(&cfg(), 30);
        // next published event continues from 2
        let c = core.on_event(&HandleEvent::Timeout).expect("back on");
        assert_eq!(c.seq, 2);
    }

    #[test]
    fn drops_and_resyncs_reach_the_report() {
        let mut core = NodeCore::new(&cfg(), 30);
        let t0 = 1_000_000_000u64;
        // a dropped frame: counter gap of 2 -> the parser flags it
        let gap = frame_with(11);
        let out = core.on_frame(&gap, t0);
        assert_eq!(out.markers.seq, 11);
        let mut dropped = frame_with(13);
        dropped.dropped = true;
        core.on_frame(&dropped, t0 + 20_000_000);
        core.set_resyncs(3);
        let r = core.diagnostics(t0 + 40_000_000);
        assert_eq!(r.drops, 1);
        assert_eq!(r.resyncs, 3);
        assert_eq!(r.frames, 2);
    }
}
