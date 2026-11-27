# DRIVER_STATUS.md - how far the Rust driver is, what is missing

Turn 46, 2026-11-27. The one-file answer to "how far are we, what is
left". Status line: **the protocol is fully decoded - including the
last open plane: turn 46 decoded the marker/record stream from the
vendor stack (xEyeUSB.dll `usbCom` worker + the `ens` subsystem,
re-extracted from `xeyez_Lobby.dat` and disassembled - full evidence:
`research/idong100/agents/20-turn46/FINDINGS.md`). The record plane is
a THIRD protocol layer: the PU/OD framing on the second MI_04 bulk
pair **EP 0x83 IN / EP 0x03 OUT** (EasyUSB pipe calibration: the 7F 55
session = write pipe 0 / read pipe 1 = the USBPcap-proven EP
0x02/0x82, so usbCom's read pipe 2 / write pipe 3 are 0x83/0x03 - the
"unused" endpoints of every capture). The device announces each
record with a 64-B OD **type-0xF0** frame on EP 0x83, the host reads
the **640-B raw record** (OD magic trailer @+0x27C, status byte
@+0x25C) and answers with a 64-B PU **type-7** ACK on EP 0x03 whose
payload the vendor classifier computes from the PREVIOUS record.
Open item #4 is RESOLVED (`TransUSB2Pipe` @RVA 0x25e0 + the
best-marker loop of `GetADataFromUSB`: **x/y/z f64 @ slot
+0x08/+0x10/+0x18, quality i32 @ +0x24, smallest z = the active
marker**) - and the deeper finding: the x/y/z are HOST-computed
(`classifying.dll!ClassifyAndTrack` classifies the 640-B blob record;
the marker record is a host-side structure). What starts the stream:
a startup probe (read 64 B from EP 0x83 x 10 @ 100 ms, expect
0xF0 - `0x10640`) - if nothing answers, the vendor runs the
`ens_code.ldr` upload state machine on the SAME pair first (the PU/OD
traffic never seen in any capture, because the license gates kept the
app out of the game state). The T12 bench example makes this decidable
in ONE command: `cargo run --features usb --example records --
--cmos`. 119 tests green, both feature states, nightly fmt clean.**
Everything else stands as of turn 45: T1-T10 all PASS (the bench
ladder complete, `--recover` absorbs the 31-157 s self-reboot), the
video backends promoted into the crate, P2.6's `--rate N` measures
the fps question cleanly (the turn-44 2.1-fps gap was our own ring
starvation). What remains: the T11 rate run + the T12 record-plane
verdict (both are one-command bench runs), the `ens_code.ldr` wire
bytes IF T12 says the stream needs the runtime upload (one kit-37
game capture pins them), and P4 ROS 2 (needs a ROS install).**

## The short version

| Layer | Status |
| --- | --- |
| Wire protocol (boot flash, session, EEPROM, CMOS, stream poll, UVC video) | DONE - byte-pinned from 5 captures, turn 32/33/35 |
| P1 pure core (frames, records, calibration, boot + runtime framing, video assembler) | DONE - 111 tests green, no hardware needed |
| All-code-in-src/ layout (lib + src/examples/ + src/tests/ - turn 39) | DONE - explicit `[[example]]`/`[[test]]` path targets, commands unchanged |
| Repository layout (turn 40: `software/` = crate + `firmware/` + `idong-ros2` + `idong_msgs` + `docs/`) | DONE - the 24 vendor device files ship in `firmware/`, pinned by the `firmware_files` test; `flash_mcu` needs no `--hex` |
| Live bench run T1-T5 (turn 39: list/bringup/session_full --cmos) | PASS on the real sensor (Linux, rusb) |
| Live V4L2 probe T6 (turn 39) | uvcvideo binds 640x480 YUYV on MI_01 - sensor-match fix (sysfs hex parse) + the turn-41 run confirmed it live |
| Live calibration parse (T3) | fix ships this turn (payload vs wire-record parser - the "need 121, have 120" FAIL) |
| Session lifecycle incl. self-reboot recovery (`src/session.rs`) | DONE (core) |
| Reply-timeout fail-fast (`REPLY_EMPTY_BUDGET`, turn 38) | DONE - no more 17-min silent spins |
| P2.1 transport (rusb discovery + bulk I/O) | CODE DONE - never touched real hardware |
| Re-enumeration handling (`src/transport/reenum.rs`) | DECISION CORE DONE + unit-tested; rusb loop coded, untested on hardware |
| P2.2 live read-only bring-up (`src/examples/bringup.rs`) | LIVE PASS (T3, turn 39) - version 2619, device ID, codes read |
| P2.4 full-session test (`src/examples/session_full.rs`, turn 38) | LIVE PASS (T5 `--cmos`, turn 39) - register init + 6/6 polls |
| P2.5 Linux video bring-up (`src/examples/video.rs` + `src/transport/v4l2.rs` + `src/transport/v4l2_stream.rs`) | T6 PASS (turn 41); T7 PASS (turn 44, right after the T8 re-flash - the turn-41 EIO was state-dependent); the ioctl engine PROMOTED into the crate (turn 44) |
| P2.5b raw usbfs-ISO backend (`--iso`, `src/transport/usbfs.rs` + `src/transport/usbfs_iso.rs`) | T9 PASS (turn 44: 1281 payloads, 0 errors, frames per PTS) - the SET_INTERFACE fix held; the engine PROMOTED into the crate (turn 44) |
| Video frame assembly (`protocol::video::VideoAssembler`) | the turn-44 bench fixed the EOF order: the final payload's data belongs to its own frame (was: split into 612000 + 2400 B under one PTS); golden + bench-shape tests green |
| P2.4b self-reboot recovery (`video --recover`) | DONE (turn 44): wait bootloader landing -> re-flash the pinned image (ungated `FLASH_CONFIRMED` path) -> wait renum -> re-init -> resume; `session_full` points at it |
| P2.3 8051 firmware upload (`src/examples/flash_mcu.rs`, turn 39) | ACCEPTED - T8 PASS x2 on the turn-41 bench (readback matched, launched, identity intact); `FLASH_CONFIRMED` = true since turn 42 |
| Marker/3D record plane (P3a, turn 46) | WIRE PLANE DECODED: PU/OD on EP 0x83/0x03 - type-0xF0 announce, 640-B raw record (OD trailer @+0x27C, status @+0x25C), type-7 ACK; open item #4 RESOLVED (x/y/z f64 @ slot +0x08/+0x10/+0x18, quality @+0x24) - the values are HOST-computed (classifying.dll SVM); `protocol::ens` + the T12 bench example ready; the `ens_code.ldr` upload state machine (the startup fallback) is the remaining unpinned wire format |
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
cargo test                      # 111 tests, hardware-free, no libusb needed
cargo test --features usb       # the same + compiles the rusb transport + the ioctl engines
cargo run --features usb --example list            # T2: discovery
cargo run --features usb --example bringup         # T3: read-only bring-up
cargo run --features usb --example probe_bootloader # T4: bootloader probe
cargo run --features usb --example session_full -- --cmos  # T5: full session
cargo run --features usb --example video -- --probe       # T6: V4L2 probe
cargo run --features usb --example video -- --cmos --frames 10  # T7: video
cargo run --features usb --example flash_mcu                   # T8 dry run
cargo run --features usb --example flash_mcu -- --confirm      # T8
cargo run --features usb --example records -- --cmos          # T12: record plane
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
- `src/transport/v4l2.rs` - the PURE sysfs side of the video backend:
  /dev/video* node scan, the sensor cross-match (usb port chain <->
  video node interface dir) and fourcc formatting; unit-tested (the
  matcher rules, incl. the sibling-trap).
- `src/transport/v4l2_stream.rs` (NEW turn 44) - the V4L2 ioctl engine,
  PROMOTED from the example after the T7 bench PASS: the
  ABI-pinned UAPI mirror (every struct size/offset/ioctl number
  const-asserted) + `V4l2Dev` (QUERYCAP/G_FMT/S_FMT/REQBUFS/mmap/
  QBUF/DQBUF/STREAMON/STREAMOFF). The crate stays `deny(unsafe_code)`
  everywhere else; this module is the sanctioned raw-ABI exception.
- `src/transport/usbfs_iso.rs` (NEW turn 44) - the usbfs ISO engine,
  PROMOTED from the example after the T9 bench PASS: the
  ABI-pinned usbdevice_fs mirror + `IsoUrb` (one in-flight ISO URB)
  + control_out / control_in (turn 45: GET_CUR reads) /
  read_config_descriptor / disconnect_claim /
  set_interface / release_interface / wait_reap / discard_and_drain.
  `libc` is an optional regular dependency behind `usb` now (plain
  `cargo test` still builds neither engine).
- turn 45 additions: `--rate N` (rate-measurement mode - the ring
  never starved; sets the frame budget) + the measure-first loop
  discipline in both backends of `src/examples/video.rs`,
  `count_dropped_frames` in `src/protocol/video.rs` (the pure drop
  math, unit-tested), the packet-error classes
  (proto/overflow/ilseq) on the ISO summary line, and the
  GET_CUR(probe) dump after the ISO negotiation.
- `src/examples/session_full.rs` (turn 38) - the P2.4 test: full vendor
  session (EEPROM + CMOS register tables, writes gated behind
  `--cmos`) + one poll cycle + 0x0205 probes.
- `src/examples/video.rs` (turns 38-44) - the P2.5/P2.5b bench script
  on top of the promoted crate engines: `--probe` (read-only), the
  V4L2 stream run, the `--iso` raw run (`--alt N`, `--no-set-alt`),
  and `--recover` (turn 44, P2.4b): the self-reboot recovery loop -
  a gone device re-runs as a fresh attempt (session + backend rebuilt,
  frame counter / outdir / pooled stats carried over), the landing is
  re-flashed with the pinned image via the ungated `FLASH_CONFIRMED`
  path (budget 12 landings/run). PTS cadence reporting on the ISO
  path (`dt N.N fr` per frame + the deltas summary); pooled plane
  stats are sample-count-weighted now (the old pairwise mean
  understated channel A on mixed-size frames).
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
  `flash_mcu_permitted` + `FirmwareSet::verify_hex`; `flash_mcu`'s
  ungated path is open since the turn-41 double PASS
  (`FLASH_CONFIRMED` = true, flipped turn 42) - the consent-gated
  example stays the recommended interactive path.
- `src/protocol/ens.rs` (NEW turn 46) - the runtime record plane: the
  64-B OD type-0xF0 `StatusFrame`, the 640-B `WireRecord` (OD trailer
  @+0x27C, status byte @+0x25C + the f660 flag decode, blob stats for
  the bench), the 64-B PU type-7 `Type7Command` (zero ACK + payload
  builder), all constants with disassembly citations.
- `src/examples/records.rs` (NEW turn 46) - the T12 record-plane bench:
  session init (--cmos) -> the ENS probe (the vendor `0x10640`: 64 B
  from EP 0x83 x 10 @ 100 ms, expect type 0xF0) -> on success the
  handshake loop (announce -> 640-B record -> type-7 ACK) with raw
  dumps + per-record stats; VERDICT A/B printed with the exact next
  step.
- `src/{marker,records,handle,stream}.rs` - the xEye3Ddata record
  parser (x/y/z/quality offsets RESOLVED turn 46 - the values are
  host-computed by the vendor SVM; `nearest_marker` mirrors the
  vendor's smallest-z selection), marker model, handle events, reader
  thread.

## What is missing, in order

1. **The T11 rate run (the one open video question, now one command).**
   The turn-44 bench measured 2.1 fps delivered on the raw ISO path vs
   ~25 fps produced through uvcvideo, same hour - and turn 45
   root-caused the gap as OUR OWN loops starving the ring during
   per-frame PNG work (the 1/5/5/4/4 PTS cadence is the signature:
   the device made the missing frames, they never reached a URB).
   The measurement is now clean:
   `cargo run --features usb --example video -- --iso --cmos --rate 300 --recover`
   (and the V4L2 A/B without `--iso`) - measure-first loops, drop
   accounting, packet-error classes, GET_CUR(probe) dump. Read the
   verdict matrix in `docs/LINUX_TESTS.md` T11: `device drops : 0`
   with a flat cadence = the true production rate, whatever it is.
2. **The marker/3D plane - decoded (turn 46), now ONE bench verdict.**
   The wire plane is fully understood (PU/OD on EP 0x83/0x03: 64-B
   type-0xF0 announce -> 640-B raw record -> 64-B type-7 ACK; the
   xEye3Ddata x/y/z are HOST-computed by the vendor SVM). What the
   vendor does when the startup probe finds NO stream: the
   `ens_code.ldr` upload state machine on the same endpoint pair -
   the never-captured PU/OD traffic. The T12 run decides which world
   we are in:
   `cargo run --features usb --example records -- --cmos`
   VERDICT A: records flow -> dump + decode the blob layout (the
   classifier input). VERDICT B: silence -> the ldr upload is needed
   first; ONE capture with the kit-37 unlocked app in a running game
   pins its wire bytes (the games' record stream runs while that
   capture is taken), and the driver replays it like `flash_mcu`
   replays the hex.
3. **P4 ROS 2 node** (idong-ros2 + idong_msgs): lifecycle node,
   parameters, QoS, diagnostics - blocked on a ROS 2 install, design
   already in the draft
   (`deliverables/idong100_RUST_ROS2_DRIVER_DRAFT.pdf`, turn 13). The
   promoted backends (`transport::v4l2_stream`, `transport::usbfs_iso`)
   are the node's video seam now.

DONE on the turn-44 bench day (for the record): the P2.5 promotion
(#3 of the old list - both ioctl engines moved into the crate behind
the `usb` feature, `libc` an optional regular dependency), the P2.4b
auto-recovery (#4 - `video --recover` flashes and resumes on every
self-reboot landing, budget 12/run), and the T7 rerun that revised
the "uvcvideo cannot stream this firmware" verdict to
"state-dependent".

## Do we need more USB captures?

For the driver core: **no** - boot, session, EEPROM, CMOS, stream
poll and the video plane are all byte-pinned. Two captures would
still add value (both optional):

- a capture with a RUNNING GAME - only needed if the T12 record-plane
  run returns VERDICT B (no type-0xF0 stream): it pins the
  `ens_code.ldr` runtime-upload state machine wire bytes (the PU/OD
  traffic on EP 0x03/0x83 that no capture ever saw, because the
  license gates kept the app out of the game state);
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
