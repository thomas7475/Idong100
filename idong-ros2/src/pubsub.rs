//! Topics + QoS (draft table 4): markers best-effort keep-last 5 (a late
//! frame is worthless), events reliable keep-last 10 (a lost click is a
//! lost action), diagnostics + time_ref reliable.  Converters from core
//! data model to idong_msgs types live here - the only place message
//! types leak into our code.
// TODO(P4.1)
