# LINUX_TESTS.md - the Linux bench test plan (turn 38)

What to run on the Linux box with the real sensor, in what order, what
PASS looks like, and what to send back. This is the full ladder from
zero to video; `docs/BRINGUP.md` stays the detailed P2.2 procedure.

Why Linux: nothing claims the sensor's vendor interface (MI_04), so the
driver takes it directly while the kernel (uvcvideo) owns the video
interface - one box, one cable, no USBPcap needed. A live USB with
`rustup` is enough.

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

All writes are the exact observed vendor bytes: T5/T7 send the same
register tables the vendor engine sends on every session (volatile
image-sensor registers - a power cycle clears them; the T24C16A EEPROM
is never written, the driver has no write path to it at all).

## Step 0 - prepare the box (once, ~15 min)

```sh
curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
. "$HOME/.cargo/env"
git clone https://github.com/thomas7475/Idong100.git
cd Idong100

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

Expected: `82 passed` in the lib (both runs), all integration tests ok.

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
three 0x0205 stream polls.

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

Runs the full session init (as T5), picks the sensor's video node
(`--device /dev/videoN` to override), negotiates 640x480, streams
mmap buffers while polling 0x0205 at ~10 Hz, and writes per-channel
PNGs + raw .bin frames into `video_runN/`.

PASS (per the turn-35 capture predictions):

- frames arrive (~8 fps; 30 fps is only the negotiated upper bound),
- `614400 B` per frame = 640x480x2 (the two-channel raster),
- channel A stats show real contrast (non-flat scene image),
- channel B stats are present (the unidentified second channel).

Anything else is still valuable: a FAIL with its console output + the
generated PNGs decides the next move (V4L2 promotion into the crate vs
the usbfs-ISO backend).

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
