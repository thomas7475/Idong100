//! Stream health -> `/idong/diagnostics` payloads (draft table 4, P4
//! turn 48; the scaffold's `diagnostics.rs` TODO(P4.3) made real).
//!
//! The vendor classification averaged disturbance over `RingSize=200`
//! frames (`xEyeUsb.ini`), so the node does the same over its
//! [`link_quality_window`] parameter: a ring of per-frame traffic
//! lights, classified with the vendor's own thresholds as fractions
//! (`Green=20 / Yellow=35`): a window whose non-green share reaches
//! 35% is red, 20% yellow, else green.
//!
//! On top of the traffic light: the degraded-link floor
//! ([`publish_hz_min`], vendor `xEyeFrame=50` with margin) measured
//! from the last second of receive timestamps, and the sanity timeout
//! (the sensor-package thread guidance: if frames stop, SAY so) - no
//! frame for [`STALL_TIMEOUT_NS`] reports ERROR with the age of the
//! last frame.
//!
//! This file is the pure decision core: timestamps come in as `u64`
//! nanoseconds (the glue passes the node clock), and the report is a
//! plain struct the glue copies into `diagnostic_msgs/DiagnosticArray`.

use super::params::TrackingConfig;
use crate::marker::Quality;

/// No frame for this long => ERROR ("frames stopped") - the sanity
/// timeout. 500 ms = 25 missing records at the 50 fps plane, long
/// enough that a busy executor tick never trips it.
pub const STALL_TIMEOUT_NS: u64 = 500_000_000;

/// `diagnostic_msgs/KeyValue` analog (the glue copies these into the
/// generated status message).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyValue {
    pub key: String,
    pub value: String,
}

impl KeyValue {
    fn new(key: &str, value: impl core::fmt::Display) -> KeyValue {
        KeyValue { key: key.to_string(), value: value.to_string() }
    }
}

/// `diagnostic_msgs/DiagnosticStatus.level` (the exact byte values of
/// the generated enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticsLevel {
    /// 0: operating normally.
    Ok,
    /// 1: degraded but producing data.
    Warn,
    /// 2: not producing usable data.
    Error,
    /// 3: the source stopped updating (the stall case reports Error
    /// with the last-frame age instead - Stale is kept for the glue's
    /// not-yet-published empty-array case).
    Stale,
}

impl DiagnosticsLevel {
    pub fn level(self) -> u8 {
        match self {
            DiagnosticsLevel::Ok => 0,
            DiagnosticsLevel::Warn => 1,
            DiagnosticsLevel::Error => 2,
            DiagnosticsLevel::Stale => 3,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            DiagnosticsLevel::Ok => "OK",
            DiagnosticsLevel::Warn => "WARN",
            DiagnosticsLevel::Error => "ERROR",
            DiagnosticsLevel::Stale => "STALE",
        }
    }
}

/// One `/idong/diagnostics` report (the glue stamps + wraps it).
#[derive(Debug, Clone, PartialEq)]
pub struct DiagnosticsReport {
    /// Overall status level.
    pub level: DiagnosticsLevel,
    /// Human-readable one-liner (the status `message` field).
    pub message: String,
    /// Window traffic light (0 green / 1 yellow / 2 red).
    pub link: Quality,
    /// Window size actually used (the live parameter).
    pub window: u32,
    /// Frames delivered since activate.
    pub frames: u64,
    /// Frame-counter gaps (record `dropped` flags).
    pub drops: u64,
    /// Stream resyncs (StatsSnapshot deltas, set by the glue).
    pub resyncs: u64,
    /// Measured rate over the last second, Hz.
    pub hz: f64,
    /// True when hz is below `publish_hz_min` with frames flowing.
    pub degraded: bool,
    /// True when no frame arrived within [`STALL_TIMEOUT_NS`].
    pub stalled: bool,
    /// Age of the last frame at report time, ns.
    pub last_frame_age_ns: u64,
}

impl DiagnosticsReport {
    /// The `diagnostic_msgs` status key/value array (stable keys; a
    /// graph or `ros2 topic echo` shows exactly these).
    pub fn key_values(&self) -> Vec<KeyValue> {
        vec![
            KeyValue::new("link", self.link.level()),
            KeyValue::new("window", self.window),
            KeyValue::new("frames", self.frames),
            KeyValue::new("drops", self.drops),
            KeyValue::new("resyncs", self.resyncs),
            KeyValue::new("hz", format!("{:.1}", self.hz)),
            KeyValue::new("degraded", self.degraded),
            KeyValue::new("stalled", self.stalled),
            KeyValue::new("last_frame_age_ms", self.last_frame_age_ns / 1_000_000),
        ]
    }
}

/// The ring of the last `window` per-frame traffic lights plus the
/// last-second receive timestamps - all aggregation state in one
/// place, driven by explicit clocks (pure = testable).
#[derive(Debug)]
pub struct Diagnostics {
    /// Below this the node flags a degraded link (the parameter).
    publish_hz_min: u32,
    /// Window size (the live parameter; the ring resizes on reconfigure).
    window: usize,
    /// Traffic-light ordinal (0/1/2) per frame, oldest first, cap = window.
    lights: std::collections::VecDeque<u8>,
    /// Receive timestamps (ns) of the frames used for the rate.
    stamps: std::collections::VecDeque<u64>,
    /// Frames observed since construction.
    frames: u64,
    /// Frame-counter gaps (from the records' `dropped` flag).
    drops: u64,
    /// Resyncs (set from StatsSnapshot deltas by the glue).
    resyncs: u64,
    /// Last frame receive time (ns); None before the first frame.
    last_frame_ns: Option<u64>,
}

impl Diagnostics {
    /// New aggregator over the hot config subset + the rate floor.
    pub fn new(cfg: &TrackingConfig, publish_hz_min: u32) -> Diagnostics {
        Diagnostics {
            publish_hz_min,
            window: cfg.link_quality_window.max(1) as usize,
            lights: Default::default(),
            stamps: Default::default(),
            frames: 0,
            drops: 0,
            resyncs: 0,
            last_frame_ns: None,
        }
    }

    /// Parameter reapply (live): the window resizes; the ring keeps
    /// its history and just caps at the new size going forward.
    pub fn set_config(&mut self, cfg: &TrackingConfig, publish_hz_min: u32) {
        self.publish_hz_min = publish_hz_min;
        self.window = cfg.link_quality_window.max(1) as usize;
        let cap = self.window;
        while self.lights.len() > cap {
            self.lights.pop_front();
        }
    }

    /// Observe one delivered frame (the executor calls this per drain).
    /// `dropped` is the record's own frame-counter-gap flag.
    pub fn observe_frame(&mut self, link: Quality, dropped: bool, recv_ns: u64) {
        self.lights.push_back(link.level());
        if self.lights.len() > self.window {
            self.lights.pop_front();
        }
        self.stamps.push_back(recv_ns);
        while self.stamps.front().is_some_and(|t| recv_ns - t > 1_000_000_000) {
            self.stamps.pop_front();
        }
        self.frames += 1;
        if dropped {
            self.drops += 1;
        }
        self.last_frame_ns = Some(recv_ns);
    }

    /// The stream layer's resync counter (StatsSnapshot deltas).
    pub fn set_resyncs(&mut self, resyncs: u64) {
        self.resyncs = resyncs;
    }

    /// Measured rate over the last second (the parameter floor's
    /// denominator). Pure: the caller's clock wins. The rate is
    /// intervals over measured span (NOT the count - a count is only
    /// Hz if the frames were exactly cadenced).
    pub fn hz(&self, now_ns: u64) -> f64 {
        self.fresh_hz(now_ns).0
    }

    /// `(rate, fresh_count)` over the trailing second - the report
    /// needs the count too (a rate off ONE timestamp is not a
    /// measurement).
    fn fresh_hz(&self, now_ns: u64) -> (f64, usize) {
        // stamps inside the trailing second (the cleanup in
        // observe_frame keeps the deque at that window, but a long
        // report gap can age entries - re-check here).
        let fresh: Vec<u64> = self
            .stamps
            .iter()
            .copied()
            .filter(|t| now_ns.saturating_sub(*t) <= 1_000_000_000)
            .collect();
        if fresh.len() <= 1 {
            return (0.0, fresh.len());
        }
        let span_ns = fresh[fresh.len() - 1] - fresh[0];
        if span_ns == 0 {
            return (0.0, fresh.len());
        }
        ((fresh.len() - 1) as f64 / (span_ns as f64 / 1e9), fresh.len())
    }

    /// The window traffic light: non-green share >= 35% red, >= 20%
    /// yellow, else green (the vendor `xEyeUsb.ini` thresholds as
    /// fractions, applied over `RingSize` frames).
    pub fn link(&self) -> Quality {
        let n = self.lights.len();
        if n == 0 {
            return Quality::Green;
        }
        let non_green = self.lights.iter().filter(|l| **l > 0).count();
        let share = non_green as f64 / n as f64;
        if share >= 0.35 {
            Quality::Red
        } else if share >= 0.20 {
            Quality::Yellow
        } else {
            Quality::Green
        }
    }

    /// No frame within the sanity timeout?
    pub fn stalled(&self, now_ns: u64) -> bool {
        match self.last_frame_ns {
            Some(last) => now_ns.saturating_sub(last) > STALL_TIMEOUT_NS,
            // activated but nothing yet: only stall after the timeout
            // has passed since construction (last_frame_ns None) - the
            // caller seeds `now_ns` at activate.
            None => false,
        }
    }

    /// Build the report (the glue publishes this on the diagnostics
    /// period and at transitions).
    pub fn report(&self, now_ns: u64) -> DiagnosticsReport {
        let stalled = self.stalled(now_ns);
        let (hz, measured) = self.fresh_hz(now_ns);
        // The floor needs a MEASURED rate: at least two receive
        // timestamps inside the window (one stamp is not a rate - a
        // trickle stream is caught by it too, once the second stamp
        // lands; a stopped stream is the stall case below).
        let degraded = !stalled && measured >= 2 && hz < self.publish_hz_min as f64;
        let link = self.link();
        // ERROR when stalled; WARN when the floor is missed or the
        // window is red; otherwise OK (a yellow window stays OK - the
        // vendor traffic light already says "usable").
        let level = if stalled {
            DiagnosticsLevel::Error
        } else if degraded || link == Quality::Red {
            DiagnosticsLevel::Warn
        } else {
            DiagnosticsLevel::Ok
        };
        let message = if stalled {
            format!(
                "frames stopped: last frame {} ms ago",
                now_ns.saturating_sub(self.last_frame_ns.unwrap_or(now_ns)) / 1_000_000
            )
        } else if degraded {
            format!("degraded link: {:.1} Hz < {} Hz floor", hz, self.publish_hz_min)
        } else {
            format!(
                "{} link, {:.1} Hz, {} tracked-window frames",
                link.level(),
                hz,
                self.lights.len()
            )
        };
        DiagnosticsReport {
            level,
            message,
            link,
            window: self.window as u32,
            frames: self.frames,
            drops: self.drops,
            resyncs: self.resyncs,
            hz,
            degraded,
            stalled,
            last_frame_age_ns: self.last_frame_ns.map_or(0, |t| now_ns.saturating_sub(t)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(window: u32) -> TrackingConfig {
        TrackingConfig {
            frame_id: "idong".into(),
            quality_threshold: 50,
            link_quality_window: window,
            enable_handle_events: true,
        }
    }

    #[test]
    fn window_classifies_with_the_vendor_thresholds() {
        let mut d = Diagnostics::new(&cfg(100), 30);
        // 10% yellow -> green
        for i in 0..100u64 {
            let q = if i % 10 == 0 { Quality::Yellow } else { Quality::Green };
            d.observe_frame(q, false, 20_000_000 * i); // 50 Hz
        }
        assert_eq!(d.link(), Quality::Green);
        // 25% non-green -> yellow
        let mut d = Diagnostics::new(&cfg(100), 30);
        for i in 0..100u64 {
            let q = if i % 4 == 0 { Quality::Yellow } else { Quality::Green };
            d.observe_frame(q, false, 20_000_000 * i);
        }
        assert_eq!(d.link(), Quality::Yellow);
        // 40% non-green (all red) -> red
        let mut d = Diagnostics::new(&cfg(100), 30);
        for i in 0..100u64 {
            let q = if i % 5 < 2 { Quality::Red } else { Quality::Green };
            d.observe_frame(q, false, 20_000_000 * i);
        }
        assert_eq!(d.link(), Quality::Red);
    }

    #[test]
    fn rate_measures_the_last_second() {
        // 50 frames 20 ms apart = exactly 50 Hz (intervals over span,
        // not the raw count)
        let t0 = 1_000_000_000u64;
        let mut d = Diagnostics::new(&cfg(200), 30);
        for i in 0..50u64 {
            d.observe_frame(Quality::Green, false, t0 + 20_000_000 * i);
        }
        let now = t0 + 20_000_000 * 49;
        assert!((d.hz(now) - 50.0).abs() < 1e-6, "49 intervals / 0.98 s");
        // half the rate: frames every 40 ms -> 25 Hz (only the
        // trailing second of stamps counts: 25 stamps, 24 intervals
        // over 0.96 s)
        let mut d = Diagnostics::new(&cfg(200), 30);
        for i in 0..50u64 {
            d.observe_frame(Quality::Green, false, t0 + 40_000_000 * i);
        }
        assert!((d.hz(t0 + 40_000_000 * 49) - 25.0).abs() < 1e-6);
    }

    #[test]
    fn stall_and_degrade_levels() {
        let t0 = 10_000_000_000u64;
        let mut d = Diagnostics::new(&cfg(200), 30);
        // healthy: 50 Hz for a second
        for i in 0..50u64 {
            d.observe_frame(Quality::Green, false, t0 + 20_000_000 * i);
        }
        let r = d.report(t0 + 20_000_000 * 49);
        assert_eq!(r.level, DiagnosticsLevel::Ok);
        assert!(!r.degraded);
        assert!((r.hz - 50.0).abs() < 1e-6);
        // degraded: 25 Hz stream under the 30 Hz floor
        let mut d = Diagnostics::new(&cfg(200), 30);
        for i in 0..25u64 {
            d.observe_frame(Quality::Green, false, t0 + 40_000_000 * i);
        }
        let r = d.report(t0 + 40_000_000 * 24);
        assert_eq!(r.level, DiagnosticsLevel::Warn);
        assert!(r.degraded, "25 Hz < 30 Hz floor");
        assert!(r.message.contains("degraded"));
        // stalled: last frame 600 ms ago
        let mut d = Diagnostics::new(&cfg(200), 30);
        for i in 0..50u64 {
            d.observe_frame(Quality::Green, false, t0 + 20_000_000 * i);
        }
        let now = t0 + 20_000_000 * 49 + 600_000_000;
        let r = d.report(now);
        assert_eq!(r.level, DiagnosticsLevel::Error);
        assert!(r.stalled);
        assert_eq!(r.last_frame_age_ns, 600_000_000);
        assert!(r.message.contains("frames stopped"));
    }

    #[test]
    fn drops_and_resyncs_are_counted() {
        let mut d = Diagnostics::new(&cfg(200), 30);
        let t0 = 1_000_000_000u64;
        for i in 0..10u64 {
            d.observe_frame(Quality::Green, i % 3 == 0, t0 + 20_000_000 * i);
        }
        d.set_resyncs(7);
        let r = d.report(t0 + 20_000_000 * 9);
        assert_eq!(r.frames, 10);
        assert_eq!(r.drops, 4); // i = 0, 3, 6, 9
        assert_eq!(r.resyncs, 7);
        assert_eq!(r.window, 200);
    }

    #[test]
    fn reconfigure_resizes_the_window() {
        let mut d = Diagnostics::new(&cfg(100), 30);
        let t0 = 1_000_000_000u64;
        // 100 frames, 30% yellow interleaved (i%10 < 3) so the full
        // window and its newest 10 (i=90..99: three yellows) both
        // classify yellow
        for i in 0..100u64 {
            let q = if i % 10 < 3 { Quality::Yellow } else { Quality::Green };
            d.observe_frame(q, false, t0 + 20_000_000 * i);
        }
        assert_eq!(d.link(), Quality::Yellow); // 30% non-green
                                               // shrink to 10: the ring keeps the newest 10 (still 30%)
        d.set_config(&cfg(10), 30);
        let r = d.report(t0 + 20_000_000 * 99);
        assert_eq!(r.window, 10);
        assert_eq!(d.link(), Quality::Yellow);
        // and the floor moves with the parameter: the trailing
        // second of the 50 Hz run measures 50 Hz, so a floor of 51
        // flags it
        d.set_config(&cfg(10), 51);
        let r = d.report(t0 + 20_000_000 * 99);
        assert!((r.hz - 50.0).abs() < 1e-6);
        assert!(r.degraded, "hz 50 < new floor 51");
    }

    #[test]
    fn key_values_and_level_ordinals() {
        let r = DiagnosticsReport {
            level: DiagnosticsLevel::Warn,
            message: "x".into(),
            link: Quality::Yellow,
            window: 200,
            frames: 1,
            drops: 0,
            resyncs: 0,
            hz: 25.5,
            degraded: true,
            stalled: false,
            last_frame_age_ns: 40_000_000,
        };
        let kv = r.key_values();
        assert_eq!(kv[0].key, "link");
        assert_eq!(kv[0].value, "1");
        assert_eq!(kv[5].value, "25.5");
        assert_eq!(kv[8].value, "40"); // ms
        assert_eq!(DiagnosticsLevel::Ok.level(), 0);
        assert_eq!(DiagnosticsLevel::Warn.level(), 1);
        assert_eq!(DiagnosticsLevel::Error.level(), 2);
        assert_eq!(DiagnosticsLevel::Stale.level(), 3);
        assert_eq!(DiagnosticsLevel::Error.as_str(), "ERROR");
    }
}
