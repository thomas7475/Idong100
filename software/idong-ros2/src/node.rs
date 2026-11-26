//! IdongNode: two threads, one bounded channel (draft 5.2).
//!
//! Executor thread spins the node, drains MarkerFrame/Event values,
//! stamps with receive time (device provides a frame counter, not a
//! clock - time_ref exposes the pairs), converts to idong_msgs, applies
//! the parameter snapshot (Arc<Mutex<TrackingConfig>>, snapshotted per
//! frame; no locks on the hot path).
// TODO(P4.1)
