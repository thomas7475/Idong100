# idong100 - Rust driver for the Toltech i-Dong Sensor V2.6

Native, idiomatic Rust driver with ROS 2 output for the i-Dong stereo
camera (USB 0471:2600 bootloader / 0471:2500 runtime). All driver code
lives under `software/` (turn 40); `research/` holds the work the driver
grew out of. The design contract is the turn-13 implementation draft
(`deliverables/idong100_RUST_ROS2_DRIVER_DRAFT.pdf`); the wire facts are
`software/docs/PROTOCOL.md`.

## Layout (turn 40)

```
software/                the driver - run every cargo command from here
  Cargo.toml             one crate (`idong`), rustfmt.toml
  src/                   protocol core, session lifecycle, transport
  src/examples/          the bench and flash examples
  src/tests/             hardware-free golden replays + firmware_files
  firmware/              the vendor device files (v2.6.0.6, v2.5.0.7)
  idong_msgs/            ROS message and service definitions
  idong-ros2/            ROS 2 lifecycle node
  docs/                  PROTOCOL, BRINGUP, DRIVER_STATUS, LINUX_TESTS,
                         CAPTURE_KIT, SOLVE_DIALOG
research/
  idong100/              findings per turn, CONTEXT.md, SUMMARY.md
  analysis/reports/      extraction and analysis outputs
  scripts/               research tools
tools/                   Windows bench kits: capture/, kit36/, kit37/
deliverables/            PDF reports (+ the turn-13 scaffold, historical)
```

| Path | Duty | Depends on |
|---|---|---|
| `software/src/` | the whole driver: protocol core + session lifecycle + (feature `usb`) the rusb transport + the V4L2 sysfs matcher (`src/transport/v4l2.rs`) + the V4L2 ioctl engine (`src/transport/v4l2_stream.rs`) + the usbfs ISO engine (`src/transport/usbfs_iso.rs`) + the firmware folder lookup (`src/firmware.rs`) | thiserror, log, crossbeam-channel; rusb + libc only with `--features usb` |
| `software/src/examples/` | ALL example binaries (turn 39): `list`, `probe_bootloader`, `bringup` (P2.2), `session_full` (P2.4), `video` (P2.5: V4L2 + `--iso` raw + `--recover` self-healing, turn 44), `flash_mcu` (P2.3: the 8051 firmware upload, two-key consent gate), `mcu_image`, `dump_frames` | png (dev-dep, examples only) |
| `software/src/tests/` | golden replays: mock, runtime, video, plus `firmware_files` (the shipped vendor files) - hardware-free | - |
| `software/firmware/` | the vendor device files the driver targets: the three pinned images (`ens_code.ldr`, `A_a_v4.4_BG0313.bit`, `app_m8051_2611.hex`), `app_m8051_2500.hex` and the other MCU images, the vendor file map and register tables; `README.md` + `SHA256SUMS` | - |
| `software/docs/` | PROTOCOL (wire facts), BRINGUP, LINUX_TESTS, DRIVER_STATUS, CAPTURE_KIT, SOLVE_DIALOG | - |
| `software/idong_msgs` | ROS message/service definitions (rosidl, colcon) | rosidl runtime |
| `software/idong-ros2` | ROS 2 lifecycle node: params, QoS, diagnostics | rclrs, idong, idong_msgs |
| `research/` | `idong100/` findings per turn, `analysis/reports/` outputs (incl. the turn-12 SHA-256 manifest), `scripts/` research tools | - |
| `tools/` | the Windows bench kits: `capture/`, `kit36/`, `kit37/` (the vendor-software unlock kit) | - |
| `deliverables/` | the PDF reports | - |

ALL code under `software/src/` (turn 39): the lib in `src/`, the examples
in `src/examples/`, the integration tests in `src/tests/` - one tree,
nothing outside it (explicit `[[example]]`/`[[test]]` path targets keep
every `cargo run --example ...` command unchanged). One crate, one
`Cargo.toml`, one version (turn 37: the old `idong` + `idong-transport`
workspace crates were merged - `src/transport/` holds the rusb code
behind the `usb` feature so plain `cargo test` stays hardware- and
libusb-free). `idong-ros2` and `idong_msgs` are separate colcon packages
next to the crate.

## Build and test

```
cd software
cargo test                                   # 108 tests, hardware-free
cargo test --features usb                    # the same + the rusb transport + the ioctl engines
cargo +nightly fmt --check
cargo run --features usb --example flash_mcu                # T8 dry run: pins only, no device
cargo run --features usb --example flash_mcu -- --confirm   # T8: the controlled flash
```

The bench ladder T2-T10 with PASS criteria: `software/docs/LINUX_TESTS.md`.

## Status (turn 44, 2026-11-27) - the bench ladder completes: T9 + T7 PASS, the backends are in the crate, the run self-heals

The turn-44 bench day decided everything the ladder had left:

- **T9 PASS** (the raw usbfs-ISO rerun with the turn-43
  SET_INTERFACE fix): MI_01 taken from uvcvideo, probe/commit PASS,
  alt 2 armed, 1281 payloads / 0 errors, frames closed per PTS.
- **T7 PASS** (the surprise): uvcvideo STREAMON - the turn-41
  "documented dead end" EIO - streamed 10 real frames right after the
  T8 re-flash. The EIO is state-dependent; the deterministic path
  stays `--iso`.
- **An assembler bug found and fixed**: the firmware flags the FINAL
  payload of every frame with EOF, and that payload carries the
  frame's tail (the bench showed every frame split into a 612000-B
  body + a 2400-B tail under one PTS = exactly 614400 B together).
  `VideoAssembler` now closes the frame WITH the EOF payload's data;
  reruns deliver one full 640x480 frame per PTS.
- **P2.5 promotion**: both ioctl engines moved from the example into
  the crate behind `usb` + linux (`src/transport/v4l2_stream.rs`,
  `src/transport/usbfs_iso.rs`; `libc` an optional regular
  dependency). The example is the bench script on top of them.
- **P2.4b auto-recovery**: `video --recover` absorbs the sensor's
  documented 31-157 s self-reboot (the reset-to-bootloader the bench
  saw after every video run): wait for the bootloader landing,
  re-flash the pinned image (the ungated `FLASH_CONFIRMED` path),
  wait for the renum, re-init, RESUME - up to 12 landings per run.
  108 tests green in both feature states (the 2 new ones pin the
  EOF/error payload semantics).

Next: the frame-rate A/B (the raw path produced ~1.8 fps under the
  golden probe/commit vs ~25 fps device-side on uvcvideo's own
  negotiation - see `docs/LINUX_TESTS.md`), the marker/3D record
  capture (needs a running game), then P4 ROS 2 -
  `software/docs/DRIVER_STATUS.md`.

## Status (turn 43, 2026-11-27) - P2.5b ISO: the first bench run decides the alt question

The turn-42 T9 run (`video --iso --cmos`, mirroring the vendor
capture's no-SET_INTERFACE flow) took MI_01 from uvcvideo, PASSed the
observed UVC probe/commit, and then failed at the first SUBMITURB
with `No such file or directory (os error 2)`: ENOENT, not a missing
file - Linux arms an ISO endpoint only in the selected alt setting,
and at alt 0 the host controller has no transfer ring for EP 0x85.
Turn 43 makes `USBDEVFS_SETINTERFACE` (streaming alt 2 =
2412 B/microframe) the default step before the ISO ring, adds
`--alt N` (the 812 B alt 1) and `--no-set-alt` (the A/B control),
and restores alt 0 + releases the claim on teardown. 106 tests
green in both feature states. Next: the T9 rerun -
`software/docs/LINUX_TESTS.md`.

## Status (turn 40, 2026-11-26) - repository reorganized

All driver code moved under `software/` (with `git mv`, history kept),
`analysis/` and `scripts/` under `research/`. The vendor device files
now ship with the crate in `software/firmware/` - 24 byte-exact files
from the vendor v2.6.0.6 and v2.5.0.7 packages, checked by
`SHA256SUMS` and by the new `firmware_files` test against the turn-12
manifest. `flash_mcu` reads `firmware/v2.6.0.6/app_m8051_2611.hex` by
default; `--hex <path-or-name>` selects another copy, and the SHA-256
pins still refuse any other image. 100 tests green in both feature
states. No protocol change; nothing new was tested on hardware this
turn.

## Status (turn 39, 2026-11-27) - live-tested: session PASS, flash ships

**The wire protocol is fully decoded and implemented, and the first
LIVE bench run (Linux) is in**: T1-T5 PASS on the real sensor (82 green
before the run; 87 at turn 39) - identity EEPROM reads, the full session
with `--cmos` register writes, 6/6 polls. The run also validated the two
turn-39 fixes: the calibration payload parser (the "need 121, have
120" parse FAIL) and the sysfs hex-id cross-match (the video probe's
empty `sensor usb ports` list - sysfs ids are HEX, not decimal).
Turn 39 also moved ALL code under `src/` (lib + `src/examples/` +
`src/tests/`, one tree) and shipped the 8051 firmware upload as the
P2.3 controlled replay (`src/examples/flash_mcu.rs`): dry-run pins
(both SHA-256s) + a two-key consent gate (`--confirm` + the
unforgeable `FlashPermit`), the exact cap3 flash with full-image
readback abort, bootloader-window wait, reenum + runtime proof. Next:
rerun T6/T7 (video), run T8 (flash) - one clean T8 PASS flips
`FLASH_CONFIRMED`. Then the marker-record trigger and ROS 2 - see
`software/docs/DRIVER_STATUS.md`.

Unlock kit for the vendor software (lobby/games): `tools/kit37`
(all three gates: xEyeUSB + xEyeRight + xEyeWarrningUI, with the
patched DLLs shipped).

## Status (turn 15, 2026-11-24) - Phase 1 COMPLETE

**P1 (pure core) implemented and green.** `cargo check` clean (0
warnings), 35 tests pass, rustfmt clean. No hardware required for any
test.

- P0.1 `docs/PROTOCOL.md` - full wire-facts table with citations
- P1.1 error model + `protocol/mod.rs` constants (every value cited)
- P1.2 `frames.rs` PU/OD codec + resync decoder; `records.rs`
  xEye3Ddata parser (10 x 88-B slots, +0x3B8 mode flag, quality >= 50)
- P1.3 `calib.rs` T24C16A EEPROM parser - checksums verified, reference
  values asserted (|T| = 100 mm, mean fx 432.33 px, R orthonormal)
- P1.4 `boot.rs` `_InitBlobUpload` state machine (safety-gated behind
  `BOOT_CONFIRMED`); `cmd.rs` 7F 55 runtime framing with byte-swapped
  u16 arg - only vendor-decoded commands representable
- P1.5 `stream.rs` reader thread, bounded channel, drop-oldest
  backpressure with exact overflow accounting; `FirmwareSet` SHA-256
  pins (verified against FIPS 180-4 vectors)
- `src/transport/usb.rs` P2.1 read-only implementation: discovery,
  bulk I/O, retry policy
- Integration: `src/tests/mock_replay.rs` - boot sequence shape,
  CRC-fail abort, safety-gate refusal, runtime framing round-trip

`idong-ros2` + `idong_msgs` remain separate colcon packages until P4
(they need a ROS installation for colcon-ros-cargo).

## Safety rules (hard constraints)

1. Read-only until the USBPcap boot transcript is byte-confirmed.
2. No speculative writes to EP 0x02; only decoded vendor sequences.
3. Firmware files sha256-verified before upload; abort on CRC mismatch.
4. NEVER write the T24C16A EEPROM (factory calibration).
5. Write-capable examples require `--i-know-the-write-risks`.

## Firmware

The vendor device files the driver targets ship with the crate in
`software/firmware/` (turn 40): byte-exact copies from the vendor
v2.6.0.6 package, plus the two files that differ in v2.5.0.7. Every
hash is in `software/firmware/SHA256SUMS`
(`sha256sum -c SHA256SUMS`), provenance, roles and the vendor file map
are in `software/firmware/README.md`, and `cargo test --test
firmware_files` re-checks them against the turn-12 manifest
(`research/analysis/reports/all_extracted_sha256.tsv`) and the pins in
the crate.

- `v2.6.0.6/ens_code.ldr`         Blackfin BF527 loader image, 54,420 B (pinned)
- `v2.6.0.6/A_a_v4.4_BG0313.bit`  Xilinx Spartan-3A XC3S400A bitstream, 235,930 B (pinned)
- `v2.6.0.6/app_m8051_2611.hex`   8051 image, 61,756 B -> 21,411 B flattened (pinned; what `flash_mcu` sends)
- `v2.6.0.6/app_m8051_2500.hex`   the `[Version2.5]` 8051 image of the same package, 61,031 B
- `v2.6.0.6/xeyez_v20.hex`, `xeyez_v21.hex`  the `[Version2.0]` / `[Version2.1]` 8051 images
- `v2.6.0.6/config/`              the vendor file map (`xeyezHal.ini`, `xEyeUsb.ini`) and the register tables
- `v2.5.0.7/`                     `app_m8051.hex` and `xeyezHal.ini` of the previous package

`flash_mcu` reads `firmware/v2.6.0.6/app_m8051_2611.hex` by default
(`src/firmware.rs` locates the folder).

## Historical copies (not the driver)

Two frozen snapshots of earlier driver drafts stay where they were
archived; the driver is only `software/`:

- `research/idong100/rust/idong/` - the turn-10 reference crate (the
  first Rust draft);
- `deliverables/idong100-rs-scaffold/` - the turn-13 workspace scaffold
  (45 files) that the P1 crate grew out of.

## ROS 2 build

colcon with `colcon-cargo` + `colcon-ros-cargo` plugins; rclrs from
crates.io (0.8 line); `idong_msgs` generates Rust via
rosidl_generator_rs. Custom messages: MarkerArray, HandleEvent;
service: DeviceInfo. Both packages sit next to the crate in
`software/`; `idong-ros2` reaches the crate by relative path
(`idong = { path = ".." }`). Plain cargo builds neither.

## License

MIT for the code in `software/`. The vendor device files under
`software/firmware/` are not covered by it: they are the vendor's
property and ship under the vendor's own terms
(`software/firmware/v2.6.0.6/licence.txt`, summarised in
`software/firmware/README.md`).
