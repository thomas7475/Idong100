# idong100-rs - Rust driver for the Toltech i-Dong Sensor V2.6

Native, idiomatic Rust driver with ROS 2 output for the i-Dong stereo
camera (USB 0471:2600 bootloader / 0471:2500 runtime). Design contract:
`docs/IMPLEMENTATION_DRAFT.pdf` (turn 13) in the project downloads.

## Layout (single crate, turn 37)

| Path | Duty | Depends on |
|---|---|---|
| `src/` | the whole driver: protocol core + session lifecycle + (feature `usb`) the rusb transport | thiserror, log, crossbeam-channel; rusb only with `--features usb` |
| `examples/` | `list`, `probe_bootloader`, `bringup` (the P2.2 hardware test) - feature `usb` | - |
| `tests/` | golden replays: mock, runtime, video - hardware-free | - |
| `idong_msgs` | ROS message/service definitions (rosidl, colcon) | rosidl runtime |
| `idong-ros2` | ROS 2 lifecycle node: params, QoS, diagnostics | rclrs, idong, idong_msgs |

One crate, one `Cargo.toml`, one version (turn 37: the old
`idong` + `idong-transport` workspace crates were merged - `src/transport/`
holds the rusb code behind the `usb` feature so plain `cargo test` stays
hardware- and libusb-free).

## Status (turn 37, 2026-11-27) - single crate + P2.2 bring-up ready

**The wire protocol is fully decoded and implemented.** All planes
byte-pinned from five captures (turn 32/33/35): bootloader flash
(0008/0101/64-B blocks/0201/0301/0401), session (EEPROM reads with
checksums, CMOS register tables, polls), the engine gates (device ID,
edition tag), the 0x0205 stream poll, and the UVC video plane
(probe/commit, 12-B payload headers, PTS frame closure, two-channel
raster). `cargo test` (and `--features usb`): 72 green. Turn 36 added
the session lifecycle with self-reboot recovery; turn 37 merged the
crates, implemented the re-enumeration watcher (`src/transport/reenum.rs`),
the read-only bring-up entry point (`Session::connect_readonly`) and
the hardware test (`examples/bringup.rs`, `docs/BRINGUP.md`). What
remains: run the bring-up on the real sensor, the video backend, the
marker-record trigger and ROS 2 - see `docs/DRIVER_STATUS.md`.

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
