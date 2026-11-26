# Chat context backup — turn 41 — 2026-11-26

Project: idong100 (Rust driver for the Toltech i-Dong Sensor V2.6)
Repository: https://github.com/thomas7475/Idong100 (public), `main` = `8de435b`
No repository change this turn (question only).

## The request

"give me the test commands to run to test the current software"
(repo URL and a PAT in the message; the PAT was not needed - the repo is public).

## What was checked

`main` on GitHub is still `8de435b` (the turn-40 merge). A fresh anonymous
clone (`/home/z/my-project/work/clone41`) was used to run the commands the
way the user will:

| Command (from `software/`) | Result |
|---|---|
| `sha256sum -c firmware/SHA256SUMS` | 24 x OK |
| `cargo test` | 100 passed, 0 failed |
| `cargo test --features usb` | 100 passed, 0 failed |
| `cargo run --features usb --example flash_mcu` (dry run) | PASS, hex from `software/firmware/v2.6.0.6/app_m8051_2611.hex` |
| `list`, no sensor attached | "no i-Dong sensor on the bus", exit 0 |
| `bringup`, no sensor | "no sensor found - PASS criteria not met" |
| `video -- --probe`, no sensor | "FAIL: no /dev/video* nodes at all on this system." |

No sensor is attached in the sandbox, so T2-T8 were not run here.

## The commands handed to the user

```sh
cd Idong100/software
sudo apt install -y libusb-1.0-0-dev

# T1 - no hardware
cargo test
cargo test --features usb
sha256sum -c firmware/SHA256SUMS
cargo run --features usb --example flash_mcu

# T2-T8 - sensor plugged in
cargo run --features usb --example list
cargo run --features usb --example bringup
cargo run --features usb --example probe_bootloader
cargo run --features usb --example session_full
cargo run --features usb --example session_full -- --cmos
cargo run --features usb --example video -- --probe
cargo run --features usb --example video -- --cmos --frames 10
cargo run --features usb --example flash_mcu -- --confirm
```

Full ladder with PASS criteria: `software/docs/LINUX_TESTS.md`.

## Open items (unchanged)

- Per the turn-39 notes T1-T5 passed on the real sensor; T6 (rerun), T7 and
  T8 are the open bench steps. One clean T8 PASS flips `FLASH_CONFIRMED`.
- The PAT was pasted into the chat again: rotate it.
