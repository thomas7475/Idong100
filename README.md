# Idong100 — i-Dong Sensor V2.6 Research & Rust Driver Project

Reverse-engineering the Toltech **i-Dong Sensor V2.6** stereo tracking
camera (USB `0471:2600` bootloader → `0471:2500` runtime) and building a
native, idiomatic **Rust driver with ROS 2-compatible output**.

## Branch layout

| Branch | Purpose | Updated |
|---|---|---|
| `main` | Base repo (README only) | on demand |
| `research/idong100-01…` | Frozen per-agent research history (turns 1–9 era) | archived |
| `project/state` | **Current project state**: research tree, analysis reports, scripts, deliverables (PDFs, Rust scaffold) | every turn |
| `chat/context` | **Chat context backups**: session snapshots so any future session can restore full conversation state | every turn |

> Working rule (turn 14, 2026-11-24): keep `project/state` and
> `chat/context` as two dedicated branches; both are updated at the end
> of every session.

## Current state (turn 14, 2026-11-24)

- Driver design finalized in
  `deliverables/idong100_RUST_ROS2_DRIVER_DRAFT.pdf` (turn 13):
  four-crate workspace — `idong` (pure protocol core, no rusb/rclrs),
  `idong-transport` (rusb), `idong_msgs` (rosidl), `idong-ros2` (rclrs).
- Rust scaffold in `deliverables/idong100-rs-scaffold/` (45 files,
  doc-comment stubs citing disassembly sources).
- Phase 0 of the implementation roadmap is this git setup; next is
  P1 (pure core with golden vectors) — `cargo check` only, never full
  builds (RAM limit).
- Research knowledge: `research/idong100/` (agents 01–13 FINDINGS),
  protocol constants in `research/idong100/agents/13-rust-ros2-driver-draft/FINDINGS.md`
  and the draft PDF appendix A.

## Safety rules (standing)

1. Read-only USB probing until the USBPcap boot transcript is
   byte-confirmed (gate P2.3).
2. No speculative writes to endpoint 0x02 — only decoded vendor
   sequences.
3. Firmware files SHA-256-verified before upload; abort on CRC mismatch.
4. NEVER write the T24C16A EEPROM (factory calibration).
5. Write-capable examples require `--i-know-the-write-risks`.
6. No vendor firmware is redistributed in this repository — users
   supply their own files, verified by hash
   (`analysis/reports/all_extracted_sha256.tsv`).

## Repository structure (project/state)

```
research/idong100/           # per-turn FINDINGS + artifacts (agents 01-13)
analysis/reports/            # extraction manifests + PE/firmware analysis
scripts/                     # extraction & analysis tools (python/js)
deliverables/                # PDF reports + Rust scaffold (turns 12-13)
```
