# Chat context backup — turn 42 — 2026-11-27

Project: idong100 (Rust driver for the Toltech i-Dong Sensor V2.6)
Repository: https://github.com/thomas7475/Idong100, `main` = `9fbeb69` (turn 42)

## The request

The user posted the full turn-41 bench output and asked to analyze it and
continue with the next driver phase. The PAT was in the message again
(rotate it - third time now).

## Bench analysis (turn 41 output)

| Test | Result |
|---|---|
| T1 cargo test | (not shown; user ran commands from ~/Projekte/Idong100/software) |
| T2 list | PASS (bootloader 2600, later runtime 2500 addr 55/57) |
| T3 bringup | PARTIAL as documented (sensor in bootloader) - the runtime probe after flash: PASS |
| T4 probe_bootloader | PASS "2600" |
| T5 session_full + --cmos | PASS (register init, 6/6 polls, 0x0205 unanswered as expected) |
| T6 video --probe | FAIL "no /dev/video nodes" while in BOOTLOADER state (expected); PASS inside the T7 run (video0 capture node) |
| T7 video --cmos | **FAIL: STREAMON EIO** - the documented decision point |
| T8 flash_mcu --confirm | **PASS TWICE** (54->55, 56->57, readback matched, version 2619, calibration parses) |
| sha256sum -c firmware/SHA256SUMS | user FAIL: 24 missing files - DOC BUG (paths are relative to firmware/, not software/) |

## What shipped (turn 42, commit 9fbeb69)

1. `FLASH_CONFIRMED = true` (P2.3 accepted; T8 PASS x2 is the evidence).
   The two-key FlashPermit consent path stays for interactive use.
   Gate tests rewritten for the new invariant (ungated entry needs the
   full byte-exact script; session connect on bootloader now flashes and
   signals NeedsReenumeration - mock script fixed to the golden order).
2. P2.5b raw usbfs-ISO backend:
   - `src/transport/usbfs.rs` (pure, 5 tests): config walker,
     wMaxPacketSize decode (bits 10..0 base, bits 12..11 additional
     transactions; 0x1324 -> 2412 B/microframe = the capture stride;
     the old "4900" was the raw field read as decimal), UVC SET_CUR/
     GET_CUR + GET_DESCRIPTOR builders, probe_commit_body encoder.
   - `src/examples/video.rs --iso`: usbfs UAPI mirror (ABI-pinned,
     gcc-verified: UrbHeader 56 B, IsoPacketDesc 12 B, all ioctl
     numbers), DISCONNECT_CLAIM MI_01 from uvcvideo, observed probe/
     commit SET_CURs, GET_DESCRIPTOR(config) via ep 0, ISO URB ring
     (4 URBs x 8 microframes, ASAP, poll+REAPURBNDELAY, re-submit),
     VideoAssembler feeding, PNG + .bin output, RELEASEINTERFACE on
     exit (uvcvideo re-binds). No SET_INTERFACE - mirrors the capture.
3. Docs: LINUX_TESTS.md (T9 section, T7/T8 bench results, 105-test
   count, the sha256sum -c cwd fix: run from software/firmware),
   DRIVER_STATUS.md (turn-42 line, P2.5b row, stale P2.5b note fixed),
   PROTOCOL.md (turn-42 section, wMaxPacketSize correction).

105 tests green in BOTH feature states; all examples build warning-free.

## Open items / next phase

- **T9 is the next bench command** (the user's box):
  `cargo run --features usb --example video -- --iso --cmos --frames 10`
  PASS = frames ~8 fps, 614400 B, chA contrast. FAIL + dmesg decides:
  SET_INTERFACE alt 1/2 before URBs, or the ep 0x03/0x83 bulk path.
- The token was pasted a third time: ROTATE IT.
- Old TODO notes: the ROS 2 node (P4) not started; marker/3D records
  never seen on any capture (needs a running-game capture).

## Key file paths

- software/src/transport/usbfs.rs (new, pure)
- software/src/examples/video.rs (--iso engine, usbfs_api mod)
- software/src/protocol/runtime.rs (FLASH_CONFIRMED = true @171)
- software/docs/LINUX_TESTS.md (T9 + the sha256sum fix)
