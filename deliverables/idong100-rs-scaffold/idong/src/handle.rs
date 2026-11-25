//! RF handle (wand / airtouch wristband) events.
//!
//! TSOL_DEVICE_HANDLE_FOUND / NOT_FOUND / TIMEOUT / FOUND_OVER ack
//! sequence over the 2.4 GHz link; NeedHandleMatch=1, MonitorList=10
//! in xEyeUsb.ini.  Surfaces as an Event queue on the stream channel.

#[derive(Debug, Clone, Copy)]
pub enum HandleEvent {
    Found,
    NotFound,
    Timeout,
    FoundOver,
    ButtonDown { button: u8 },
    ButtonUp { button: u8 },
}
