# DRIVER_STATUS.md - how far the Rust driver is, what is missing

Turn 36, 2026-11-27. The one-file answer to "how far are we, what is
left". Status line: **the protocol is fully decoded (every plane
captured), the pure core implements it all with golden tests; what is
missing is the hardware bring-up (P2) and the video/ROS output (P2.5+
P4). No protocol blocker remains.**

## The short version

| Layer | Status |
| --- | --- |
| Wire protocol (boot flash, session, EEPROM, CMOS, stream poll, UVC video) | DONE - byte-pinned from 5 captures, turn 32/33/35 |
| P1 pure core (frames, records, calibration, boot + runtime framing, video assembler) | DONE - 64 tests green, no hardware needed |
| Session lifecycle incl. self-reboot recovery (`session.rs`, new turn 36) | DONE (core) - transport wiring pending |
| P2.1 transport (rusb discovery + bulk I/O, re-enumeration wait) | CODE DONE - never touched real hardware |
| P2.2 live bring-up (enumerate real device, read-only: version, EEPROM, device ID) | NOT DONE - needs the bench + your unit |
| P2.3 flash gate (`FLASH_CONFIRMED`) | CLOSED by evidence, OPEN for hardware replay |
| P2.5 video backend (V4L2 / usbfs ISO for EP 0x85) | SEAM DONE (`VideoSource`), backend missing |
| Marker/3D record decoding (xEye3Ddata 10-slot records) | PARSER DONE - never seen on the wire in ANY capture (open) |
| P4 ROS 2 node (idong-ros2, idong_msgs) | NOT STARTED (needs a ROS install) |

## What exists today (idong + idong-transport crates)

- `idong/src/protocol/runtime.rs` - the observed `7F 55` protocol:
  version, flash sequence (0101/blocks/0201/0301/0401, byte-exact from
  cap3+cap8), EEPROM reads with checksum verify (device ID, edition
  tag, calibration, codes), CMOS init (56-B + 24-B tables, 8-B
  chunks), polls, and the 0x0205 stream poll.
- `idong/src/protocol/video.rs` - the UVC plane: 12-B payload headers,
  PTS-advance frame closure (`VideoAssembler`), two-channel 640-px
  raster split, probe/commit bodies, `StreamPollState`.
- `idong/src/session.rs` (NEW turn 36) - the lifecycle: probe phase
  ("2600" bootloader / "2619" runtime), connect (flash + re-enum
  hand-off signal, or session init), steady-state poll cycle, stream
  poll, and `recover()` - the self-reboot recovery that re-flashes on
  every bootloader landing exactly like the vendor engine (the sensor
  reboots itself every 31-157 s; this is a REQUIREMENT, not a corner
  case).
- `idong/src/calib.rs` - T24C16A EEPROM factory calibration parser
  (verified against your 24c16-dump.bin: |T| = 100 mm, R orthonormal).
- `idong/src/{marker,records,handle,stream}.rs` - the xEye3Ddata
  record parser, marker model, handle events, reader thread.
- `idong-transport/src/usb.rs` - rusb discovery + bulk I/O with the
  retry policy; `reenum.rs` is a stub (see missing #3).

## What is missing, in order

1. **P2.2 live read-only bring-up** (the next real step, ~a day on the
   bench): run the `list` example on Linux with the sensor attached
   (vendor xeyez.sys NOT loaded - Linux has no driver claim on MI_04,
   so rusb can take it directly; the UVC interfaces 0-3 stay with the
   kernel). Then version read, EEPROM reads, device ID. Every frame is
   already golden-tested, so a byte diff against cap2/cap3 is the
   acceptance test. Needs: a Linux box (or live USB) + your sensor.
2. **P2.3 flip `FLASH_CONFIRMED`** after ONE controlled hardware flash
   replay: the sequence is byte-identical to the vendor's (pinned twice
   on the wire), but the rule is "this driver replayed it against real
   hardware before the gate opens". Optional; the vendor engine keeps
   re-flashing the MCU anyway (it does it on EVERY bootloader landing),
   so our driver can also simply let the sensor stay in runtime mode.
3. **Transport: re-enumeration handling** (`reenum.rs`): watch the
   2600 -> 2500 swap after launch / self-reboot, reopen, claim MI_04
   (detach the kernel driver if any), and wire `Session::recover` up
   to it. Pure plumbing over rusb hotplug or a poll loop.
4. **P2.5 video backend**: `VideoSource` is the seam; implement the
   Linux side - either V4L2 (the standard path for the UVC MI_01; the
   probe/commit values are pinned in `video_golden.rs`) or raw usbfs
   ISO on EP 0x85 after SET_INTERFACE alt-1. Then `VideoAssembler`
   feeds PNG/video out; channel A is the scene image, channel B is
   still unidentified (second lens / FPGA intermediate - open).
5. **The marker/3D plane**: NOT on the wire in any capture so far. The
   vendor app streams raw video + polls 0x0205; the 10-slot 88-B
   marker records (xEyeClass.dll GetADataFromUSB) never appeared - the
   running games receive them over the engine pipes, so the likely
   trigger is a command we have not sent (the lobby's
   getMiddleCameraCalibrateData flow) or they travel on EP 0x03/0x83
   (512-B vendor endpoints, unused in every capture). Next capture
   should include a RUNNING GAME (not just xEyeThirdLens) to decide.
   The parser for the record layout exists and is tested - only the
   trigger is unknown.
6. **P4 ROS 2 node** (idong-ros2 + idong_msgs): lifecycle node,
   parameters, QoS, diagnostics - blocked on a ROS 2 install, design
   already in the draft (docs/IMPLEMENTATION_DRAFT turn 13).

## Do we need more USB captures?

For the driver core: **no** - boot, session, EEPROM, CMOS, stream
poll and the video plane are all byte-pinned. Two captures would still
add value (both optional):

- a capture with a RUNNING GAME (marker records / EP 0x03/0x83
  traffic) - decides missing #5;
- a capture of the lobby actually passing the hardware test with the
  kit-36 patch applied (validates the unlock end-to-end and shows the
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
