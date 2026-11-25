# Turn 13 — Native Rust driver with ROS 2 output: implementation draft

**Date:** 2026-11-24
**Request:** research a good, easy-to-maintain standard driver and file
structure; make a draft on how to implement it with an overview of the
driver structure and the steps to implement it. Rust, ROS 2-compatible
output.

## What was done

1. **Web research (verified 2026-11):**
   - rclrs is the official ROS 2 Rust client library; 0.5.0 release notes
     (Aug 2025, discourse.openrobotics.org/t/49681): fully async API,
     Windows support, Jazzy/Kilted/Rolling; released on crates.io (0.8
     line). Feature list covers pub/sub + QoS + loaned messages,
     services, actions, parameters, timers, executors. **No managed
     lifecycle nodes; no API stability guarantees** (README).
   - rosidl_generator_rs is released on the ROS buildfarm (Rolling);
     custom messages generate Rust bindings from plain rosidl packages.
   - colcon integration via colcon-cargo + colcon-ros-cargo plugins +
     cargo-ament-build; package.xml build_type `ament_cargo`
     (iceoryx2 colcon examples).
   - Design benchmarks: ros2_ouster design doc (lifecycle node +
     component, sensor/data interface split, GetMetaData service,
     preallocation at configure, 80% coverage target);
     "What makes a good sensor package" discourse thread
     (t/38805: single lifecycle node, params for all settings, live
     reconfigure, standard messages, diagnostics, timestamp discipline
     - stamp at receive + time_ref pairs when no PTP); xtion-rs (rusb
     transport, reader thread, typed boot errors, golden vectors,
     #[ignore] hardware tests).
2. **Architecture decision — four-crate workspace:**
   - `idong` (pure core: protocol, records, calib, stream; NO rusb, NO
     rclrs; testable with golden vectors + MockTransport),
   - `idong-transport` (rusb; discovery, bulk I/O, re-enum wait, udev),
   - `idong_msgs` (rosidl: MarkerArray.msg, HandleEvent.msg,
     DeviceInfo.srv),
   - `idong-ros2` (rclrs node: params, QoS, diagnostics, lifecycle
     semantics via internal state machine + ~/transition service since
     rclrs lacks managed nodes - isolated in lifecycle.rs for future
     swap).
   - QoS split: markers best-effort keep-last 5; events reliable;
     time_ref topic exposes (device frame counter, host clock) pairs.
3. **Deliverables:**
   - `download/idong100_RUST_ROS2_DRIVER_DRAFT.pdf` — 15 pp: cover
     (Template 07), TOC, ch 1-10, appendices A (protocol constants
     quick reference) + B (package.xml / Cargo.toml skeletons); 2
     figures (workspace+dataflow, P0-P4 roadmap), 8 tables. pdf_qa PASS
     (2 benign nested-callout table-centering warnings), toc.check
     pass, placeholder scan clean, page images rendered and checked.
   - `download/idong100-rs-scaffold/` + `idong100_rust_scaffold.zip` —
     45-file workspace skeleton exactly per section 4 of the PDF:
     workspace Cargo.toml/rustfmt, README with safety rules,
     docs/PROTOCOL.md skeleton, 4 crates with doc-comment stubs citing
     disassembly sources, examples (list/probe/upload/dump), tests
     README, launch file, params.yaml.
   - Scripts: `scripts/gen_driver_report.py`, `merge_driver_report.py`,
     `make_driver_figures.py`, `make_scaffold.py`, diagram HTMLs.

## Key design facts pinned by the draft

- Safety gate P2.3 (USBPcap byte-confirm) blocks all writes; write
  examples require `--i-know-the-write-risks`; node parameter
  `i_know_the_write_risks` default false.
- rclrs caveats contained by crate boundary: core has zero rclrs deps;
  when rclrs changes, only the thin adapter crate ports.
- Test matrix: unit (golden vectors/fuzz) → replay (MockTransport with
  failure injections) → node integration (mock over rclrs) → hardware
  (#[ignore]) → release (24 h soak).
- Protocol constants table (appendix A) is exactly what becomes
  `protocol/mod.rs`; PROTOCOL.md skeleton in the scaffold mirrors it.

## Standing constraints honored

English only; zip backup in download/; chat-context backup; no full
cargo builds (scaffold verified by structure, not compiled - noted in
final reply).
