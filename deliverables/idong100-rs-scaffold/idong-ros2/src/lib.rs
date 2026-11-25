//! ROS 2 adapter crate - the ONLY crate that sees rclrs (draft 3.1).
//!
//! Node surface (draft table 4): /idong/markers (sensor-data QoS),
//! /idong/events (reliable), /idong/diagnostics, /idong/time_ref,
//! get_device_info service, ~/transition.  Parameters: draft table 5.
//! Threading: reader thread (core) -> bounded channel -> executor
//! spin loop (draft 5.2).

pub mod diagnostics;
pub mod lifecycle;
pub mod node;
pub mod params;
pub mod pubsub;

// rclrs_create_node! registration lands at P4.1; see draft section 5
// for the ~40-line entry-point skeleton.
