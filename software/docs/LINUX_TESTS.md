# LINUX_TESTS.md - the Linux bench test plan (turn 38)

What to run on the Linux box with the real sensor, in what order, what
PASS looks like, and what to send back. This is the full ladder from
zero to self-healing video; `docs/BRINGUP.md` stays the detailed P2.2
procedure.

Why Linux: nothing claims the sensor's vendor interface (MI_04), so the
driver takes it directly while the kernel (uvcvideo) owns the video
interface - one box, one cable, no USBPcap needed. A live USB with
`rustup` is enough. The ladder runs from zero to self-healing video
(T10: the driver babysits the sensor through its own reboots, exactly
like the vendor engine does).

## The risk ladder (what each test sends)

| Test | Sends to the sensor | Writes? |
| --- | --- | --- |
| T1 `cargo test` | nothing (no hardware) | no |
| T2 `list` | nothing (descriptors only) | no |
| T3 `bringup` | 1 version read + 4 EEPROM reads (vendor bytes) | no |
| T4 `probe_bootloader` | 1 version read | no |
| T5 `session_full` | T3 + CMOS register tables (vendor bytes) | **yes** (`--cmos`) |
| T6 `video --probe` | nothing (V4L2 queries only) | no |
| T7 `video` | T5 + V4L2 streaming | **yes** (`--cmos`) |
| T8 `flash_mcu` | the 8051 firmware upload (the vendor flash itself) | **yes** (`--confirm`) |
| T9 `video -- --iso` | T5 + UVC probe/commit + SET_INTERFACE(alt 2) + raw ISO reads on EP 0x85 | **yes** (`--cmos`) |
| T10 `video -- --recover` | T7/T9 + the automatic self-reboot recovery (re-flash + resume) | **yes** (`--cmos`, `--recover`) |
| T11 `video -- --rate N` | T7/T9 with the ring never starved by host work + rate/drop reporting | **yes** (`--cmos`) - **PASS (run 13, turn 48: 48 MHz clock pinned)** |

All writes are the exact observed vendor bytes: T5/T7 send the same
register tables the vendor engine sends on every session (volatile
image-sensor registers - a power cycle clears them; the T24C16A EEPROM
is never written, the driver has no write path to it at all). T8 sends
the vendor FLASH itself - which the vendor engine re-runs on EVERY
bootloader landing (several times per hour by itself): the image lands
in 8051 RAM, is read back in FULL before the commit key, and a mismatch
aborts the launch (the mask-ROM bootloader can never be overwritten, so
a failed flash is always recoverable by simply rerunning it).

## Step 0 - prepare the box (once, ~15 min)

```sh
curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
. "$HOME/.cargo/env"
git clone https://github.com/thomas7475/Idong100.git
cd Idong100/software        # the crate lives here (turn 40) - run every cargo command from software/

sudo apt install -y libusb-1.0-0-dev     # the usb feature needs it

# unprivileged access to the vendor interface (src/transport/udev.rs)
sudo tee /etc/udev/rules.d/42-idong.rules <<'EOF'
# Toltech i-Dong sensor (idong crate) - unprivileged libusb access
SUBSYSTEM=="usb", ATTRS{idVendor}=="0471", ATTRS{idProduct}=="2600", MODE="0666"
SUBSYSTEM=="usb", ATTRS{idVendor}=="0471", ATTRS{idProduct}=="2500", MODE="0666"
EOF
sudo udevadm control --reload-rules && sudo udevadm trigger
# replug the sensor after this

# /dev/videoN access (the T6/T7 V4L2 side)
sudo usermod -aG video $USER    # then log out and back in
```

## T1 - build + unit tests (no hardware)

```sh
cargo test && cargo test --features usb
```

Expected: `74 passed` in the lib (both runs, 105 with the integration
tests + doc-test), all green.

## T2 - discovery (read-only)

```sh
cargo run --features usb --example list
```

PASS: `1 sensor device(s) found` - `0471:2500` (runtime) or `0471:2600`
(bootloader). Cross-check `lsusb | grep 0471`.

## T3 - THE P2.2 read-only bring-up

```sh
cargo run --features usb --example bringup
```

PASS: version "2619", device ID `AB460DC8...` (32 hex), edition tag
`000T`, code B `2613`, calibration 120 B that parses (baseline ~100 mm,
focal ~432 px). Details: `docs/BRINGUP.md`.

## T4 - bootloader probe (optional)

```sh
cargo run --features usb --example probe_bootloader
```

PASS right after plugging in: `bootloader answered, version "2600"`.

## T5 - P2.4 full-session bring-up (first writes)

```sh
cargo run --features usb --example session_full             # dry view
cargo run --features usb --example session_full -- --cmos   # real run
```

The `--cmos` run sends the CMOS register init (2 tables + 6 chunks +
end marker, exactly the cap3 bytes), then one poll cycle (6 polls) and
three 0x0205 stream polls. (If the sensor sits in its bootloader: T8
flashes it from Linux now - no vendor software needed.)

PASS: `PASS: full-session bring-up complete.` - register init answered,
poll cycle 6/6, calibration still parses. If a step fails with a USB
error after the sensor was talking: that is the 31-157 s self-reboot
(normal) - just rerun; automatic recovery is the next phase (P2.4b,
`transport/reenum` decision core is already in place).

## T6 - V4L2 probe (read-only, the format question)

```sh
cargo run --features usb --example video -- --probe
```

Lists every /dev/video* node with driver, card, formats, frame sizes
and the current format, and marks which node matches the sensor by the
sysfs cross-match. This answers, with zero risk: did uvcvideo bind the
sensor, and what format does it expose?

PASS criteria: at least one node marked `matches sensor: true`, with a
format listed around 640x480. Send the full output either way - a FAIL
here decides the P2.5b fallback (raw usbfs ISO on EP 0x85).

## T7 - THE P2.5 stream test

```sh
cargo run --features usb --example video -- --cmos --frames 10
```

Turn-41 bench result: the node + format negotiation PASS (640x480 YUYV,
uvcvideo), then `STREAMON: Input/output error (os error 5)` - the
documented decision point. uvcvideo cannot stream this firmware; the
fallback is T9 (the raw usbfs-ISO backend on EP 0x85), which is the
ACTIVE test now.

Runs the full session init (as T5), picks the sensor's video node
(`--device /dev/videoN` to override), negotiates 640x480, streams
mmap buffers while polling 0x0205 at ~10 Hz, and writes per-channel
PNGs + raw .bin frames into `video_runN/`.

Turn-41 bench result: the node + format negotiation PASS (640x480 YUYV,
uvcvideo), then `STREAMON: Input/output error (os error 5)` - then the
documented decision point. **Turn-44 bench result: PASS** - right after
the T8 re-flash (video_run2): 10 frames, 614400 B each, 8.7 fps
app-side (~25 fps device-side, seq 0..26), channel A a real image.
The turn-41 EIO was therefore state-dependent, not permanent: both
observations stand, and the deterministic path stays T9 (`--iso`).

PASS (per the turn-35 capture predictions):

- frames arrive (~8-25 fps device-side; 30 fps is only the negotiated
  upper bound),
- `614400 B` per frame = 640x480x2 (the two-channel raster),
- channel A stats show real contrast (non-flat scene image),
- channel B stats are present (the unidentified second channel).

Anything else is still valuable: a FAIL with its console output + the
generated PNGs decides the next move.

## T9 - THE P2.5b raw usbfs-ISO stream test (the active frontier)

```sh
cargo run --features usb --example video -- --iso --cmos --frames 10
# alternates for the next A/B rounds:
cargo run --features usb --example video -- --iso --cmos --frames 10 --alt 1
cargo run --features usb --example video -- --iso --cmos --frames 10 --no-set-alt
```

The P2.5b backend: takes the UVC VideoStreaming interface from uvcvideo
(`USBDEVFS_DISCONNECT_CLAIM` - reversible, uvcvideo re-binds after
exit), runs the observed probe/commit SET_CURs (golden bodies),
SELECTS the streaming alt-setting (`USBDEVFS_SETINTERFACE` alt 2 =
2412 B/microframe; `--alt 1` forces the 812 B alt), then submits ISO
URBs on EP 0x85 directly (no kernel UVC driver in the path); on exit
it restores alt 0 and releases the claim.

Turn-43 bench result: the first run (turn 42, mirroring the vendor
capture's no-SET_INTERFACE flow) failed at the first SUBMITURB with
`No such file or directory (os error 2)` - ENOENT, not a missing
file: Linux arms an ISO endpoint only in the selected alt setting,
and at alt 0 (the zero-bandwidth alt, no endpoints) the host controller
has no transfer ring for EP 0x85. SET_INTERFACE became the default
step; `--no-set-alt` keeps the failing variant as the A/B control.

**Turn-44 bench result: PASS** (video_run1, the fix built): MI_01
taken, probe/commit PASS, alt 2 selected, 1281 payloads / 0 errors,
frames closed per PTS. The run also exposed the assembler's EOF
order (every frame split into a 612000-B body + a 2400-B tail under
one PTS = exactly 614400 B together) - fixed in turn 44 (`docs/`
`PROTOCOL.md` "Turn 44"): a rerun closes ONE FULL 614400-B frame per
PTS and prints the PTS cadence (`dt N.N fr` per frame + the deltas
summary - the bench showed 3/4.5/4.5/6 frame periods between frames,
~1.8 fps produced under the golden probe/commit vs ~25 fps on the
V4L2 path the same hour; that difference is the open A/B).

PASS: frames arrive, 614400 B each (one per PTS, EOF-closed), channel
A shows real contrast, full 640x480 PNGs per channel.

FAIL with output + `dmesg | tail -50`: the next decisions are the
negotiation A/Bs (the uvcvideo commit bytes via usbmon, `--alt 1`,
ISO right after a flash) and, only if not one ISO byte ever moves,
the ep 0x03/0x83 bulk path (never observed on the wire).

## T10 - THE P2.4b self-reboot recovery soak (video --recover)

```sh
# the V4L2 backend, 40+ frames - crosses at least one 31-157 s cycle:
cargo run --features usb --example video -- --cmos --frames 40 --recover
# the raw ISO backend (start it with the sensor in the bootloader - it
# flashes it itself and goes on):
cargo run --features usb --example video -- --iso --cmos --frames 12 --recover
```

What this proves: the sensor self-reboots to its 0471:2600 bootloader
every 31-157 s (documented firmware behavior, observed 3x in cap7 and
1x in cap8, and on every turn-44 bench run) and STAYS there until
something flashes it - the vendor engine's whole babysitting job.
With `--recover` the driver takes that job: on a mid-run landing it
waits for the bootloader, re-flashes the pinned 8051 image (the ungated
`FLASH_CONFIRMED` path - the exact vendor sequence, readback-verified
before the commit key; the pins are checked once at startup, before
any device is touched), waits for the 2500 re-enumeration, re-inits
the session and RESUMES capture. The frame counter, output folder
and pooled stats carry across recoveries; the summary reports how
many landings were absorbed (budget: 12 per run).

PASS: the run crosses at least one self-reboot - watch for

```
      self-reboot #1/12 mid-run (...)
      recovery: bootloader landing - re-flashing (the vendor sequence)
      recovery: flash ok (readback matched), launched
      recovery: runtime re-enumerated (bus 1, addr NN)
```

- and still ends with `frames captured : 40 (1 self-reboot
recovery/recoveries)`. Starting the command on an already-landed
bootloader is the same test (the first attempt reports the gone
device, the recovery flashes, the capture starts).

FAIL notes: a recovery that never finds the bootloader within 180 s
= the sensor is absent/unplugged; a flash that fails twice in a row
= collect `lsusb -v -d 0471:2600` + the output. Without `--recover`
the run still ends exactly like the old one (a gone device = the
"self-reboots every 31-157 s - just rerun" note).

## T11 - THE P2.6 rate measurement (video --rate N)

```sh
# the raw ISO backend, 300 frames, never starved, self-healing:
cargo run --features usb --example video -- --iso --cmos --rate 300 --recover
# the V4L2 backend for the A/B:
cargo run --features usb --example video -- --cmos --rate 300 --recover
```

The one open video question (turn 44): the same device measured 2.1
fps delivered on the ISO path vs ~25 fps produced through uvcvideo,
same hour. The suspect was never the device - it was our own loops:
per-frame host work (PNG encode, plane stats) sat between a reaped
URB and its resubmission, the ring ran dry during our processing,
and ASAP scheduling hopped the next URB to a distant microframe.
Frames the device produced simply never arrived - invisible to
every error counter. The PTS cadence 1/5/5/4/4... of the turn-44
runs is exactly that starvation signature.

T11 is the clean measurement: `--rate N` captures N frames with the
loop NEVER doing per-frame host work beyond a byte copy (requeue /
resubmit first, stats + PNGs only for the first 3 frames), and
prints the full rate table:

- `host wall rate` - what our loop actually received (wall clock),
- `device rate` - the PTS cadence over the captured span,
- `device drops` - gap accounting (V4L2 buffer seq gaps / ISO PTS
  gaps): frames the DEVICE made that our loop never saw,
- `payloads ... errors: N proto, N overflow, N ilseq` - the ISO
  ring's own per-packet health (babble / CRC classes),
- and on the ISO path a `GET_CUR(probe)` reply - what the firmware
  ACTUALLY settled on after the negotiation (the one control read
  that decides the interval question).

PASS criteria: `device drops : 0` with a flat PTS cadence (every
delta 1.0 frame period) - that is the device's true production rate,
whatever the number is. A cadence still showing 5.0/4.0 steps WITH
zero drops and zero packet errors means the device itself spaces
frames (its own pacing) - the 30-fps interval is then nominal only,
and the marker plane work can rely on the measured number.

What the numbers will decide (the A/B matrix):

| observation | verdict |
| --- | --- |
| ISO wall rate ≈ PTS rate ≈ 25 fps, drops 0 | the device produces 25 fps; the old 2.1 fps was pure host starvation - question closed |
| PTS cadence 1.0 flat, wall rate lower | our loop still drops (PNG path, poll cadence) - keep tightening, not a device issue |
| cadence 5.0/4.0 steps WITH drops 0 and packet errors 0 | the device itself paces frames at ~2 fps despite the commit - firmware behavior, documented and lived with |
| packet errors nonzero (proto/overflow/ilseq) | bus-level signal integrity - try another cable/port before believing any rate |

**Turn-47 bench result (run 11, `--rate 300 --recover`, hand partly
covering the sensor)**: 255 frames over 9 self-reboots - but the rate
table itself FAILED the run's own honesty criteria, and the failure
was diagnostic gold:

- `device drops : 1675` (86.8% of the device's frames never arrived) -
  the starvation the turn-45 "measure-first" rework promised to remove
  was STILL THERE: the `--rate` mode parsed its flag but never used
  it (the PNG- and stats-per-frame work never left the reap thread),
  and a 4-URB ring is only 4 ms of ISO buffer. Two PNG encodes per
  frame in a debug build = ~100 ms of ring drought per frame.
- The run also exposed THREE accounting bugs, all fixed turn 47:
  `host wall rate 51.5 fps` divided the total frame count by the LAST
  session's span; `device rate 1.0 fps` divided delivered frames by a
  PTS span that included ~30 s reboot gaps per recovery; and the
  `1675 drops` counted the reboot gaps themselves (the device was in
  its bootloader - nothing was dropped then; the real starvation loss
  was ~700, still 86.8% of what the device made while streaming).
- The 9th recovery failed with `open bootloader: Access denied` - a
  fresh `/dev/bus/usb` node can appear before udev applies its
  permissions; the first 8 opens won the race, the 9th lost it. The
  bootloader open now retries (5 x 1 s) while the bootloader is still
  on the bus; the /dev/video open retries the same way.
- The hand-cover data itself: channel A is confirmed a real image
  (mean swept 6..153 with occlusion, sd 4..82; pooled sd 65.9 over
  78.3 M samples); channel B stayed at 127.3 +- 6.4 (range 65..207) -
  B sd rises to ~19 when A is bright (weak scene coupling) but that
  is noise-level contrast, not an image.

The turn-47 loop makes the T11 promise real, and rerunning run 12
settles everything the run-11 log could not:

```sh
cargo run --features usb --example video -- --iso --cmos --rate 300 --recover
```

- ALL per-frame work (channel stats, the console line, .bin + PNG
  writes, PTS/seq accounting) runs on a frame WORKER thread; the
  reap loop only copies packet bytes, resubmits, assembles and hands
  closed frames over (`FrameSink`, bounded channel, 256 frames of
  headroom).
- `--ring N` (default 16) instead of the run-11 four URBs = 16 ms of
  ISO buffer before the bus, drought-proof against OS hiccups.
- The in-ring ~10 Hz 0x0205 polls are GONE (one pre-flight poll after
  session init instead): the device never answers 0x0205 while
  streaming on this bench (every run since turn 42), and each
  unanswered poll blocked the reap loop ~3 s - run 11 shows exactly
  that as the dt 4.0/5.0/6.0 staircase right after every recovery.
- PNGs are written only for the first 3 frames in `--rate` mode
  (the documented policy, now the actual code), `--save N` keeps any
  frame's raw .bin, and the summary prints the honest per-session
  accounting: active-span wall fps, device-made fps (delivered +
  intra-session gaps), the median/max PTS cadence, and - when the
  cadence is flat and drops are 0 - the implied PTS tick rate
  against the assumed 15.36 MHz (the golden capture read 8.0 fps,
  the uvcvideo bench ~25 fps; a clean run decides which clock is
  real).

PASS (unchanged): `device drops : 0` (intra-session), every PTS delta
1.0. Then `--analyze <outdir>` runs the turn-47 channel analysis over
the saved .bin files (A/B stats, row-mean structure, A/B correlation,
the channel-B image verdict) with no device attached.

**Turn-48 bench result (run 13, the first clean T11 run):
PASS.** `--iso --cmos --rate 300 --recover`, 300 frames across 3
capture sessions, 0 payload errors, honest per-session accounting:

- Session 1 (undisturbed, 111 frames): **110/110 consecutive PTS
  deltas EXACTLY 1,920,000 ticks** - the flat cadence the run-11
  table could not show. Device rate 25.0 fps.
- Sessions 2/3 (disturbed - hand moving in front of the sensor,
  auto-exposure beats visible): deltas only 1,920,000 / 2,400,000 /
  2,880,000 ticks (multiset over the run: x246 / x8 / x43) - whole
  multiples of a **480,000-tick (10 ms) skip quantum**: when the
  device misses the 40-ms cadence it skips in quarter-frames, never
  fractional ticks.
- **The tick-rate question is closed: 48 MHz.** The summed
  per-session PTS span 615,360,000 ticks over 297 frame deltas vs
  the measured 12.8 s active wall span: 12.820 s at 48 MHz (0.2%
  agreement), impossible at 15.36 MHz (that span would take 40.1 s)
  or 44.1 MHz (13.95 s). The 15.36 MHz constant had been a
  golden-capture window artifact (546 FID transitions / 68 s read
  8.0 fps; the golden video window is that same anomaly).
  `DEVICE_CLOCK_HZ` = 48,000,000 and `PTS_SKIP_QUANTUM` = 480,000
  since turn 48; the frame period is 40.0 ms = 25 fps nominal, the
  negotiated 30 fps (interval 333333) is an upper bound only, and
  alt 2's 2412 B/microframe (19.3 MB/s) is exactly the headroom
  25 fps x 614,400 B needs (15.36 MB/s).
- The summary's tick-rate cross-check now divides the ACTUAL summed
  PTS span by the active wall span (the old (frames-1) x
  PTS_PER_FRAME form ignored the skip deltas and read a bogus
  44.85 MHz on this very run).
- Evidence: the run-13 console log, re-analyzed any time by
  `research/scripts/analyze_run13_pts.py`.

T11 status: **PASS, closed** (the pre-turn-48 "44.8 MHz" reading was
the old cross-check formula, not the device).

## T12 - THE P3a record-plane bench (records --cmos, turn 46)

```
cargo run --features usb --example records -- --cmos
cargo run --features usb --example records -- --cmos --frames 25 --out recdir
```

The marker/record plane, decoded turn 46 from the vendor stack (full
evidence: `research/idong100/agents/20-turn46/FINDINGS.md`): the PU/OD
framing on the SECOND MI_04 bulk pair - the device announces every
record with a 64-B OD **type-0xF0** frame on **EP 0x83**, the host
reads the **640-B raw record** (OD magic trailer at +0x27C, status byte
at +0x25C) and answers with a 64-B PU **type-7** ACK on **EP 0x03**.
(The 7F 55 session stays on EP 0x02/0x82 - EasyUSB pipe calibration.)

What the run does: session init (register tables + polls, exactly
T5 --cmos) -> the ENS probe (the vendor `0x10640`: read 64 B from
EP 0x83 ten times at 100 ms, expect a type-0xF0 OD frame) -> on
success the handshake loop (announce -> record -> zero-payload type-7
ACK; our ACK carries no classifier output, so `--frames 25` runs ~1 s
of the stream) with raw dumps in `--out`.

The verdict matrix:

| observation | verdict |
| --- | --- |
| VERDICT A: type-0xF0 answers, records read, trailer OK | the record plane streams spontaneously after the session init - dump the blob records, the classifier-input layout is the next decode |
| 64 B arrive but not OD/0xF0 | something else lives on EP 0x83 - send the hex head printed by the run |
| VERDICT B: silence for all 10 probes (read-only, no writes happened) | the runtime `ens_code.ldr` upload must run first - the ONE capture with the kit-37 unlocked app in a running game pins its wire bytes (T12 decides whether that capture is needed at all) |
| records stop mid-run | check the self-reboot window (31-157 s); rerun on a fresh landing (the run is short on purpose) |

PASS criteria: VERDICT A with `records N/N` and raw dumps, or VERDICT B
recorded as the capture request (both are decisive outcomes - that is
the point of the probe).

## T8 - THE P2.3 8051 firmware upload (controlled flash replay)

```sh
# dry run first (read-only: both SHA-256 pins, no device opened):
cargo run --features usb --example flash_mcu
# the flash itself (two-key gate: pins + --confirm):
cargo run --features usb --example flash_mcu -- --confirm
# the files arrive with the clone - check them once (24 x OK).
# NOTE: the paths inside SHA256SUMS are relative to firmware/, so run
# the check FROM software/firmware (from software/ it would miss every
# file - the turn-41 bench hit exactly that):
cd firmware && sha256sum -c SHA256SUMS && cd ..
```

`app_m8051_2611.hex` ships with the crate as
`firmware/v2.6.0.6/app_m8051_2611.hex` (turn 40; byte-exact copy of the
vendor file, hashes pinned from the turn-12 manifest - see
`firmware/README.md`), so no `--hex` is needed. `--hex <path-or-name>`
selects another copy of the same file; the pins refuse any other image
before a byte leaves the host.

What it does: waits for the 0471:2600 bootloader window (the sensor
self-reboots to it every 31-157 s; replug to force it at once), then the
EXACT cap3 vendor flash: `0x0101` begin, 335 raw 64-B blocks, `0x0201`
verify + FULL image readback compare, `0x0301` key, `0x0401` launch ->
re-enumerates as 0471:2500, then proves the runtime (version "2619" +
the four EEPROM reads).

PASS criteria (all printed): firmware pins hold; bootloader answered
"2600"; readback matched 21,411 B; re-enumerated as 0471:2500; version
"2619"; identity + calibration parse.

Turn-41 bench result: PASS - TWICE (addr 54->55 and 56->57, both
flashes readback-matched, launched, identity + calibration intact).
`FLASH_CONFIRMED` is flipped in the crate since turn 42: the ungated
`flash_mcu` path is open; the consent-gated example stays the
recommended interactive path.

A mismatched readback ABORTS before key+launch - a partial download is
never committed, and the bootloader ROM is never at risk. Send the full
output either way: ONE clean PASS here flips `FLASH_CONFIRMED` in the
crate and opens the ungated flash path.

## If something fails - what to collect

```sh
lsusb -v -d 0471:2500 2>/dev/null || lsusb -v -d 0471:2600
dmesg | tail -50
sudo cat /sys/kernel/debug/usb/devices
# optional kernel trace of what the driver sent (the Linux USBPcap):
sudo modprobe usbmon
sudo cat /sys/kernel/debug/usb/usbmon/1u > /tmp/idong.usbmon &
# ... run the failing example, then:  sudo killall cat
```

Send: the full console output of every test you ran, `list` output,
and the files of the failing step (PNGs/.bin for T7). With the usbmon
trace we can byte-compare the driver's writes against the vendor
captures exactly like we did with USBPcap on Windows.

## What the whole ladder proves

- T1-T4: the single crate builds + the pure protocol core is intact on
  the bench box; the rusb transport opens the real device (P2.1/P2.2
  hardware acceptance).
- T5: the session layer completes the observed vendor lifecycle on the
  real sensor, writes included (P2.4 acceptance).
- T6/T7: the video plane works through uvcvideo or we learn exactly why
  not (P2.5 acceptance or the P2.5b decision).
- T8: the driver brings the sensor up from a cold bootloader on its own
  (P2.3 acceptance) - the complete vendor lifecycle without any vendor
  software.
- T12: the record plane (P3a) - either the marker records stream and we
  hold the raw blob data, or the one remaining unpinned wire format (the
  runtime `ens_code.ldr` upload) is isolated as the single capture ask.
