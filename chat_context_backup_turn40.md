# Chat context backup — turn 40 — 2026-11-26

Project: idong100 (Rust driver for the Toltech i-Dong Sensor V2.6)
Repository: https://github.com/thomas7475/Idong100 (public)
Local clone: `/home/z/my-project/work/Idong100`

## State at the end of turn 40

| Item | Value |
|---|---|
| `main` | `8de435b` — merge of `project/state-turn40` (`--no-ff`) |
| `project/state-turn40` | `3d8379f` — `state(turn 40): repository reorganized …` |
| Both branches | pushed (verified with `git ls-remote`) |
| Tests | 100 passed, 0 failed, in both feature states |
| rustfmt (nightly) | clean |
| Hardware | nothing new tested this turn (no sensor attached) |

## The request

"fetch the latest state from the repo, the folder structure of the repo
needs reorganizing the software should be in a software folder like
idong-ros2, idong_msgs, src, examples, docs, basically all our rust driver
code to get the repo clean, include the A_a_v4.4_BG0313.bit, and
app_m8051_2500.hex, app_m8051_2611.hex and any other vendor specific files
as well place them where they are correct in the file structure. the
analysis folder should be under research the same for the scripts folder.
update the software and tests to work with the new file structure, the
flash_mcu command should have the path to the hex files."

## What was done

### Layout (all moves with `git mv`, history kept)

```
software/        Cargo.toml, rustfmt.toml, src/, docs/, idong-ros2/, idong_msgs/, firmware/   (was the repo root)
research/analysis/   (was analysis/)
research/scripts/    (was scripts/)
tools/, deliverables/ stay at the root
```

Run every cargo command from `software/`.

### Vendor files: `software/firmware/` (24 files, byte-exact)

- `v2.6.0.6/`: `A_a_v4.4_BG0313.bit` (235,930 B), `ens_code.ldr` (54,420),
  `app_m8051_2611.hex` (61,756), `app_m8051_2500.hex` (61,031),
  `xeyez_v20.hex`, `xeyez_v21.hex`, `licence.txt`, `config/`
  (`xeyezHal.ini`, `xEyeUsb.ini`, `DriverConfig.ini`, 12 register tables in
  `30fps/` and `30fps_1/`).
- `v2.5.0.7/`: the only two files that differ there (`app_m8051.hex`,
  `config/xeyezHal.ini`).
- `README.md` (provenance, roles, vendor file map, licence summary) and
  `SHA256SUMS` (24 lines).
- Checks done: `xeyez_alone` == `xeyez_Lobby` byte for byte; every file ==
  the turn-12 manifest `research/analysis/reports/all_extracted_sha256.tsv`;
  every git index blob == `SHA256SUMS`; `sha256sum -c` 24/24 OK.
- `.gitattributes`: `software/firmware/** -text` (hex files are CRLF; the
  SHA-256 pins cover those exact bytes).
- Not included: vendor program code (DLL/EXE), the Windows driver package
  (`xeyez.sys`, `xeyez.inf`), app-only configs.

### Code

- New `software/src/firmware.rs`: `firmware_dir_candidates`,
  `firmware_dir`, `resolve_hex`, `read_hex`, `load_set`, `sha256_hex`;
  constants `VENDOR_VERSION`, `FIRMWARE_DIR`, `LDR_FILE`, `BIT_FILE`,
  `HEX_FILE`. Lookup order: crate folder (compiled in), the program's parent
  folders, the working directory.
- New error variant `IdongError::FirmwareFile { path, reason }`.
- `flash_mcu`: no `--hex` needed; default
  `firmware/v2.6.0.6/app_m8051_2611.hex`; `--hex <path-or-name>` (a bare
  name is looked up in the folder); the two SHA-256 pins refuse every other
  image before a byte leaves the host.
- `mcu_image` uses the same default. `bringup` help text updated.
- `software/idong-ros2/Cargo.toml`: `idong = { path = ".." }` (was
  `"../.."`).
- New integration test `software/src/tests/firmware_files.rs` (8 tests).

### Docs

`README.md` and `.state-README.md` rewritten; `software/docs/PROTOCOL.md`,
`LINUX_TESTS.md`, `BRINGUP.md`, `DRIVER_STATUS.md` updated;
`SOLVE_DIALOG.md` got a layout note (byte-exact edit, mixed line endings
kept); `research/idong100/README.md` got a turn-40 note.

### Research scripts

13 Python scripts + `depth_report.js` now derive `research/analysis` and the
repo root from their own location (`ANALYSIS`, `REPO`, `_HERE`; override
with the environment variable `IDONG_ANALYSIS`). `lint_kit37.py` passes.

## Verified output of the new default

```
cd software
cargo run --features usb --example flash_mcu
  hex file : .../software/firmware/v2.6.0.6/app_m8051_2611.hex (61756 B)
  PASS: raw hex matches the turn-12 manifest pin
  PASS: flattened image 21411 B (0x53A3), pin matches
  PASS: dry run complete (firmware verified, gate closed).
```

`--hex app_m8051_2500.hex` is refused (exit 1); `--hex nope.hex` names the
places looked at (exit 1).

## Decisions made without asking

1. `examples/` and `tests/` stay under `software/src/` (the turn-39 layout).
2. Firmware folder holds the device-side vendor files only.
3. `tools/` and `deliverables/` stay at the root.
4. The frozen drafts `deliverables/idong100-rs-scaffold/` and
   `research/idong100/rust/idong/` stay where they were, labelled historical.
5. `v2.5.0.7/` holds only the files that differ from `v2.6.0.6/`.
6. `--hex` is optional; a bare file name resolves inside `firmware/v2.6.0.6`.

## Flags for the user

- The repository is public: the vendor firmware files are now world-readable
  together with everything else already in the repo.
- The vendor's terms ship as `software/firmware/v2.6.0.6/licence.txt`
  (clause 1: no separate copying/modifying; clause 2: no reverse
  engineering; clause 3: freeware, non-commercial use/distribution,
  commercial use needs written permission).
- A GitHub PAT was pasted into the chat; rotate it.

## Open items (unchanged from turn 39)

- Bench: rerun T6/T7 (video), run T8 (`flash_mcu -- --confirm`) — one clean
  T8 PASS flips `FLASH_CONFIRMED`.
- P2.5 promotion, P2.4b auto-recovery, the marker/3D plane, P4 ROS 2 node —
  see `software/docs/DRIVER_STATUS.md`.

## Working notes for the next session

- Toolchain env: `. /home/z/my-project/work/rust/env40.sh`, then run cargo
  from `/home/z/my-project/work/Idong100/software`.
- Chat backups live on the `chat/context` branch as
  `chat_context_backup_turnNN.md`.
- Scripts of this turn (agent workspace, not in the repo):
  `/home/z/my-project/scripts/populate_firmware40.py`,
  `/home/z/my-project/scripts/reroot_scripts40.py`,
  `/home/z/my-project/scripts/solve_dialog_note40.py`.
