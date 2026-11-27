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
| `software/src/` | the whole driver: protocol core + session lifecycle + (feature `usb`) the rusb transport + the V4L2 sysfs matcher (`src/transport/v4l2.rs`) + the V4L2 ioctl engine (`src/transport/v4l2_stream.rs`) + the usbfs ISO engine (`src/transport/usbfs_iso.rs`) + the firmware folder lookup (`src/firmware.rs`) + the ROS 2 adapter's pure data layer (`src/ros/`: params, lifecycle, pubsub shapes, diagnostics, node core - no rclrs, unit-tested) | thiserror, log, crossbeam-channel; rusb + libc only with `--features usb` |
| `software/src/examples/` | ALL example binaries (turn 39): `list`, `probe_bootloader`, `bringup` (P2.2), `session_full` (P2.4), `video` (P2.5: V4L2 + `--iso` raw + `--recover` self-healing + `--rate` measurement, turns 44-45), `flash_mcu` (P2.3: the 8051 firmware upload, two-key consent gate), `mcu_image`, `dump_frames` | png (dev-dep, examples only) |
| `software/src/tests/` | golden replays: mock, runtime, video, `ens_replay` (the turn-49 ENS startup machine: the ens_code.ldr code-load + hot start, byte-exact), plus `firmware_files` (the shipped vendor files) - hardware-free | - |
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
cargo test                                   # 158 tests, hardware-free
cargo test --features usb                    # the same + the rusb transport + the ioctl engines
cargo +nightly fmt --check
cargo run --features usb --example flash_mcu                # T8 dry run: pins only, no device
cargo run --features usb --example flash_mcu -- --confirm   # T8: the controlled flash
cargo run --features usb --example records -- --cmos        # T13: cold start + the record plane
```

The bench ladder T2-T13 with PASS criteria: `software/docs/LINUX_TESTS.md`.

## Status (turn 50, 2026-11-28) - the T13 failure solved: `A_a_v4.4_BG0313.bit` is the FPGA bitstream, now downloaded by the driver

The T13 bench failed at the cold machine's state 1 (`vendor -2: Rx
DEVICE ID Msg Err`) with steps 1-3 PASS. Researching that failure
answered the open `.bit` question in the same move: **the vendor
software DOES use `A_a_v4.4_BG0313.bit`** - `xEyeUsb.ini [Config]
BitFile=` names it, the xEyeUSB manager init hands it to
`xeyezHal_SetFpgaFile`, and `xeyezHal_StartFlow` downloads it to the
on-board **Xilinx Spartan-3A XC3S400A** on EVERY device open (the
configuration is volatile; the open state machine @0x13b70 runs the
8051 flash update, the FPGA download, the CMOS/FPGA/LED table flows,
in that order). The DSP sits behind the FPGA - until the bitstream is
loaded, the record plane (EP 0x83/0x03) is silent, which IS the T13
state-1 timeout; the turn-32 "no FPGA upload on the wire" was a
capture-window false negative. The download wire (same 7F 55 session
plane, EP 0x02/0x82): START `7f 55 02 00..` -> QUERY_CFG `7f 55 02
01..` -> announce `7f 55 02 02 00 00 00 04` + size BE u32 -> the
WHOLE file **bit-reversed** in raw <=64 KiB writes (no replies) ->
END -> DONE; replies are 8 B with ack `01` @+4. Full decode:
`research/idong100/agents/22-turn50/FINDINGS.md` (+ the usbCom
PRE-LOOP @0x114c0 bonus decode: records -> 2 ACKs -> PU 0x7FF/OD
0x7EE "Reset DSP"). In the crate: `protocol::fpga::{fpga_step_frame,
fpga_send_announce, fpga_bit_reverse, FpgaDownload,
FpgaDownloadReport}` + the `fpga_replay` test (byte-exact frames,
64 KiB chunking, the vendor bit-reversal chain pinned over all 256
bytes, the four vendor failure codes), and the records bench is now
7 steps - [4/7] the FPGA download with `--cmos` (`--no-fpga` skips,
`--fpga-only` runs just it), then probe -> cold/hot start -> records;
the ENS state-1 error now carries its cause (silence vs wrong-frame).
166 tests green in both feature states, nightly fmt clean, examples
warning-free both states. NEXT BENCH (T14): `cargo run --features
usb --example records -- --cmos --frames 10`.

## Status (turn 49, 2026-11-28) - P3b: the ens_code.ldr upload decoded from the vendor DLL itself, the capture ask retired

The T12 bench returned **VERDICT B** (silence on EP 0x83 after the
session init - the DSP needs its runtime loader upload before the
marker records flow), which under the old plan made a kit-37
game capture the last unpinned wire format. That capture is no longer
needed: this turn pinned the upload STATICALLY, from the vendor stack
itself (the v2.6.0.6 DLLs re-extracted from `xeyez_Lobby.dat`,
byte-verified against the turn-12 manifest; artifacts in
`research/idong100/agents/21-turn49/`). **The cold machine**
`0xf790` is a linear 8-state driver - its own GBK log strings name
the phases: startup-begin (PU 1 -> OD 1), code-load (the
`ens_code.ldr` upload: PU type 2 {file size, CRC32} -> OD 2 "ready
to load" -> ceil(size/64) RAW 64-B chunks -> OD 3 with verdict word
0xF), a skip state, the pairing select (OD 5 -> PU 0x25/0x26 per
`NeedHandleMatch`), ID & version (PU 0x8001 -> OD 1: the 32-B device
ID + 8-B DSP version -> `setDeviceID`), the TSOL pairing loop
(OD 0xFFF/4 -> PU 0xFFE/4) and the lens parameters (OD 6, 128 B ->
PU 6). **The hot start** `0x10640`+`0x10900` (the probe is a
hot-start detector: 3 record cycles with 2 ACKs, then PU 0x10 -> OD
0x10 lens + PU 0x9001 -> OD 0x9001 DID). **The checksum** is the
vendor crc.dll variant - the reflected 0xEDB88320 table with a
hybrid MSB-style update, init 0, no final xor (the shipped 54,420-B
ldr -> 0x63EAA8CA); not a catalog CRC, ported literally. In the
crate: `protocol::ens::{RecordIo, ColdStart, HotStart, StartupOutcome,
crc32_ens, ...}` + `RecordIo` for `UsbTransport`/`MockTransport`, the
`ens_replay` test (byte-exact cold start with the pairing loop,
match-off, the CRC-verdict failure, the hot start, probe silence),
and the records example now runs the whole T13 flow: probe -> hot or
cold start (the shipped SHA-256-pinned ldr; `--no-ldr` keeps the old
read-only T12 behavior) -> the record handshake loop. 158 tests
green in both feature states, nightly fmt clean, examples
warning-free both states. The next bench run (`records -- --cmos`)
is the one that turns the marker plane on.

## Status (turn 48, 2026-11-27) - the clock pinned at 48 MHz (T11 closed), P4 ROS 2 pure layer done

Two closures. (1) Run 13, the first clean T11 bench
(`video --iso --cmos --rate 300 --recover`, 300 frames, 3 capture
sessions, 0 payload errors), settled the PTS tick-rate question that
turn 44 opened: **the device clock is 48 MHz** - the summed
per-session PTS span (615,360,000 ticks over 297 frame deltas) vs
the measured 12.8 s active wall span agrees to 0.2% at 48 MHz and at
no other plausible crystal (15.36 MHz would take 40.1 s, 44.1 MHz
13.95 s); 1,920,000 ticks = 40.0 ms = **25 fps nominal** (the
negotiated 30 fps is an upper bound only; the undisturbed session
ran 110/110 consecutive deltas EXACTLY 1,920,000 - a flat cadence),
and skips quantize at 480,000 ticks = 10 ms (the disturbed sessions'
deltas were only 4/5/6 quanta, never fractional). The old 15.36 MHz
was a golden-capture window artifact; `DEVICE_CLOCK_HZ` = 48,000,000
and `PTS_SKIP_QUANTUM` = 480,000 since this turn, and the example's
tick-rate cross-check now divides the actual summed PTS span by the
active wall span (the old (frames-1) x PTS_PER_FRAME form read a
bogus 44.85 MHz on run 13 itself). (2) **P4 began: the ROS 2
adapter's pure data layer** (`software/src/ros/`, all unit-tested
with plain `cargo test` - no rclrs, no hardware): the draft-table-5
parameter model + the params.yaml parser (defaults pinned to the
launch file by test), the draft-5.3 lifecycle machine (a total
function over state x transition), the draft-table-4 interface
surface + the field-for-field message shapes with their converters
(the live quality gate, the handle-event codes), the diagnostics
aggregation (the vendor RingSize=200 traffic-light window with the
Green=20/Yellow=35 thresholds as fractions, the publish_hz_min
degraded floor over the trailing second, the 500-ms stall timeout),
and the draft-5.2 node core (drain -> convert -> stamp, TimeRefPair
for /idong/time_ref, live parameter reapply, the host-assigned event
sequence). `idong_msgs/Marker.msg` dropped the stale `age` field
(turn 46 proved record +0x18 is z). What remains for P4 is only the
rclrs glue in `idong-ros2` (thin copy code) and a ROS 2 install to
build it against. 149 tests green in both feature states, nightly
fmt clean, examples warning-free both states.

## Status (turn 47, 2026-11-27) - the run-11 post-mortem: the video pipeline never starves again, channel B is not an image

The `--rate 300 --recover` hand-cover bench (run 11) delivered 255
frames across NINE self-reboot recoveries - and its own rate table
failed honestly, which is exactly what a measurement tool is for:
`--rate` parsed its flag but never used it (two PNG encodes + two
plane-stats passes + a console line per frame, all on the ISO reap
thread; a 4-URB ring is 4 ms of bus buffer), so 86.8% of the
device's frames never arrived. Three summary bugs fell with it
(the 51.5-fps host wall rate divided total frames by the last
session's span; the 1.0-fps device rate mixed delivered frames with
reboot-gap time; the 1675 "drops" counted reboots), plus a udev
permission race on the 9th bootloader re-enumeration that ended the
run at 255/300. The turn-47 loop: ALL per-frame work on a frame
WORKER thread (both backends), `--ring N` (default 16 URBs), one
pre-flight 0x0205 poll instead of the in-ring loop (the device
ignores it while streaming; each unanswered poll starved the ring
~3 s), honest per-session rate accounting with the implied
PTS-tick cross-check (15.36 vs 48 MHz - the open clock question),
`--save N`, and the offline `--analyze DIR` channel analysis. The
hand-cover verdict: **channel A is a real image (mean swept
6..153 with occlusion, pooled sd 65.9 over 78.3 M samples); channel
B is NOT an image (127.3 +- 6.4, weak scene-coupled noise)** - the
video stream carries one image plane, and the marker/3D data lives
on the MI_04 record pair (0x83/0x03) per the turn-46 disassembly.
125 tests green in both feature states, nightly fmt clean.

## Status (turn 46, 2026-11-27) - P3a: the marker/record plane decoded

The last open plane fell: the marker/record stream, decoded from the
vendor stack itself (the v2.6.0.6 engine DLLs re-extracted from
`xeyez_Lobby.dat` and disassembled - evidence:
`research/idong100/agents/20-turn46/FINDINGS.md`). It is a THIRD
protocol layer - the PU/OD framing on the **second MI_04 bulk pair,
EP 0x83 IN / EP 0x03 OUT** (the "unused" endpoints of every capture;
pinned by EasyUSB pipe calibration: the 7F 55 session = write pipe 0 /
read pipe 1 = the USBPcap-proven EP 0x02/0x82, usbCom reads pipe 2 /
writes pipe 3). The exchange: the device announces each record with a
64-B OD **type-0xF0** frame, the host reads a **640-B raw record**
(OD-magic trailer @+0x27C, status byte @+0x25C) and answers with a
64-B PU **type-7** ACK whose payload the vendor classifier computes
from the previous record. Open item #4 fell with it: **x/y/z f64 @
slot +0x08/+0x10/+0x18, quality @+0x24, smallest z = the active
marker** (`TransUSB2Pipe` + the best-marker loop of `GetADataFromUSB`)
- and the deeper finding: those coordinates are HOST-computed
(`classifying.dll!ClassifyAndTrack` classifies the raw blob record;
the marker record is a host-side structure). New in the crate:
`protocol::ens` (the wire-plane model, all constants cited),
`transport::usb::{read_bulk_ep, write_bulk_ep}`, the T12 bench example
`records` (session init -> the ENS probe - the vendor `0x10640`
equivalent - -> the handshake loop with raw dumps). If the probe
finds no stream, the vendor runs the `ens_code.ldr` upload state
machine on the same pair first - the last unpinned wire format, one
kit-37 game capture away. 119 tests green in both feature states.

## Status (turn 45, 2026-11-27) - P2.6: the rate question, one command

The turn-44 rate mystery (2.1 fps delivered on the raw ISO path vs
~25 fps produced through uvcvideo, same device, same hour) had the
answer hiding in our own loops: per-frame host work (PNG encode,
plane stats) sat between a reaped URB and its resubmission, so the
ring ran dry during our own processing and ASAP scheduling hopped
the next URB to a distant microframe - frames the device produced
never arrived, invisible to every error counter (the 1/5/5/4/4 PTS
cadence of the turn-44 runs is exactly that starvation signature).
Both capture loops are now measure-first (requeue/resubmit BEFORE
any per-frame work), drop accounting counts frames the device made
but our loop missed (V4L2 buffer-seq gaps / ISO PTS gaps - pure,
unit-tested), the ISO ring classifies its packet errors
(proto/overflow/ilseq), `--rate N` measures N frames with the ring
never starved and prints the full rate table, and a GET_CUR(probe)
after the ISO negotiation reports what the firmware actually settled
on. 111 tests green in both feature states. The verdict matrix for
the next bench run lives in `docs/LINUX_TESTS.md` (T11).

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
  111 tests green in both feature states (the 3 new ones pin the
  EOF/error payload semantics).

Next: run T11 (`video -- --iso --cmos --rate 300 --recover`) - the
  clean rate measurement that closes the frame-rate question, the
  marker/3D record capture (needs a running game), then P4 ROS 2 -
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
