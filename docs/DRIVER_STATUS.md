# DRIVER_STATUS.md - how far the Rust driver is, what is missing

Turn 38, 2026-11-27. The one-file answer to "how far are we, what is
left". Status line: **the protocol is fully decoded (every plane
captured), the single crate implements it all with 82 tests green, and
the COMPLETE Linux bench ladder is coded - read-only identity (P2.2),
full session (P2.4), and the video bring-up (P2.5) all ship as
examples with a step-by-step test plan (`docs/LINUX_TESTS.md`). Running
T2-T7 on the bench is the next step. No protocol blocker remains.**

## The short version

| Layer | Status |
| --- | --- |
| Wire protocol (boot flash, session, EEPROM, CMOS, stream poll, UVC video) | DONE - byte-pinned from 5 captures, turn 32/33/35 |
| P1 pure core (frames, records, calibration, boot + runtime framing, video assembler) | DONE - 82 tests green, no hardware needed |
| Single-crate layout (one src/, one Cargo.toml - turn 37) | DONE |
| Session lifecycle incl. self-reboot recovery (`src/session.rs`) | DONE (core) |
| Reply-timeout fail-fast (`REPLY_EMPTY_BUDGET`, turn 38) | DONE - no more 17-min silent spins |
| P2.1 transport (rusb discovery + bulk I/O) | CODE DONE - never touched real hardware |
| Re-enumeration handling (`src/transport/reenum.rs`) | DECISION CORE DONE + unit-tested; rusb loop coded, untested on hardware |
| P2.2 live read-only bring-up (`examples/bringup.rs`) | CODE DONE - run it as bench step T3 (`docs/LINUX_TESTS.md`) |
| P2.4 full-session test (`examples/session_full.rs`, turn 38) | CODE DONE - writes gated behind `--cmos`; bench step T5 |
| P2.5 Linux video bring-up (`examples/video.rs` + `src/transport/v4l2.rs`, turn 38) | CODE DONE - V4L2 UAPI mirror const-asserted + pure sysfs matcher; bench steps T6/T7; promotion into the crate awaits the first PASS |
| P2.3 flash gate (`FLASH_CONFIRMED`) | CLOSED by evidence, OPEN for hardware replay |
| Marker/3D record decoding (xEye3Ddata 10-slot records) | PARSER DONE - never seen on the wire in ANY capture (open) |
| P4 ROS 2 node (idong-ros2, idong_msgs) | NOT STARTED (needs a ROS install) |

## Layout (turn 37: single crate)

Everything lives in one crate now - one `src/`, one `Cargo.toml`, one
version. The rusb transport is a cargo feature:

```
cargo test                      # 82 tests, hardware-free, no libusb needed
cargo test --features usb       # same + compiles the rusb transport
cargo run --features usb --example list            # T2: discovery
cargo run --features usb --example bringup         # T3: read-only bring-up
cargo run --features usb --example probe_bootloader # T4: bootloader probe
cargo run --features usb --example session_full -- --cmos  # T5: full session
cargo run --features usb --example video -- --probe       # T6: V4L2 probe
cargo run --features usb --example video -- --cmos --frames 10  # T7: video
```

The full ladder with PASS criteria and failure collection:
`docs/LINUX_TESTS.md` (turn 38).

## What exists today

- `src/protocol/runtime.rs` - the observed `7F 55` protocol: version,
  flash sequence (0101/blocks/0201/0301/0401, byte-exact from
  cap3+cap8), EEPROM reads with checksum verify (device ID, edition
  tag, calibration, codes), CMOS init (56-B + 24-B tables, 8-B
  chunks), polls, and the 0x0205 stream poll.
- `src/protocol/video.rs` - the UVC plane: 12-B payload headers,
  PTS-advance frame closure (`VideoAssembler`), two-channel 640-px
  raster split, probe/commit bodies, `StreamPollState`.
- `src/session.rs` - the lifecycle: probe phase ("2600" bootloader /
  "2619" runtime), connect (flash + re-enum hand-off signal, or
  session init), `recover()` (the self-reboot recovery - the sensor
  reboots itself every 31-157 s and the vendor engine re-flashes on
  every landing; so does the driver), `connect_readonly()` (NEW turn
  37 - the four EEPROM reads only, the P2.2 first-contact entry).
- `src/transport/reenum.rs` (NEW turn 37) - the re-enumeration
  decision core (`BusState`, `ReenumWaiter`, pure + unit-tested over
  the observed event shapes: gone -> runtime, bootloader relanding,
  timeout) and the rusb poll loop (`wait_runtime`).
- `src/transport/usb.rs` - rusb discovery + bulk I/O with the retry
  policy; `src/transport/udev.rs` - the udev rule for unprivileged
  access.
- `src/transport/v4l2.rs` (NEW turn 38) - the PURE sysfs side of the
  video backend: /dev/video* node scan, the sensor cross-match
  (usb port chain <-> video node interface dir) and fourcc formatting;
  unit-tested (the matcher rules, incl. the sibling-trap).
- `examples/session_full.rs` (NEW turn 38) - the P2.4 test: full vendor
  session (EEPROM + CMOS register tables, writes gated behind
  `--cmos`) + one poll cycle + 0x0205 probes.
- `examples/video.rs` (NEW turn 38) - the P2.5 test: `--probe` (what
  does uvcvideo see) and the full stream run (session + V4L2 mmap
  streaming + 0x0205 at 10 Hz + per-channel PNGs + raw .bin + stats).
  The V4L2 UAPI mirror is const-asserted against the kernel headers
  (any ABI drift fails the build, not the bench); kept in the example
  until the first hardware PASS promotes it behind a crate feature.
- `src/protocol/runtime.rs` - turn 38: `read_until_reply` fails FAST
  on a silent device (`REPLY_EMPTY_BUDGET`, 3 consecutive empty reads
  = ~3 s) instead of spinning 1024 x 1 s (~17 min).
- `src/calib.rs` - T24C16A EEPROM factory calibration parser
  (verified against your 24c16-dump.bin: |T| = 100 mm, R orthonormal)
  + `parse_calibration_record` for the live session read (turn 37).
- `src/{marker,records,handle,stream}.rs` - the xEye3Ddata record
  parser, marker model, handle events, reader thread.

## What is missing, in order

1. **Run the Linux bench ladder** (`docs/LINUX_TESTS.md`, turn 38):
   T2 discovery -> T3 read-only bring-up (P2.2 acceptance) -> T4
   bootloader probe -> T5 full session with `--cmos` (P2.4 acceptance)
   -> T6 V4L2 probe -> T7 the stream test (P2.5 acceptance or the
   P2.5b decision). ~20 minutes end to end; every step prints
   PASS/FAIL and what to collect on a FAIL.
2. **P2.5 promotion**: after T7 PASSes once, promote the video backend
   from `examples/video.rs` into the crate behind a feature (and wire
   `VideoSource`/`VideoAssembler` if payload-level access is ever
   needed). If T6/T7 FAIL on the uvcvideo side, the fallback is P2.5b:
   raw usbfs ISO on EP 0x85 (rusb has no ISO support - needs the
   libusb1-sys async API or raw usbfs URB ioctls).
3. **P2.4b auto-recovery**: `session_full`/`video` currently tell you
   to rerun after a self-reboot; wire the existing
   `transport::reenum` waiter into the examples' error path.
4. **P2.3 flip `FLASH_CONFIRMED`** after ONE controlled hardware flash
   replay. Optional: the vendor engine re-flashes the MCU on every
   bootloader landing anyway, so read-only sessions work indefinitely.
5. **The marker/3D plane**: NOT on the wire in any capture so far.
   The running games receive the 10-slot records over the engine pipes;
   the likely trigger is a command we have not sent or EP 0x03/0x83
   (512-B vendor endpoints, unused in every capture). A capture with a
   RUNNING GAME decides (the lobby unlock kit 37 makes the game
   runnable on Windows). The parser exists and is tested.
6. **P4 ROS 2 node** (idong-ros2 + idong_msgs): lifecycle node,
   parameters, QoS, diagnostics - blocked on a ROS 2 install, design
   already in the draft (docs/IMPLEMENTATION_DRAFT turn 13).

## Do we need more USB captures?

For the driver core: **no** - boot, session, EEPROM, CMOS, stream
poll and the video plane are all byte-pinned. Two captures would
still add value (both optional):

- a capture with a RUNNING GAME (marker records / EP 0x03/0x83
  traffic) - decides missing #5;
- a capture of the lobby actually passing the hardware test with the
  kit-37 patch applied (validates the unlock end-to-end and shows the
  MC-calibration exchange).

## Hardware notes for the bench day

- Linux sees the sensor as 0471:2600 (after power-up) or 0471:2500
  (runtime); MI_04 is the vendor interface (ep 0x02/0x82 bulk, 512-B
  maxpacket at runtime; the bootloader is 64-B maxpacket quirk).
- The bootloader maxpacket quirk (64 B at high speed) is accepted by
  rusb; transfers must be 64-B on 2600.
- The sensor self-reboots every 31-157 s (3x in cap7, 1x in cap8) -
  the session layer expects it; do not power-cycle manually.
- NEVER write the T24C16A EEPROM (factory calibration) - the driver
  has no write path to it at all.
