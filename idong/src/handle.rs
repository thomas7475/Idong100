//! RF handle (wand / airtouch wristband) events.
//!
//! TSOL_DEVICE_HANDLE_FOUND / NOT_FOUND / TIMEOUT / FOUND_OVER ack
//! sequence over the 2.4 GHz link; NeedHandleMatch=1, MonitorList=10
//! in xEyeUsb.ini.  Surfaces as an Event queue on the stream channel.
//!
//! The exact handle-config blob layout is open item #6; until it is
//! pinned from a captured session, the config is serialized verbatim
//! from a recording and only the decoded event kinds are modelled.

/// One RF handle event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleEvent {
    /// TSOL_DEVICE_HANDLE_FOUND: a handle entered the RF link.
    Found,
    /// TSOL_DEVICE_HANDLE_NOT_FOUND: matching round found nothing.
    NotFound,
    /// TSOL_DEVICE_HANDLE_TIMEOUT: matching round timed out.
    Timeout,
    /// TSOL_DEVICE_HANDLE_FOUND_OVER: matching round completed.
    FoundOver,
    /// A button press on the handle (button index 1..=N).
    ButtonDown { button: u8 },
    /// A button release on the handle.
    ButtonUp { button: u8 },
}

impl HandleEvent {
    /// Stable lowercase name (ROS event strings / JSONL dumps).
    pub fn as_str(self) -> &'static str {
        match self {
            HandleEvent::Found => "handle_found",
            HandleEvent::NotFound => "handle_not_found",
            HandleEvent::Timeout => "handle_timeout",
            HandleEvent::FoundOver => "handle_found_over",
            HandleEvent::ButtonDown { .. } => "handle_button_down",
            HandleEvent::ButtonUp { .. } => "handle_button_up",
        }
    }

    /// Is this a button event?
    pub fn is_button(self) -> bool {
        matches!(self, HandleEvent::ButtonDown { .. } | HandleEvent::ButtonUp { .. })
    }
}

/// Handle-match configuration (vendored verbatim until open item #6).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HandleConfig {
    /// NeedHandleMatch=1, MonitorList=10 (xEyeUsb.ini).
    pub need_match: bool,
    pub monitor_list: usize,
    /// Raw serialized config blob from a captured session (open item #6).
    pub raw: Vec<u8>,
}

impl HandleConfig {
    /// Vendor defaults from xEyeUsb.ini.
    pub fn vendor_default() -> Self {
        HandleConfig { need_match: true, monitor_list: 10, raw: Vec::new() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_names_stable() {
        assert_eq!(HandleEvent::Found.as_str(), "handle_found");
        assert!(HandleEvent::ButtonDown { button: 2 }.is_button());
        assert!(!HandleEvent::Timeout.is_button());
        assert_eq!(HandleConfig::vendor_default().monitor_list, 10);
    }
}
