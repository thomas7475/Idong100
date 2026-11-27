//! Parameter surface (draft table 5, P4 turn 48).
//!
//! Every vendor runtime setting the node mirrors as a ROS parameter:
//! serial match, frame_id, the degraded-link floor, the slot quality
//! gate, the link-quality window, the channel depth, the firmware
//! folder for the recovery path, the handle-event switch, and the
//! write-risk consent flag. All reconfigurable where the hardware
//! allows; the hot subset is snapshotted per frame into
//! [`TrackingConfig`] (draft 5.2: no locks on the hot path - a mutex
//! snapshot per frame, never during conversion).
//!
//! The defaults ARE the launch file's `params.yaml` (pinned by the
//! `params_yaml_matches_defaults` test - the file may drift only
//! together with the code defaults, never alone).

use crate::error::{IdongError, Result};

/// Node parameters (draft table 5, all fields cited).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosParams {
    /// Expect this device only (xeyez stack handle-match). Empty =
    /// any device. The 32-char EEPROM device ID string when set.
    pub serial_number: String,
    /// `header.frame_id` of all outputs.
    pub frame_id: String,
    /// Below this the node flags a degraded link (vendor xEyeFrame=50;
    /// the record plane is 50 fps, 30 keeps margin).
    pub publish_hz_min: u32,
    /// Slot quality gate (xEyeClass filter: quality >= 50 tracked).
    pub quality_threshold: i32,
    /// Frames averaged for the green/yellow/red level (RingSize=200).
    pub link_quality_window: u32,
    /// Bounded channel depth = QoS depth.
    pub buffer_depth: u32,
    /// User-supplied vendor firmware dir (sha256-verified before any
    /// upload; the recovery path only).
    pub firmware_dir: String,
    /// NeedHandleMatch=1 analog - publish handle events.
    pub enable_handle_events: bool,
    /// Must be set for ANY interface that writes to the device (the
    /// same consent posture as `flash_mcu`'s two-key gate; the glue
    /// refuses to flash with it unset).
    pub i_know_the_write_risks: bool,
}

impl Default for RosParams {
    fn default() -> Self {
        RosParams {
            serial_number: String::new(),
            frame_id: "idong".to_string(),
            publish_hz_min: 30,
            quality_threshold: 50,
            link_quality_window: 200,
            buffer_depth: 5,
            firmware_dir: String::new(),
            enable_handle_events: true,
            i_know_the_write_risks: false,
        }
    }
}

impl RosParams {
    /// Validate the parameter set (configure-time; the glue rejects
    /// the configure transition on error).
    pub fn validate(&self) -> Result<()> {
        if !self.serial_number.is_empty()
            && (self.serial_number.len() != 32
                || !self.serial_number.chars().all(|c| c.is_ascii_hexdigit()))
        {
            return Err(IdongError::Usb(format!(
                "serial_number must be the 32-char EEPROM device ID (hex), got {:?}",
                self.serial_number
            )));
        }
        if self.frame_id.is_empty() {
            return Err(IdongError::Usb("frame_id must not be empty".into()));
        }
        // The record plane is 50 fps (vendor xEyeFrame); 1..=50 keeps
        // the floor honest in both directions.
        if !(1..=50).contains(&self.publish_hz_min) {
            return Err(IdongError::Usb(format!(
                "publish_hz_min must be 1..=50 (the record plane is 50 fps), got {}",
                self.publish_hz_min
            )));
        }
        if !(0..=100).contains(&self.quality_threshold) {
            return Err(IdongError::Usb(format!(
                "quality_threshold must be 0..=100, got {}",
                self.quality_threshold
            )));
        }
        if self.link_quality_window == 0 {
            return Err(IdongError::Usb("link_quality_window must be > 0".into()));
        }
        if !(1..=64).contains(&self.buffer_depth) {
            return Err(IdongError::Usb(format!(
                "buffer_depth must be 1..=64 (QoS depth), got {}",
                self.buffer_depth
            )));
        }
        Ok(())
    }

    /// The per-frame hot subset (draft 5.2): snapshotted under the
    /// node's mutex, then read lock-free for the whole frame.
    pub fn tracking(&self) -> TrackingConfig {
        TrackingConfig {
            frame_id: self.frame_id.clone(),
            quality_threshold: self.quality_threshold,
            link_quality_window: self.link_quality_window,
            enable_handle_events: self.enable_handle_events,
        }
    }
}

/// Lock-free snapshot of the per-frame parameters (draft 5.2: applied
/// through a shared mutex that the conversion snapshots per frame).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackingConfig {
    /// `header.frame_id` of the marker output.
    pub frame_id: String,
    /// Slot quality gate (>= threshold = tracked).
    pub quality_threshold: i32,
    /// Frames averaged for the link-quality level.
    pub link_quality_window: u32,
    /// Handle events published?
    pub enable_handle_events: bool,
}

/// Parse the launch file's `params.yaml` (the fixed two-level shape
/// ROS 2 writes: `/idong: ros__parameters:` then `key: value` lines).
///
/// Not a YAML engine - the shipped file is one flat block of scalar
/// parameters, and that is all the node owns. Unknown keys are an
/// error (typo protection: a mistyped parameter that silently falls
/// back to a default is the classic ROS footgun).
pub fn parse_params_yaml(text: &str) -> Result<RosParams> {
    let mut p = RosParams::default();
    let mut seen_params_block = false;
    let mut seen: Vec<String> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim_end();
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let t = line.trim();
        if t == "/idong:" || t == "idong:" {
            seen_params_block = true;
            continue;
        }
        if !seen_params_block {
            continue; // preamble before the node's block
        }
        let Some((key, value)) = t.split_once(':') else {
            continue; // structural line (e.g. "ros__parameters:")
        };
        let value = value.trim();
        match key {
            "ros__parameters" => {}
            "serial_number" => p.serial_number = unquote(value),
            "frame_id" => p.frame_id = unquote(value),
            "publish_hz_min" => p.publish_hz_min = parse_num(value, "publish_hz_min")?,
            "quality_threshold" => p.quality_threshold = parse_num(value, "quality_threshold")?,
            "link_quality_window" => {
                p.link_quality_window = parse_num(value, "link_quality_window")?
            }
            "buffer_depth" => p.buffer_depth = parse_num(value, "buffer_depth")?,
            "firmware_dir" => p.firmware_dir = unquote(value),
            "enable_handle_events" => {
                p.enable_handle_events = parse_bool(value, "enable_handle_events")?
            }
            "i_know_the_write_risks" => {
                p.i_know_the_write_risks = parse_bool(value, "i_know_the_write_risks")?
            }
            other => {
                return Err(IdongError::Usb(format!(
                    "params.yaml: unknown parameter {other:?} (typo protection)"
                )))
            }
        }
        seen.push(key.to_string());
    }
    if !seen_params_block {
        return Err(IdongError::Usb("params.yaml: no /idong: block found".into()));
    }
    if seen.is_empty() {
        return Err(IdongError::Usb("params.yaml: no parameters under ros__parameters".into()));
    }
    Ok(p)
}

fn unquote(v: &str) -> String {
    let v = v.trim();
    let v = v.strip_prefix('"').unwrap_or(v);
    let v = v.strip_suffix('"').unwrap_or(v);
    v.to_string()
}

fn parse_num<T: std::str::FromStr>(v: &str, name: &str) -> Result<T> {
    v.trim()
        .parse::<T>()
        .map_err(|_| IdongError::Usb(format!("params.yaml: {name} is not a number: {v:?}")))
}

fn parse_bool(v: &str, name: &str) -> Result<bool> {
    match v.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => {
            Err(IdongError::Usb(format!("params.yaml: {name} must be true/false, got {other:?}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_draft_table() {
        let d = RosParams::default();
        assert_eq!(d.serial_number, "");
        assert_eq!(d.frame_id, "idong");
        assert_eq!(d.publish_hz_min, 30);
        assert_eq!(d.quality_threshold, 50);
        assert_eq!(d.link_quality_window, 200);
        assert_eq!(d.buffer_depth, 5);
        assert_eq!(d.firmware_dir, "");
        assert!(d.enable_handle_events);
        assert!(!d.i_know_the_write_risks);
        assert!(d.validate().is_ok());
    }

    #[test]
    fn tracking_snapshot_carries_the_hot_subset() {
        let t = RosParams::default().tracking();
        assert_eq!(t.frame_id, "idong");
        assert_eq!(t.quality_threshold, 50);
        assert_eq!(t.link_quality_window, 200);
        assert!(t.enable_handle_events);
    }

    #[test]
    fn validation_rejects_the_bad_values() {
        let mut p = RosParams::default();
        p.serial_number = "AB460DC8".into(); // not 32 chars
        assert!(p.validate().is_err());
        p.serial_number = "ab460dc8ba12c6840b49cd5786c329dd".into(); // valid, lowercase hex
        assert!(p.validate().is_ok());
        p.publish_hz_min = 0;
        assert!(p.validate().is_err());
        p.publish_hz_min = 51;
        assert!(p.validate().is_err());
        p.publish_hz_min = 50;
        assert!(p.validate().is_ok());
        p.quality_threshold = 101;
        assert!(p.validate().is_err());
        p.quality_threshold = 100;
        assert!(p.validate().is_ok());
        p.buffer_depth = 0;
        assert!(p.validate().is_err());
        p.frame_id.clear();
        assert!(p.validate().is_err());
    }

    #[test]
    fn params_yaml_matches_defaults() {
        // The launch file may only drift together with the code
        // defaults, never alone.
        let text =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/idong-ros2/params.yaml"))
                .expect("idong-ros2/params.yaml ships with the repository");
        let parsed = parse_params_yaml(&text).expect("the shipped params.yaml parses");
        assert_eq!(parsed, RosParams::default());
    }

    #[test]
    fn yaml_parser_covers_values_and_errors() {
        let text = "/idong:\n  ros__parameters:\n    frame_id: \"cam\"\n    \
                    publish_hz_min: 25\n    enable_handle_events: false\n";
        let p = parse_params_yaml(text).unwrap();
        assert_eq!(p.frame_id, "cam");
        assert_eq!(p.publish_hz_min, 25);
        assert!(!p.enable_handle_events);
        // everything else falls back to the defaults
        assert_eq!(p.quality_threshold, 50);

        // unknown key = typo protection
        let bad = "/idong:\n  ros__parameters:\n    quailty_threshold: 50\n";
        let err = parse_params_yaml(bad).unwrap_err().to_string();
        assert!(err.contains("unknown parameter"), "{err}");

        // bad bool, bad int, missing block
        assert!(parse_params_yaml("/idong:\n  ros__parameters:\n    enable_handle_events: yes\n")
            .is_err());
        assert!(parse_params_yaml("/idong:\n  ros__parameters:\n    buffer_depth: five\n").is_err());
        assert!(parse_params_yaml("frame_id: x\n").is_err());
    }
}
