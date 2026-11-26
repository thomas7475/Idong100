# Turn 37 findings - the box DLL patched, the kit-36 hangs fixed, the single crate

Date: 2026-11-27. Inputs: the user's report that patch_all.bat prints
only "=== idong100 engine state, kit 36 ===" and a blinking cursor
(same for a few other .bat files); run_capture works but opens 4 USBPcap
root-hub windows of which only USBPcap1 captures, and every run reuses
the run1 folder; the request to patch xEyeWarrningUI.dll (the license
box), to move the Rust code into a single src/ + one Cargo.toml, to
implement the next driver phase with its hardware tests, and to update
the repo with the established naming.

## 1. xEyeWarrningUI.dll reversed and patched (the third gate)

Static analysis of both engine builds (v2.6.0.6 + v2.5.0.7; the .text
sections are byte-identical between them):

- 3 exports: ShowDeviceIDInvalidWindow @ RVA 0x1F54,
  ShowChipIDInvalidWindow @ RVA 0x2085, ShowNoiceWindow @ RVA 0x2167.
  File offset == RVA in .text (raw off 0x1000 = VA 0x1000).
- ShowDeviceIDInvalidWindow: GetModuleHandleA -> CreateDialogParamA
  (dialog id 0x65, template 0x100018f4 - the box) -> SendMessageA
  (0x401) -> GetPrivateProfileIntA -> GetWindowRect/GetSystemMetrics/
  SetWindowPos (centering) -> xEyeUSBManager::getSingleton +
  xEyeClassManager::getSingleton vcalls -> xor eax,eax -> bare `ret`
  (leave; ret). On dialog failure: OutputDebugStringA + or eax,-1.
- The caller is xeyez.exe (the tray wndproc, RVA 0x40457D block):
  LoadLibraryA("xEyeWarrningUI.dll") once, GetProcAddress
  ("ShowDeviceIDInvalidWindow"), `call proc; add esp,4` - CDECL, ONE
  ARG, the caller cleans, the return value is IGNORED, then the wndproc
  sets its shown-flag (state 2) and returns 0 normally. A state 3
  check next to it is the chip-ID box slot.
- Conclusion: an entry patch `31 C0 C3` (xor eax,eax; ret) at the
  function entry is stack-safe (bare-ret epilogue + cdecl caller),
  byte-minimal, and the engine host continues its normal wndproc flow
  (no exit, no retry loop) - the box simply never appears.

The patch: ShowDeviceIDInvalidWindow @ file 0x1F54 AND
ShowChipIDInvalidWindow @ file 0x2085 in BOTH builds (6 bytes each;
ShowNoiceWindow untouched - the neutral notice). No relocation entry
overlaps the patched ranges (verified). lief: machine/EP/import/export
tables unchanged.

Emulation proof (unicorn, section-mapped image, IAT pre-filled with
loader-like stubs - see warn37_build.py):

- vendor DLL: the full show path runs (GetModuleHandleA,
  CreateDialogParamA, strcpy, both singletons, SendMessageA,
  GetPrivateProfileIntA, GetWindowRect, GetSystemMetrics x2,
  SetWindowPos, UpdateWindow) and returns 0, stack balanced;
- patched DLL: ZERO imports fire, EAX == 0, returns to the caller
  immediately, stack balanced. The box can never appear.

## 2. The kit-36 hangs (root causes found and fixed)

Two independent hang causes, both in the scripts the user named:

1. `net session` as the admin gate (patch_all36 + restore_all36):
   net session requires the LanmanServer service; in a VM with the
   service stopped/disabled it can block for minutes with no output.
   Fix: fltmc (local, instant, no service).
2. The install search: `dir /s /b "<root>\xEyeUSB.dll"` over every
   collected root. A registry value (RootPath/Location - the same
   values the finder trusts) of "C:\" or any large parent folder makes
   cmd crawl an entire drive SILENTLY - the for /f consumes the output,
   the console shows only the header + a blinking cursor for minutes.
   Fix: the kit-37 finder is BOUNDED - a root is refused if it is a
   drive root (normalized via `%%~fT`), and each root is scanned at
   depth <= 2 subfolders for <root>\xEyeUSB.dll and <root>\Chs\
   xEyeUSB.dll (the engine folder is always root or root\Chs in both
   installers). Every root checked is printed. Worst case: a few
   seconds.

## 3. run_capture repairs

- The hub mapping discovery: the user's --extcap-interfaces list
  arrives as USBPcap4, USBPcap1, USBPcap2, USBPcap3 - the LIST ORDER
  does not track the device numbers! (Kit 36 captured HUBDEV_1 =
  the first list entry = \\.\USBPcap4.) Fix: the kit reads the sensor
  LocationInformation (Port_#NNNN.Hub_#MMMM) from the registry for
  VID_0471 PID 2600 AND 2500 (the sensor re-enumerates between them on
  the same port), maps hub number -> device BY NAME (USBPcapN), and
  captures only that hub. Fallback: `all` argument / no location info
  -> every hub, named by real device number.
- The runN picker: kit 36 probed WRITABILITY (probe copy succeeded on
  the existing run1 forever). Fix: first runN that does not exist, or
  exists but is EMPTY (dir /b /a + findstr). Bounded at 999.

## 4. Decision-logic + static proofs of the kit

- simulate_kit37.py: 40 checks PASS - the bounded finder over 8 layouts
  (direct, Chs, sub\Chs, sub\sub\Chs, sub, deep-NOT-found, drive-root
  refused, trailing backslash), the patch/verify/restore state matrix
  (fresh vendor, t31, t33, kit-36 state, v2.5 engine, missing DLLs,
  foreign builds, restore from .idong3*-backups incl. kit-36 tags),
  the runN picker (run1 full -> run2, empty run3 reused), the hub
  selection with the user's real hub order (hub 1 -> \\.\USBPcap1 -
  the list index would have picked USBPcap4).
- lint_kit37.py: CRLF, ASCII, label/goto integrity, quote/paren
  balance, percent sanity - PASS on all 9 scripts.
- warn37_build.py: the DLL patch assertions + emulation (above).

## 5. The Rust driver - single crate + the P2.2 bring-up

- Single crate (user request): idong + idong-transport merged. One
  src/, one Cargo.toml, one version. The rusb transport is a cargo
  feature (`usb`), so plain `cargo test` stays hardware- and
  libusb-free; `idong-ros2`/`idong_msgs` stay separate colcon packages
  with concrete values (no workspace inheritance).
- NEW `src/transport/reenum.rs` - the re-enumeration handling (the old
  stub): pure decision core (BusState classification + ReenumWaiter
  with the observed event shapes: launch -> gone -> runtime, self-reboot
  -> bootloader relanding, timeout budget) + the rusb poll loop
  (wait_runtime). 5 unit tests.
- NEW `Session::connect_readonly` - the P2.2 first-contact entry point:
  the four EEPROM reads of the session init (device ID, code A, code B,
  calibration) and NOTHING else - no CMOS writes, no polls. Golden test
  pins the exact 4x 0x0107 request bytes.
- NEW `calib::parse_calibration_record` - the 120-B page parser for the
  live session read (payload + checksum, no 0xFF separator) with
  round-trip + checksum-rejection tests.
- NEW `examples/bringup.rs` - THE hardware test: enumerate -> open
  MI_04 read-only -> version "2619" -> the four EEPROM reads -> format
  checks (32-hex ID, 4-char codes, 120-B calibration that parses with
  baseline ~100 mm / focal ~432 px). Bootloader fallback path prints
  the PARTIAL result. docs/BRINGUP.md = the bench procedure.
- examples/list.rs + probe_bootloader.rs implemented (were TODO
  stubs); all live examples behind `--features usb`.
- Test suite: 72 green (was 64; +5 reenum, +1 connect_readonly, +1
  calibration record, +1 session... net +8), `cargo +nightly fmt
  --check` clean, `cargo test --features usb` also green (the rusb
  code compiles).

## 6. Artifacts

- tools/kit37/ - the consolidated kit (three gates, all patched DLLs
  shipped, the bounded finder, the fixed run_capture)
- docs/BRINGUP.md - the P2.2 bench procedure (what to run with the
  hardware, expected output, what to send back)
- docs/DRIVER_STATUS.md + README.md - the single-crate status
- scripts/warn37_*.py (the DLL reversal + patch proof),
  make_kit37_*.py, lint_kit37.py, simulate_kit37.py

## 7. Next

1. User: kit 37 - patch_all37.bat (admin) -> verify_all37.bat -> lobby
   or run_direct37.bat. The hardware test should pass now (all three
   gates open).
2. User: the Linux bring-up (docs/BRINGUP.md) - the first read-only
   contact of the Rust driver with the real sensor.
3. Optional captures: the lobby passing the hardware test end-to-end;
   a RUNNING GAME (the marker records / EP 0x03-0x83 decision).
