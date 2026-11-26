# Chat context backup - turn 38 - 2026-11-27

Project: idong100 (Toltech / i-dong "idong Sensor V2.6.0.6", USB 0471:2600
boot -> 0471:2500 running). Repo:
https://github.com/thomas7475/Idong100.git (main 30db043 at turn 38 end,
branch project/state-turn38). Phase 2: kit unlock + Rust driver; goal:
the camera as 3D scanner.

## User message of turn 38

Repo link + fresh PAT + "lets continue with the next driver phase what
tests should i run on linux".

Standing rules: English only; zips + chat backup in download/ every
turn; PAT never printed or stored; robust modifications only.

## What turn 38 did (driver: the Linux bench ladder, no hardware here)

The next driver phase after the turn-37 read-only bring-up (P2.2) is
the SESSION (P2.4) + the VIDEO plane (P2.5) - both coded now as bench
tests, plus the one-stop Linux test plan the user asked for.

1. NEW examples/session_full.rs - the P2.4 full-session hardware test:
   read-only identity first (exactly bringup), then the CMOS register
   init - the FIRST writes this driver ever sends, gated behind an
   explicit `--cmos` flag (the dry run without the flag prints exactly
   what would be sent: 2 register tables 56 B + 24 B each written +
   read back, 6 x 8-B chunks written + read back, 0x0303 end - 17
   writes, all exact cap3 bytes, all volatile BG0313 registers, a
   power cycle clears them, EEPROM NEVER written) - then one poll
   cycle (0003/0004/0005/000D/0404/040F) + three 0x0205 stream polls
   (unanswered is EXPECTED without video; ~3 s budget each, not a
   hang). PASS/FAIL per step; a USB error prints the 31-157 s
   self-reboot note ("just rerun") instead of failing silently.

2. NEW examples/video.rs - the P2.5 video bring-up on Linux:
   - `--probe`: the READ-ONLY diagnostic (QUERYCAP + ENUM_FMT +
     ENUM_FRAMESIZES + G_FMT on every /dev/video* node) - the 10-second
     answer to "did uvcvideo bind the sensor and what format does it
     expose?".
   - default: the full stream test - session init (only with --cmos),
     sensor video node picked by the sysfs cross-match (--device
     override), 640x480 negotiation, mmap streaming (REQBUFS 4 ->
     QUERYBUF/mmap -> QBUF -> STREAMON -> poll/DQBUF loop), 0x0205
     polled at ~10 Hz alongside (the vendor pattern), per-channel
     PNGs + raw .bin + per-frame stats into video_runN/ (the kit-37
     runN rule: first nonexistent or EMPTY folder). PASS criteria
     printed (frames, 614400 B = 640x480x2, channel A contrast,
     ~8 fps expected).
   - The V4L2 UAPI mirror is the key robustness piece: every struct
     (capability 104 B, pix 48, format 208 with union at offset 8,
     fmtdesc 64, frmsizeenum 44, requestbuffers 20, buffer 88 with
     ts@24/m@64/length@72) and every ioctl number is CONST-ASSERTED
     against gcc-verified values from /usr/include/linux/videodev2.h
     - any compiler/arch drift fails the BUILD, not the bench day.
     Kernel docs on ioctl-size tolerance noted; 5.10+ layouts used.
   - Kept in the example on purpose: the crate itself stays
     100% unsafe-free (deny(unsafe_code) holds); the first hardware
     PASS promotes the backend into the crate behind a feature.
     Fallback if uvcvideo cannot stream the device (the capture
     showed no SET_INTERFACE alt>0 - unusual for UVC): P2.5b = raw
     usbfs ISO on EP 0x85 (rusb has no ISO support - needs libusb1-sys
     async or raw usbfs URB ioctls).

3. NEW src/transport/v4l2.rs - the PURE sysfs side (unit-tested, 4
   tests): /dev/video* node scan (index-sorted), sensor USB port scan
   (/sys/bus/usb/devices, idVendor/idProduct), the cross-match (a
   node's /sys/class/video4linux/videoN/device interface dir like
   "2-1.3:1.1" belongs to the sensor device "2-1.3"; the colon rule
   keeps the sibling "2-1.4" and the hub parent "2-1" from matching -
   and "2-1.30" from matching "2-1.3"), fourcc formatting.

4. NEW plane_stats/PlaneStats (src/protocol/video.rs, 4 tests) - the
   PASS/FAIL contrast check for streamed planes (mean/min/max/stddev,
   empty plane defined, never NaN).

5. Robustness fix (the bench-hang class, same family as kit-36's
   `net session`): RuntimeSession::read_until_reply pushed empty reads
   as raws and spun through 1024 iterations of the 1-s transport
   timeout = up to ~17 min of silence. NEW REPLY_EMPTY_BUDGET = 3
   consecutive empties (~3 s) -> clean timeout error. The slowest
   observed legit gap (the 0x0004 poll, ~1.2 s = 2 empties) is
   tolerated - 2 new tests (fail-fast-on-silence + tolerate-slow-gap).

6. Cargo: libc + png added as DEV-dependencies (examples only; plain
   cargo test stays hardware-free and libusb-free).

## docs/LINUX_TESTS.md (NEW) - the answer to the user's question

The one-stop Linux bench plan: the risk ladder table (T1-T7: what each
test sends, which ones write), step 0 prep (rustup, libusb, the udev
rule, usermod -aG video), then T1 cargo test -> T2 list -> T3 bringup
-> T4 probe_bootloader -> T5 session_full --cmos -> T6 video --probe
-> T7 video --cmos --frames 10, each with the exact command and PASS
criteria, the failure-collection kit (lsusb -v, dmesg, /sys/kernel/
debug/usb/devices, and usbmon = the Linux USBPcap for byte-exact
comparisons of what the driver sends), and what the ladder proves.
BRINGUP.md got the turn-38 pointer (T3 of the ladder), DRIVER_STATUS
rewritten for turn 38 (82 tests, what's missing in order), README
status updated.

## Verification (this sandbox, no hardware)

cargo test: 82 green (59 unit + 5 mock_replay + 6 runtime_replay + 11
video_golden + 1 doc) both with and without --features usb; nightly
rustfmt clean (reinstalled in the fresh sandbox); examples compile
warning-free in both feature states; every no-hardware FAIL path runs
clean and exits 1 (video --probe "no /dev/video* nodes", session_full
"no sensor", video --cmos "cannot open MI_04"). UAPI sizes/offsets/
ioctl numbers cross-checked by compiling a C probe against the real
kernel headers (gcc) and pinning the numbers as const asserts.

## Repo state (main 30db043)

Commit state(turn 38) on project/state-turn38 + the no-ff merge into
main, both pushed. Chat backup pushed to chat/context
(chat_context_backup_turn38.md). Driver deliverable zip:
idong100_phase2_turn38_2026-11-27.zip in download/ (src + examples +
tests + docs + Cargo.toml + README of the single crate).

## Next step for the user

1. On the bench (any Linux box or live USB with the sensor attached),
   run docs/LINUX_TESTS.md: step 0 once, then T1-T7 in order. T3 is
   the P2.2 acceptance (all reads), T5 the first writes (--cmos), T7
   the video question. ~20 minutes end to end. Send the console
   outputs back (plus PNGs/.bin of T7 and, on any FAIL, the collection
   commands' output from the doc's last section).
2. With T7 PASS: promote the V4L2 backend into the crate (P2.5
   promotion) and decode channel B (the second image plane - the
   3D-scan question).
3. With T6/T7 FAIL on the uvcvideo side: the P2.5b fallback decision
   (raw usbfs ISO on EP 0x85).
4. Still open for the 3D goal: a capture with a RUNNING GAME (marker
   records / EP 0x03/0x83) - kit 37 makes the lobby+game runnable on
   Windows.
