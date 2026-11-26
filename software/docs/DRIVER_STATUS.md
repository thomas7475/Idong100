# DRIVER_STATUS.md - how far the Rust driver is, what is missing

Turn 40, 2026-11-26. The one-file answer to "how far are we, what is
left". Status line: **the protocol is fully decoded (every plane
captured), the driver lives under `software/` of the repository (turn
40: the crate with the vendor device files in `firmware/`, the ROS 2
packages and `docs/` next to it; all code under `src/` since turn 39),
100 tests green, and the bench ladder
T1-T8 passed its first LIVE run on Linux: T1-T5 PASS (identity, session,
CMOS writes, polls), the video probe (T6) listed the UVC nodes - the
turn-39 fixes (sysfs hex parse, calibration payload parser) clear the
two reported FAILs - and the 8051 firmware upload ships as the T8
controlled replay (`src/examples/flash_mcu.rs`, two-key consent gate),
now reading the hex that ships with the crate.
Next: rerun T6/T7 for the video plane, run T8 for the P2.3 flip. No
protocol blocker remains.**

## The short version

| Layer | Status |
| --- | --- |
| Wire protocol (boot flash, session, EEPROM, CMOS, stream poll, UVC video) | DONE - byte-pinned from 5 captures, turn 32/33/35 |
| P1 pure core (frames, records, calibration, boot + runtime framing, video assembler) | DONE - 100 tests green, no hardware needed |
| All-code-in-src/ layout (lib + src/examples/ + src/tests/ - turn 39) | DONE - explicit `[[example]]`/`[[test]]` path targets, commands unchanged |
| Repository layout (turn 40: `software/` = crate + `firmware/` + `idong-ros2` + `idong_msgs` + `docs/`) | DONE - the 24 vendor device files ship in `firmware/`, pinned by the `firmware_files` test; `flash_mcu` needs no `--hex` |
| Live bench run T1-T5 (turn 39: list/bringup/session_full --cmos) | PASS on the real sensor (Linux, rusb) |
| Live V4L2 probe T6 (turn 39) | uvcvideo binds 640x480 YUYV on MI_01 - sensor-match fix ships this turn (sysfs hex parse) |
| Live calibration parse (T3) | fix ships this turn (payload vs wire-record parser - the "need 121, have 120" FAIL) |
| Session lifecycle incl. self-reboot recovery (`src/session.rs`) | DONE (core) |
| Reply-timeout fail-fast (`REPLY_EMPTY_BUDGET`, turn 38) | DONE - no more 17-min silent spins |
| P2.1 transport (rusb discovery + bulk I/O) | CODE DONE - never touched real hardware |
| Re-enumeration handling (`src/transport/reenum.rs`) | DECISION CORE DONE + unit-tested; rusb loop coded, untested on hardware |
| P2.2 live read-only bring-up (`src/examples/bringup.rs`) | LIVE PASS (T3, turn 39) - version 2619, device ID, codes read |
| P2.4 full-session test (`src/examples/session_full.rs`, turn 38) | LIVE PASS (T5 `--cmos`, turn 39) - register init + 6/6 polls |
| P2.5 Linux video bring-up (`src/examples/video.rs` + `src/transport/v4l2.rs`) | T6 probe ran: uvcvideo sees 640x480 YUYV; the sensor cross-match fix (sysfs hex ids) ships turn 39 - rerun T6/T7 |
| P2.3 8051 firmware upload (`src/examples/flash_mcu.rs`, turn 39) | CODE DONE - two-key consent gate (SHA-256 pins + `--confirm` + `FlashPermit`), waits for the bootloader window, readback abort, reenum proof; `FLASH_CONFIRMED` stays closed until the first T8 PASS |
| Marker/3D record decoding (xEye3Ddata 10-slot records) | PARSER DONE - never seen on the wire in ANY capture (open) |
| P4 ROS 2 node (idong-ros2, idong_msgs) | NOT STARTED (needs a ROS install) |

## Layout (turn 40: everything under `software/`)

The repository root holds `software/` (all driver code), `research/`
(the findings, analysis outputs and research scripts), `tools/` (the
Windows bench kits) and `deliverables/` (the PDF reports). Run every
cargo command from `software/`. Everything there is one crate - one
`Cargo.toml`, ALL code under `src/` (turn 39): the lib in `src/`, the
examples in `src/examples/`, the integration tests in `src/tests/`
(explicit path targets, so the commands are unchanged). The vendor
device files ship in `firmware/` (turn 40; `firmware/README.md`), and
`idong-ros2` / `idong_msgs` are the two colcon packages next to the
crate. The rusb transport is a cargo feature:

```
cargo test                      # 100 tests, hardware-free, no libusb needed
cargo test --features usb       # same + compiles the rusb transport
cargo run --features usb --example list            # T2: discovery
cargo run --features usb --example bringup         # T3: read-only bring-up
cargo run --features usb --example probe_bootloader # T4: bootloader probe
cargo run --features usb --example session_full -- --cmos  # T5: full session
cargo run --features usb --example video -- --probe       # T6: V4L2 probe
cargo run --features usb --example video -- --cmos --frames 10  # T7: video
cargo run --features usb --example flash_mcu                   # T8 dry run
cargo run --features usb --example flash_mcu -- --confirm      # T8
```

`flash_mcu` reads `firmware/v2.6.0.6/app_m8051_2611.hex` by default
(`src/firmware.rs` finds the folder); `--hex <path-or-name>` selects
another copy of that file. The full ladder with PASS criteria and
failure collection: `docs/LINUX_TESTS.md` (turn 38/39, T8 updated turn
40).

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
- `src/examples/session_full.rs` (turn 38) - the P2.4 test: full vendor
  session (EEPROM + CMOS register tables, writes gated behind
  `--cmos`) + one poll cycle + 0x0205 probes.
- `src/examples/video.rs` (turn 38) - the P2.5 test: `--probe` (what
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
  + `parse_calibration_record` for the live session read (turn 37)
  + `parse_calibration_payload` (turn 39) for the 120-B payload that
  `SessionInfo::calibration` actually holds - the first bench run fed
  it to the 121-B wire-record parser and printed "need 121, have 120".
- `src/examples/flash_mcu.rs` (NEW turn 39) - the P2.3 8051 firmware
  upload as a controlled hardware replay: dry run (both SHA-256 pins,
  no device touched) + `--confirm` run (waits out the 31-157 s
  bootloader window, the exact cap3 flash with full-image readback
  abort, key+launch, reenum wait, version + identity proof). Two-key
  gate: `FirmwareSet::verify_hex` + `mcu_image` pins AND the
  `FlashPermit` consent token (`FLASH_CONSENT_PHRASE`, unforgeable
  outside the crate). Since turn 40 the hex it sends ships with the
  crate.
- `src/firmware.rs` (NEW turn 40) - the `firmware/` folder and its
  lookup rules (`firmware_dir`, `resolve_hex`, `read_hex`, `load_set`):
  no `--hex` argument is the shipped `app_m8051_2611.hex`, a bare file
  name is looked up in the folder, anything else is a path. Pinned in
  `src/tests/firmware_files.rs` together with every shipped vendor file.
- `src/protocol/runtime.rs` (turn 39) - `FlashPermit` +
  `flash_mcu_permitted` + `FirmwareSet::verify_hex`; `flash_mcu` itself
  stays compile-gated (`FLASH_CONFIRMED`) until the first T8 PASS.
- `src/{marker,records,handle,stream}.rs` - the xEye3Ddata record
  parser, marker model, handle events, reader thread.

## What is missing, in order

1. **Pull turn 40 and rerun the fixed bench steps** (the turn-38 run
   hit two bugs fixed in turn 39; all cargo commands run from
   `software/` now): T3 `bringup` again (the calibration parse
   now PASSES - `parse_calibration_payload`), T6 `video --probe` (the
   sensor ports list is no longer empty - sysfs ids are HEX, not
   decimal, so the nodes now cross-match: expect `matches sensor:
   true` on /dev/video0), then T7 the stream test.
2. **T8 the 8051 firmware upload** (`src/examples/flash_mcu.rs`): dry
   run + `--confirm` on the real sensor (the hex ships in `firmware/`,
   no file hunting). ONE clean PASS flips
   `FLASH_CONFIRMED` (P2.3 acceptance) and from then on the driver
   brings the sensor up from any bootloader landing itself.
3. **P2.5 promotion**: after T7 PASSes once, promote the video backend
   from `src/examples/video.rs` into the crate behind a feature (and
   wire `VideoSource`/`VideoAssembler` if payload-level access is
   ever needed). If T6/T7 FAIL on the uvcvideo side, the fallback is
   P2.5b: raw usbfs ISO on EP 0x85 (rusb has no ISO support - needs
   the libusb1-sys async API or raw usbfs URB ioctls).
4. **P2.4b auto-recovery**: `session_full`/`video` currently tell you
   to rerun after a self-reboot; wire the existing
   `transport::reenum` waiter into the examples' error path.
5. **The marker/3D plane**: NOT on the wire in any capture so far.
   The running games receive the 10-slot records over the engine pipes;
   the likely trigger is a command we have not sent or EP 0x03/0x83
   (512-B vendor endpoints, unused in every capture). A capture with a
   RUNNING GAME decides (the lobby unlock kit 37 makes the game
   runnable on Windows). The parser exists and is tested.
6. **P4 ROS 2 node** (idong-ros2 + idong_msgs): lifecycle node,
   parameters, QoS, diagnostics - blocked on a ROS 2 install, design
   already in the draft
   (`deliverables/idong100_RUST_ROS2_DRIVER_DRAFT.pdf`, turn 13).

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
  the session layer expects it; do not power-cycle manually. Every
  landing shows the bootloader 0471:2600 for a while - T8 can flash
  from any of them (the example waits the window out for you).
- The turn-39 bench note: right after a self-reboot the device can sit
  in the bootloader across replugs until something flashes it; on a
  pure-Linux bench that something is T8 (no vendor software needed).
- A `Resource busy` on opening MI_04 once after a re-enumeration was
  seen on the bench (addr 96) and cleared on rerun - the kernel was
  still settling the fresh composite device; rerun is the fix.
- NEVER write the T24C16A EEPROM (factory calibration) - the driver
  has no write path to it at all.
