# ZCode Chat Context Backup — 2026-11-24 (turn 13)

**How to use this file:** attach/upload it in a future session so the
assistant can restore full context of this conversation and continue
where it left off. This file was written by the assistant itself as a
session snapshot.

---

## User's standing rules (apply to EVERY reply from now on)

1. **English only** for all communication.
2. **Zip backup of generated files** must be placed in
   `/home/z/my-project/download/` every time files are produced.
3. **Chat-context backup** (this kind of file) must be produced every
   time as well, downloadable from `/home/z/my-project/download/`.
4. **NEVER run full `cargo` builds** for the complete project — not
   enough RAM. Use `cargo check`, single-bin/example builds, or lowered
   parallelism.

## Turn 13 request and outcome

Q: "lets start with the implementation of a native idiomatic rust
driver for the camera with ROS 2 compatible output, do research for a
good easy to maintain standard driver and file structure, make a draft
on how to implement it with a overview of the driver structure and
steps to implement it."

**Done — all parts:**
- Research verified 2026-11: rclrs 0.5/0.8 official Rust client library
  (async, Windows, Jazzy/Kilted/Rolling, crates.io; **no managed
  lifecycle nodes, no API stability guarantees**);
  rosidl_generator_rs on the ROS buildfarm; colcon-cargo +
  colcon-ros-cargo plugins, package.xml `ament_cargo` build type.
  Benchmarks: ros2_ouster design doc, "What makes a good sensor
  package" thread (t/38805), xtion-rs.
- **PDF**: `download/idong100_RUST_ROS2_DRIVER_DRAFT.pdf` (15 pp, cover
  Template 07 Crystal Blue, TOC, 10 chapters + app A constants + app B
  manifests, 2 figures, 8 tables; pdf_qa PASS with 2 benign
  nested-callout centering warnings; toc.check pass; placeholder scan
  clean; page images verified programmatically - this model has NO
  image input).
- **Scaffold**: `download/idong100-rs-scaffold/` (45 files) +
  `download/idong100_rust_scaffold.zip`: 4-crate workspace exactly as
  PDF section 4 (idong pure core / idong-transport rusb / idong_msgs
  rosidl / idong-ros2 rclrs), doc-comment stubs citing disassembly
  sources, PROTOCOL.md skeleton, README with safety rules, launch +
  params.yaml. NOT compiled (no cargo in sandbox this session - rule 4;
  structure verified by review).
- Research tree: `research/idong100/agents/13-rust-ros2-driver-draft/
  FINDINGS.md`.
- Zip backup: `download/idong100_research_turn13_2026-11-24.zip`
  (PDF + scaffold zip + FINDINGS + this backup).
- Scripts (rebuild pipeline): `scripts/gen_driver_report.py` (body,
  ReportLab, TocDocTemplate + multiBuild, roman TOC/arabic body via
  `_body_start` marker) → `scripts/merge_driver_report.py` (cover via
  html2poster.js --width 794px + pypdf normalize to A4) → meta.brand →
  pdf_qa.py. Diagrams: `scripts/diagram_driver_ws.html`,
  `diagram_roadmap.html` → `scripts/make_driver_figures.py`
  (playwright screenshot @2x).

## Key knowledge added this turn (driver design facts)

- **Four-crate split is THE decision**: `idong` core has zero rusb AND
  zero rclrs deps (golden vectors + MockTransport = full CI without
  hardware or ROS); `idong-ros2` is the only crate seeing rclrs, so
  rclrs API churn is contained; `idong_msgs` is plain rosidl files.
- **Node surface**: /idong/markers (sensor-data QoS best-effort
  keep-last 5, custom MarkerArray msg), /idong/events (reliable),
  /idong/diagnostics, /idong/time_ref (frame-counter/host-clock pairs
  for drift estimation - no PTP), get_device_info service;
  ~/transition service implements lifecycle semantics internally
  (rclrs lacks managed nodes) — isolated in lifecycle.rs.
- **Params** (table 5): serial_number, frame_id, publish_hz_min=30,
  quality_threshold=50 (vendor gate), link_quality_window=200
  (RingSize), buffer_depth=5, firmware_dir, enable_handle_events=true,
  i_know_the_write_risks=false (write gate).
- **Threading**: one reader thread in core (bulk reads → resync decode
  → keep-alive inline → bounded crossbeam channel, drop-oldest) + rclrs
  executor thread draining it; config via Arc<Mutex> snapshot per frame.
- **Roadmap P0-P4** with the single write gate at P2.3 (USBPcap
  byte-confirm of _InitBlobUpload) before ANY hardware writes.
- **Idioms checklist** (table 6): thiserror + BootStep enums; total
  decode (never panic on device bytes); enum state machines;
  crossbeam bounded channels; small Transport trait + MockTransport;
  fuzz targets for decoders; #![deny(unsafe_code)] in core; log facade
  in core, ROS sink only in node.
- Scaffold paths: `download/idong100-rs-scaffold/{Cargo.toml,
  README.md, docs/PROTOCOL.md, idong/{src,examples,tests},
  idong-transport/src, idong_msgs/{msg,srv,package.xml,CMakeLists.txt},
  idong-ros2/{src,launch,params.yaml,package.xml}}`.

## Continuity from turn 12 (still in force)

- Full protocol map in `upload/RUST_DRIVER_DRAFT-v2.md` (turn-11 draft,
  826 lines) and turn-12 FINDINGS
  (`research/idong100/agents/12-extraction-sha/FINDINGS.md`): PU/OD
  boot framing, 7F 55 runtime framing, cmd 3/5/6/15, xEye3Ddata
  0x3BC layout, T24C16A EEPROM layout, register scripts (0x02=
  0x0A blob / 0x0B image mode), firmware sha256 manifest
  `analysis/reports/all_extracted_sha256.tsv`.
- Analysis tree intact: analysis/nsis (both driver gens),
  analysis/extracted_sportsking, analysis/remote, analysis/reports.
- pefile/capstone need `pip install` after sandbox recycle; 7zz at
  /tmp/7zip/7zz (re-download from GitHub ip7z/7zip 24.08 if gone).
- Safety rules in force: read-only EP 0x82 until USBPcap gate; no
  speculative EP 0x02 writes; NEVER write the T24C16A EEPROM; no full
  cargo builds.
- GitHub repo thomas7475/Idong100 cloned earlier (public); local
  research tree is AHEAD (agents 01/02/09/10/12/13); never pushed -
  ask user first.

## Status / likely next turns

1. P0.1: fill `docs/PROTOCOL.md` from turn-12 FINDINGS (it is a
   skeleton with the tables already in place).
2. P1.1-P1.5: implement the pure core (frames/records/calib/boot/cmd/
   stream) with golden vectors; cargo check only.
3. P2.3 gate: USBPcap capture of one vendor Windows boot (user-side).
4. Optional: 8051 disasm (app_m8051_2611.hex), Blackfin LDR framing,
   Toltech SDK email thread (serial AB460DC8BA12C6840B49CD5786C329DD).
