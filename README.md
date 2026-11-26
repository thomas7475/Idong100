# idong100-rs - Rust driver for the Toltech i-Dong Sensor V2.6

Native, idiomatic Rust driver with ROS 2 output for the i-Dong stereo
camera (USB 0471:2600 bootloader / 0471:2500 runtime). Design contract:
`docs/IMPLEMENTATION_DRAFT.pdf` (turn 13) in the project downloads.

## Layout (single crate, turn 37)

| Path | Duty | Depends on |
|---|---|---|
| `src/` | the whole driver: protocol core + session lifecycle + (feature `usb`) the rusb transport + the pure V4L2 sysfs matcher (`src/transport/v4l2.rs`) | thiserror, log, crossbeam-channel; rusb only with `--features usb` |
| `src/examples/` | ALL example binaries (turn 39): `list`, `probe_bootloader`, `bringup` (P2.2), `session_full` (P2.4), `video` (P2.5: V4L2), `flash_mcu` (P2.3: the 8051 firmware upload, two-key consent gate), `mcu_image`, `dump_frames` | libc + png (dev-deps, examples only) |
| `src/tests/` | golden replays: mock, runtime, video - hardware-free | - |
| `idong_msgs` | ROS message/service definitions (rosidl, colcon) | rosidl runtime |
| `idong-ros2` | ROS 2 lifecycle node: params, QoS, diagnostics | rclrs, idong, idong_msgs |

ALL code under `src/` (turn 39): the lib in `src/`, the examples in
`src/examples/`, the integration tests in `src/tests/` - one tree,
nothing outside it (explicit `[[example]]`/`[[test]]` path targets keep
every `cargo run --example ...` command unchanged). One crate, one
`Cargo.toml`, one version (turn 37: the old
`idong` + `idong-transport` workspace crates were merged - `src/transport/`
holds the rusb code behind the `usb` feature so plain `cargo test` stays
hardware- and libusb-free).

## Status (turn 39, 2026-11-27) - live-tested: session PASS, flash ships

**The wire protocol is fully decoded and implemented, and the first
LIVE bench run (Linux) is in**: T1-T5 PASS on the real sensor (82 green
before the run; 87 now) - identity EEPROM reads, the full session with
`--cmos` register writes, 6/6 polls. The run also validated the two
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
`docs/DRIVER_STATUS.md`.

Unlock kit for the vendor software (lobby/games): `tools/kit37`
(all three gates: xEyeUSB + xEyeRight + xEyeWarrningUI, with the
patched DLLs shipped).

## Status (turn 15, 2026-11-24) - Phase 1 COMPLETE

**P1 (pure core) implemented and green.** `cargo check --workspace`
clean (0 warnings), 35 tests pass (`cargo test -p idong`), rustfmt
clean. No hardware required for any test.

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
  bulk I/O, retry policy (hardware paths untested - no device attached)
- Integration: `tests/mock_replay.rs` - boot sequence shape, CRC-fail
  abort, safety-gate refusal, runtime framing round-trip

`idong-ros2` + `idong_msgs` remain separate colcon packages until P4
(they need a ROS installation for colcon-ros-cargo). Sandbox note:
never run full `cargo build` of the whole workspace - not enough RAM;
`cargo check` and single-bin builds only.

Next: P2 read-only bring-up on hardware, then the P2.3 USBPcap gate.

## Safety rules (hard constraints)

1. Read-only until the USBPcap boot transcript is byte-confirmed.
2. No speculative writes to EP 0x02; only decoded vendor sequences.
3. Firmware files sha256-verified before upload; abort on CRC mismatch.
4. NEVER write the T24C16A EEPROM (factory calibration).
5. Write-capable examples require `--i-know-the-write-risks`.

## Firmware

NOT redistributed. Users supply vendor v2.6.0.6 files; the
`FirmwareSet` verifier pins the SHA-256 hashes from the turn-12
manifest (analysis/reports/all_extracted_sha256.tsv):

- ens_code.ldr        (Blackfin BF527, 54,420 B)
- A_a_v4.4_BG0313.bit (Spartan-3 XC3S400, 235,930 B)
- app_m8051_2611.hex  (8051, 21,362 B records)

## ROS 2 build

colcon with `colcon-cargo` + `colcon-ros-cargo` plugins; rclrs from
crates.io (0.8 line); `idong_msgs` generates Rust via
rosidl_generator_rs. Custom messages: MarkerArray, HandleEvent;
service: DeviceInfo.

## License

MIT for this code. Vendor firmware/licence constraints documented in
docs/PROTOCOL.md - private interoperability research only.
