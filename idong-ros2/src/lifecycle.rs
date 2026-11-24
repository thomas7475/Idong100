//! Lifecycle semantics without framework lifecycle (draft 5.3).
//!
//! rclrs has no managed-lifecycle node type yet, so we implement the
//! same four states (unconfigured/inactive/active/finalized) with
//! standard transition names, exposed as a ~/transition service and
//! reflected on /idong/status.  Allocation at configure, USB discovery
//! at activate, teardown at shutdown (ros2_ouster order).  When rclrs
//! gains native lifecycle, this whole file swaps out - nothing else.
// TODO(P4.1)
