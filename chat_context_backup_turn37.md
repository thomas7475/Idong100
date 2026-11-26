# Chat context backup - turn 37 - 2026-11-27

Project: idong100 (Toltech / i-dong "idong Sensor V2.6.0.6", USB 0471:2600
boot -> 0471:2500 running). Repo:
https://github.com/thomas7475/Idong100.git (main cc69a1f at turn 37 end,
branch project/state-turn37). Phase 2: kit unlock + Rust driver; goal:
the camera as 3D scanner.

## User message of turn 37

Repo link + fresh PAT + "can the source code be moved in a single src
folder with its own cargo.toml to make the version management easier,
implement the next driver phase and what test to run with the hardware,
the updated patch_all.bat do absolute nothing all i get is === idong100
engine state, kit 36 === and a blinking _, this happen to a view of the
other .bat as well, run_capture works but makes 4 [2/5] USB root hubs
(USBPcap4/1/2/3) only USBPcap1 captures anything and only in run1
folder for subsequent runs you have to delete the folder contents, can
you patch xEyeWarrningUI.dll to circumvent remove the license gate,
update the git repo make sure to use the existing naming conventions
from the older commits for idong100_phase2_turn and chat context backup
naming." Uploads: chat_context_backup_2026-11-26_turn33.md,
idong100_phase2_turn30_2026-11-25.zip (confirms the deliverable zip
naming convention idong100_phase2_turnNN_YYYY-MM-DD.zip).

Standing rules: English only; zips + chat backup in download/ every
turn; PAT never printed or stored; pure cmd .bat, no PowerShell; robust
modifications only.

## What turn 37 found

1. The kit-36 hangs, root causes (both fixed in kit 37):
   a. `net session` as the admin gate - needs the LanmanServer service;
      in a VM with it stopped/disabled it can block for minutes with no
      output. -> fltmc (local, instant).
   b. `dir /s /b "<root>\xEyeUSB.dll"` over every collected install
      root - a registry value like "C:\" in RootPath/Location makes cmd
      crawl the whole drive SILENTLY (the for /f consumes the output;
      the console shows only the header + blinking cursor).
      -> the kit-37 finder is BOUNDED: drive roots refused, depth <= 2
      subfolders per root (engine is root or root\Chs), every root
      printed, worst case a few seconds.
2. run_capture: the user's --extcap-interfaces hub list arrives as
   USBPcap4, USBPcap1, USBPcap2, USBPcap3 - the LIST ORDER does not
   track device numbers (kit 36's HUBDEV_1 was \\.\USBPcap4!). Fix: the
   kit reads the sensor LocationInformation (Port_#NNNN.Hub_#MMMM)
   from the registry for VID_0471 PID 2600 AND 2500 and captures only
   the root hub(s) the sensor sits on, mapped BY NAME (USBPcapN); the
   runN picker now takes the first runN that does not exist or exists
   but is EMPTY (kit 36 probed writability -> always reused run1);
   `run_capture.bat all` forces the old behavior.

3. xEyeWarrningUI.dll reversed + patched (the third gate, the user
   request): 3 exports - ShowDeviceIDInvalidWindow @ file 0x1F54,
   ShowChipIDInvalidWindow @ 0x2085, ShowNoiceWindow. The box function:
   GetModuleHandleA -> CreateDialogParamA -> SendMessageA 0x401 ->
   ... -> xor eax,eax -> BARE ret (leave;ret). The caller is xeyez.exe
   (tray wndproc): `call proc; add esp,4` = cdecl, 1 arg, caller
   cleans, return value IGNORED, then sets its shown-flag and returns
   normally - no exit, no retry loop. Patch: entry `xor eax,eax; ret`
   (31 C0 C3) at BOTH invalid-window functions, both builds (.text
   byte-identical), 6 bytes each, ShowNoiceWindow untouched, no reloc
   overlap, lief machine/EP/imports/exports unchanged. Unicorn proof
   (loader-like IAT stubs): vendor runs the full show path
   (CreateDialogParamA, SendMessageA, SetWindowPos, both singletons)
   and returns 0; patched fires ZERO imports, EAX=0, stack balanced -
   the box can never appear and the engine host continues normally.

## Kit 37 (tools/kit37, zipped as idong100_phase2_turn37_2026-11-27.zip)

One folder, one linear flow, all patched DLLs shipped:
patch_all37 / verify_all37 / restore_all37 / run_direct37 (all with the
bounded finder + fltmc), run_capture (fixed), check_capture /
test_usbpcap / check_sensor_attached / predir (carried, machine
validated), patched/ (xEyeUSB_t33, xEyeRight_t34, xEyeRight_t34_v25,
xEyeWarrningUI_t37, xEyeWarrningUI_t37_v25), ref/ (7 refs incl. both
WarrningUI vendors). Proofs: simulate_kit37.py 40 checks PASS (finder
layouts incl. drive-root refusal + deep-not-found, patch/verify/restore
state matrix, runN picker, hub selection with the user's real hub order,
verify state texts), lint_kit37.py PASS (CRLF, ASCII, labels, quotes,
parens, percent sanity), warn37_build.py (patch + emulation).

## The Rust driver: single crate + P2.2 bring-up

- Single crate (user request): idong + idong-transport merged - ONE
  src/, ONE Cargo.toml, ONE version. The rusb transport is the `usb`
  cargo feature (optional dep); plain `cargo test` stays hardware- and
  libusb-free; `cargo run --features usb --example bringup` on the
  bench. idong-ros2/idong_msgs stay standalone colcon packages with
  concrete values.
- NEW src/transport/reenum.rs: BusState classification + ReenumWaiter
  decision core (gone -> runtime, bootloader relanding, timeout) - pure,
  5 unit tests - + the rusb wait loop.
- NEW Session::connect_readonly: the four EEPROM reads only (device ID,
  code A, code B, calibration) - the P2.2 first-contact entry, golden
  test pins the exact 4x 0x0107 request bytes.
- NEW calib::parse_calibration_record: the live 120-B page + checksum,
  round-trip + rejection tests.
- NEW examples/bringup.rs + docs/BRINGUP.md: THE hardware test -
  enumerate -> open MI_04 read-only -> version "2619" -> EEPROM
  identity + calibration with PASS/FAIL, expected output documented,
  bootloader fallback included. examples/list + probe_bootloader
  implemented (were TODO stubs).
- 72 tests green (49 unit + 5 mock_replay + 6 runtime_replay + 11
  video_golden + 1 doc), both with and without --features usb; cargo
  +nightly fmt clean. Rust 1.100.0 (reinstalled in the fresh sandbox).

## Repo state (main cc69a1f)

Commits: analysis(turn 37), kit(turn 37), state(turn 37), merge of
project/state-turn37. docs/DRIVER_STATUS.md + README.md updated to the
single-crate layout; docs/BRINGUP.md new; docs/PROTOCOL.md paths updated
AND re-normalized to pure CRLF (115 stray LFs from the turn-36 append
restored); docs/CAPTURE_KIT.md points at kit 37.

## Next step for the user

1. kit 37: patch_all37.bat (admin) -> verify_all37.bat (must say: all
   three gates open) -> lobby or run_direct37.bat. The hardware test
   should pass now.
2. The Linux bring-up (docs/BRINGUP.md): any Linux box/live USB with
   the sensor attached; udev rule; cargo run --features usb --example
   list, then --example bringup; expect version "2619", device ID
   AB460DC8BA12C6840B49CD5786C329DD, tag 000T, calibration baseline
   ~100 mm / focal ~432 px. Send the output back.
3. Optional: a capture with a RUNNING GAME (marker records / EP
   0x03/0x83 - the last open protocol question).

Tools this turn (scripts/): warn37_disasm/full/verify/caller/caller2/
caller3/build.py (the DLL reversal + patch + emulation),
make_kit37_bats.py, make_kit37_capture.py, lint_kit37.py,
simulate_kit37.py.
