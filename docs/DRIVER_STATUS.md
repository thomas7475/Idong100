# DRIVER_STATUS.md - how far the Rust driver is, what is missing

Turn 37, 2026-11-27. The one-file answer to "how far are we, what is
left". Status line: **the protocol is fully decoded (every plane
captured), the single crate implements it all with 72 tests green, and
the read-only hardware bring-up (P2.2) is coded and ready to run on
your bench - that run is the next step. No protocol blocker remains.**

## The short version

| Layer | Status |
| --- | --- |
| Wire protocol (boot flash, session, EEPROM, CMOS, stream poll, UVC video) | DONE - byte-pinned from 5 captures, turn 32/33/35 |
| P1 pure core (frames, records, calibration, boot + runtime framing, video assembler) | DONE - 72 tests green, no hardware needed |
| Single-crate layout (one src/, one Cargo.toml - turn 37) | DONE |
| Session lifecycle incl. self-reboot recovery (`src/session.rs`) | DONE (core) |
| P2.1 transport (rusb discovery + bulk I/O) | CODE DONE - never touched real hardware |
| Re-enumeration handling (`src/transport/reenum.rs` - turn 37) | DECISION CORE DONE + unit-tested; rusb loop coded, untested on hardware |
| P2.2 live read-only bring-up (`Session::connect_readonly` + `examples/bringup.rs`) | CODE DONE - **the next step is running it on your bench** (`docs/BRINGUP.md`) |
| P2.3 flash gate (`FLASH_CONFIRMED`) | CLOSED by evidence, OPEN for hardware replay |
| P2.5 video backend (V4L2 / usbfs ISO for EP 0x85) | SEAM DONE (`VideoSource`), backend missing |
| Marker/3D record decoding (xEye3Ddata 10-slot records) | PARSER DONE - never seen on the wire in ANY capture (open) |
| P4 ROS 2 node (idong-ros2, idong_msgs) | NOT STARTED (needs a ROS install) |

## Layout (turn 37: single crate)

Everything lives in one crate now - one `src/`, one `Cargo.toml`, one
version. The rusb transport is a cargo feature:

```
cargo test                      # 72 tests, hardware-free, no libusb needed
cargo test --features usb       # same + compiles the rusb transport
cargo run --features usb --example list            # discovery (read-only)
cargo run --features usb --example probe_bootloader  # version probe (read-only)
cargo run --features usb --example bringup         # THE P2.2 hardware test
```

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
- `src/calib.rs` - T24C16A EEPROM factory calibration parser
  (verified against your 24c16-dump.bin: |T| = 100 mm, R orthonormal)
  + `parse_calibration_record` for the live session read (turn 37).
- `src/{marker,records,handle,stream}.rs` - the xEye3Ddata record
  parser, marker model, handle events, reader thread.

## What is missing, in order

1. **P2.2 run the read-only bring-up on the bench** (~10 minutes):
   boot a Linux box/live-USB with the sensor attached, install the
   udev rule, then `cargo run --features usb --example bringup`.
   Acceptance: version "2619", the 32-hex device ID, 4-char codes,
   120-B calibration that parses (baseline ~100 mm, focal ~432 px).
   Every byte on the wire is an EEPROM read the vendor engine also
   sends - zero write risk. Full procedure: `docs/BRINGUP.md`.
2. **P2.3 flip `FLASH_CONFIRMED`** after ONE controlled hardware flash
   replay. Optional for now: the vendor engine keeps re-flashing the
   MCU on every bootloader landing anyway, so the driver can run
   read-only sessions on the runtime device indefinitely.
3. **P2.5 video backend**: `VideoSource` is the seam; implement the
   Linux side - either V4L2 (the standard path for the UVC MI_01; the
   probe/commit values are pinned in `video_golden.rs`) or raw usbfs
   ISO on EP 0x85 after SET_INTERFACE alt-1. Then `VideoAssembler`
   feeds PNG/video out; channel A is the scene image, channel B is
   still unidentified (second lens / FPGA intermediate - open).
4. **The marker/3D plane**: NOT on the wire in any capture so far.
   The vendor app streams raw video + polls 0x0205; the 10-slot 88-B
   marker records never appeared - the running games receive them
   over the engine pipes, so the likely trigger is a command we have
   not sent or EP 0x03/0x83 (512-B vendor endpoints, unused in every
   capture). A capture with a RUNNING GAME decides. The parser for
   the record layout exists and is tested - only the trigger is
   unknown.
5. **P4 ROS 2 node** (idong-ros2 + idong_msgs): lifecycle node,
   parameters, QoS, diagnostics - blocked on a ROS 2 install, design
   already in the draft (docs/IMPLEMENTATION_DRAFT turn 13).

## Do we need more USB captures?

For the driver core: **no** - boot, session, EEPROM, CMOS, stream
poll and the video plane are all byte-pinned. Two captures would
still add value (both optional):

- a capture with a RUNNING GAME (marker records / EP 0x03/0x83
  traffic) - decides missing #4;
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
