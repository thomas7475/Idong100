//! ROS 2 adapter - the pure data layer (P4, turn 48).
//!
//! Everything the ROS node needs that is NOT rclrs: the parameter
//! model (draft table 5), the lifecycle machine (draft 5.3), the
//! topic/QoS surface with the `idong_msgs` message shapes and their
//! converters from the core data model (draft 5.1), the diagnostics
//! aggregation, and the node core (draft 5.2 - the executor side of
//! the reader-thread channel: drain, convert, stamp).
//!
//! The rclrs glue itself lives in `software/idong-ros2` - the ONLY
//! crate that sees rclrs and the generated `idong_msgs` crate (it is
//! built by colcon-ros-cargo inside a ROS workspace; cargo never
//! builds it, because the generated message crate only exists there).
//! This split keeps the whole adapter testable with plain
//! `cargo test` - hardware-free and ROS-free, exactly like the rest
//! of the crate.
//!
//! The pure message structs here mirror the `.msg`/`.srv` files in
//! `software/idong_msgs` field-for-field (a unit test pins the
//! `params.yaml` defaults the same way). The glue copies them into
//! the generated types - the only place message types leak.

pub mod diagnostics;
pub mod lifecycle;
pub mod node;
pub mod params;
pub mod pubsub;

pub use diagnostics::DiagnosticsReport;
pub use lifecycle::{LifecycleState, Transition};
pub use node::NodeCore;
pub use params::{RosParams, TrackingConfig};
