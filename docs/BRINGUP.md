# BRINGUP.md - the P2.2 read-only hardware test (turn 37)

What to run on the bench with the real sensor, in what order, and what
PASS looks like. Everything in this procedure is READ-ONLY against the
sensor: every byte sent is a version request (0x0008) or an EEPROM read
(0x0107) that the vendor engine also sends. No flash, no register
writes, no stream, no EEPROM writes. The only failure mode is a failed
read.

**Turn 38 update**: this test is step T3 of the full bench ladder
(`docs/LINUX_TESTS.md`) which continues with the full session (T5,
`examples/session_full.rs`) and the video bring-up (T6/T7,
`examples/video.rs`) - same style, same PASS/FAIL output, writes gated
behind `--cmos`.

## Why Linux

On Windows the vendor driver stack (xeyez.sys + the engine) claims the
device. On Linux nothing binds to the vendor interface MI_04, so the
driver can take it directly, and the UVC interfaces stay with the
kernel. Any Linux box or live USB works (Ubuntu/Debian/Arch; a live
USB with `cargo`/`rustc` preinstalled is enough - or build a static
binary elsewhere and copy it).

## Step 0 - prepare the box (once, ~15 minutes)

```sh
# rustup (if not present)
curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
. "$HOME/.cargo/env"

# the driver
git clone https://github.com/thomas7475/Idong100.git
cd Idong100

# udev rule: unprivileged access (src/transport/udev.rs)
sudo tee /etc/udev/rules.d/42-idong.rules <<'EOF'
# Toltech i-Dong sensor (idong crate) - unprivileged libusb access
SUBSYSTEM=="usb", ATTRS{idVendor}=="0471", ATTRS{idProduct}=="2600", MODE="0666"
SUBSYSTEM=="usb", ATTRS{idVendor}=="0471", ATTRS{idProduct}=="2500", MODE="0666"
EOF
sudo udevadm control --reload-rules
sudo udevadm trigger
# then replug the sensor

# libusb (most distros have it; the usb feature needs it)
sudo apt install -y libusb-1.0-0-dev   # Debian/Ubuntu
```

## Step 1 - discovery (READ-ONLY, no interface claimed)

```sh
cargo run --features usb --example list
```

Expected with the sensor plugged in:

```
bus   1  addr  12  0471:2500  Runtime
PASS: 1 sensor device(s) found
```

- `0471:2500` = runtime (the vendor software already flashed the MCU)
- `0471:2600` = bootloader (just plugged in / after a self-reboot)
- both can appear for a moment; the runtime device is the one used

If nothing is found: plug the sensor in; in a VM also attach it to the
VM. `lsusb | grep 0471` is the independent cross-check.

## Step 2 - THE test: the read-only bring-up

```sh
cargo run --features usb --example bringup
```

With the runtime device present, expected output:

```
=== idong P2.2 read-only bring-up ===
[1/3] enumerate
      bus   1 addr  12 0471:2500 Runtime
      PASS: 1 sensor device(s) found
[2/3] runtime device 0471:2500, opening MI_04 read-only (bus 1, addr 12)
      PASS: vendor interface MI_04 claimed (UVC/UAC untouched)
[3/3] read-only session (version + four EEPROM reads)
      PASS: version "2619" (runtime)
      device ID   : AB460DC8BA12C6840B49CD5786C329DD
      edition tag : 000T (code A)
      version code: 2613 (code B)
      calibration : 120 bytes
      PASS device ID is 32 hex chars / PASS codes are 4 chars / PASS calibration is 120 B
      PASS calibration parses: baseline 100.0 mm, focal 432.3 px
PASS: read-only bring-up complete.
```

Your unit should show exactly the device ID above (it matches the
24c16 dump and every capture) and the "000T" edition tag (the letter
that the vendor version gate refuses - the reason this project exists).
Values that differ per unit: none expected on your sensor.

If the sensor is in its bootloader instead (right after plugging in,
or after one of its self-reboots), the example reports PARTIAL with the
version "2600" - run it again once the vendor software (or the engine
on the Windows box) has flashed the MCU.

## Step 3 (optional) - bootloader probe

```sh
cargo run --features usb --example probe_bootloader
```

Expected in the bootloader state: `PASS: bootloader answered, version
"2600"`. This is the Rust port of idong_probe.py - one version
request, nothing else.

## What to send back from the bench day

1. The full `bringup` output (photo or text - the terminal text is
   better).
2. If anything FAILs: the `list` output too, plus `lsusb -v -d 0471:2500`
   (or 2600).
3. Optional but valuable: a `run_capture.bat` capture (kit 37) of the
   bring-up run on the Windows box - it byte-compares what the driver
   sent against the vendor captures.

## What this proves

- the rusb transport opens the real device and claims MI_04
  (P2.1 verified on hardware);
- the request bytes are byte-identical to the vendor engine's
  (golden-tested in CI, now confirmed against the real device);
- the EEPROM of the real unit answers with the expected identity and
  calibration (P2.2 acceptance);
- the re-enumeration watcher and the session layer have a live device
  for their next phase.

## What this deliberately does NOT do

- flash the MCU: the flash sequence is byte-pinned and mock-tested, but
  the `FLASH_CONFIRMED` gate stays closed until one controlled
  hardware replay (P2.3) - and it is not needed, the vendor engine
  re-flashes on every bootloader landing anyway;
- the CMOS register writes (0x0104/0x0203/0x0103), the poll cycle and
  the stream poll: all mock-verified, all write-capable, all gated
  behind later phases;
- anything with EP 0x85 (video) - that is the P2.5 backend.
