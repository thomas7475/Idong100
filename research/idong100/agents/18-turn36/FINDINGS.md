# Turn 36: the hardware-test dialog, the consolidated kit, the session layer

Date: 2026-11-27. Inputs: the user's report that the sensor now
connects and the lobby starts to load, but the "hardware test" fails
with a box "对不起，您的感应器硬件没有授权！请联系泰山在线！" (Sorry,
your sensor hardware is not authorized!) plus the screenshot
`start-after12.png`, and two requests: consolidate the bat files +
provide all patched files, and continue the Rust driver (how far,
what is missing).

## 1. The dialog identified - it is the license gate, the fourth layer

The box is NOT from the lobby and NOT a hardware defect. Element
search across the engine (v2.6.0.6 + v2.5.0.7 install zips) and the
lobby/games folder (idong.zip, 362 files):

- The exact text (设备编号, service@toltech.cn, bbs.idong.com) is in NO
  binary as GBK or UTF-16 - the dialog text is BAKED INTO BITMAPS.
- The engine's `Chs\images\` folder holds EXACTLY the elements of the
  screenshot: `ShiBieQiWeiShouQuan2.bmp` (598x34, "识别器未授权" =
  sensor-not-authorized banner), `sorrytext.bmp` (598x34), `DeviceID.bmp`
  (85x17, the 设备编号 title), `phone.bmp` (263x20),
  `email.bmp` (361x21), `bbs.bmp` (265x21) - one-to-one with the
  screenshot layout (title bar, orange warning line, phone, e-mail,
  forum, green OK button).
- Those bitmaps belong to `xEyeWarrningUI.dll` (exports
  `ShowDeviceIDInvalidWindow`), drawn by the engine host `xeyez.exe` -
  the turn-34 finding, now proven by the dialog elements themselves.

So the flow is: sensor connects (the xEyeUSB gates are open - the
user's kit state proves the t33 patch is in), the lobby starts to
load (progress vs turn 35: the MC-calibration state no longer blocks
at "Connecting"), and then the ENGINE HOST shows
ShowDeviceIDInvalidWindow because the license check in `xEyeRight.dll`
never passes - `MakeFinalResult` can only copy the net result, and the
net result can never be 1 because the server `us.i-dong.com` is dead
since ~2012. The turn-34 fix (MakeFinalResult patch at file offset
0x927) was correct but never reached the user:

## 2. The real delivery gap - the turn-34 kit shipped no DLLs

`tools/capture/patch_engine34.bat` references reference DLLs under
`xEyeUSB\` and `xEyeRight\` subfolders that were NEVER committed to
the repo (the kit existed only in that session's download folder).
The user physically could not apply the license patch. This turn
closes the gap: the kit ships the finished patched DLLs as files.

## 3. The consolidated kit (tools/kit36, deliverables zip)

One folder, one linear flow, every patched file included:

| File | Role |
| --- | --- |
| patch_all36.bat (admin) | find install (registry 6 values + 2 services + Program Files + C:/D:), state-check each DLL via fc /b against the shipped refs, copy the right patched file (backup + verify + rollback) |
| verify_all36.bat | read-only state report: LOCKED / PARTIAL / UNLOCKED + LICENSE GATE ACTIVE / AUTHORIZED |
| restore_all36.bat | undo: from .idong36-backup, else from the ref folder (only known patched states - a foreign file is never touched) |
| run_direct36.bat | engine + xEyeThirdLens.exe without the lobby |
| run_capture.bat / check_capture.bat / test_usbpcap.bat / check_sensor_attached.bat / predir.bat | the capture tools (unchanged, machine-validated) |
| patched/ | xEyeUSB_t33.dll, xEyeRight_t34.dll, xEyeRight_t34_v25.dll |
| ref/ | the 5 vendor/intermediate references for state detection |

Patch state (byte-verified, all original bytes asserted before
writing):

- `xEyeUSB_t33.dll` md5 47f5bbee... - byte-identical to the turn-33
  kit (offsets 0x7520, 0x77D0, 0x16036; verified by md5 match).
- `xEyeRight_t34.dll` md5 98d7a90e... - MakeFinalResult guard block
  (0x2D bytes at 0x927) replaced by `xor edx,edx; mov dword ptr
  [ecx+0x24],1; jmp` into the vendor's own success tail (push 0; push
  0; push 0x7E8; push [ecx+0x1EC]; call [USER32!SendMessageA]); the
  vendor call keeps file offset 0x961 and its relocation entry
  (reloc-safe). NOP-padded to the end of the 0x2D-byte block.
- `xEyeRight_t34_v25.dll` md5 590e108d... - the same patch on the
  v2.5.0.7 build (guard region byte-identical in both, asserted).
- `ref/xEyeUSB_t31.dll` md5 86709857... - the turn-31 intermediate
  state, for detection (md5 matches the historical kit).

Proofs: unicorn emulation (section-mapped PEs, SendMessageA captured)
of all 16 combinations (4 files x 4 initial states): patched v2.6 and
v2.5 always end with final result = 1 AND fire the vendor success
notification (hwnd, 0x7E8, 0, 0); the vendor files follow their
original logic (final stays 0 on fresh guards, 3 after 20 failed
rounds, notify only on net result 1). LIEF structural check: machine,
entry point, import and export tables unchanged in every patched DLL.
Decision logic simulated in Python over a fake install tree: 9 patch
cases + 2 restore cases, all PASS (vendor->patched, t31->patched,
t33->skip, vendor25->noted with xEyeRight still patched,
foreign->refused; restore from backup and from ref both byte-verified).
All .bat: pure cmd.exe, ASCII, CRLF, windows stay open.

## 4. Expected result on the machine

With both DLLs patched the engine host stops showing the
not-authorized window, the tray reaches "running", and the lobby's
hardware test should pass (its sensor questions go through the pipe to
the engine, which is now fully live). If the lobby STILL fails after
this, the next suspect is the lobby's own VideoSign gate
(getMiddleCameraCalibrateState) - but that gate passed in turn-35
analysis whenever the engine is fully up, so the license patch is
expected to be the last one.

## 5. The Rust driver - how far, what is missing

Full answer in `docs/DRIVER_STATUS.md` (new). Short version: the
protocol is COMPLETE (boot flash, session, EEPROM, CMOS, stream poll,
UVC video - all byte-pinned from 5 captures); the pure core
implements all of it with 64 golden tests green; the missing pieces
are hardware bring-up (P2.2), the re-enumeration transport wiring, the
V4L2/usbfs video backend, the marker-record trigger (never observed
on the wire), and the ROS 2 node (P4).

New this turn - `idong/src/session.rs` (the lifecycle layer):

- `Session::probe` - one 0x0008 version read: "2600" = bootloader,
  "2619" = runtime;
- `Session::connect` - bootloader: write-risk gate + the byte-exact
  flash sequence, then `IdongError::NeedsReenumeration` (the transport
  hand-off: drop, reopen 0471:2500, reconnect); runtime: the observed
  session init (device ID, code A, code B, calibration, CMOS tables);
- `Session::recover` - the self-reboot recovery: the sensor drops to
  the bootloader every 31-157 s (3x in cap7, 1x in cap8); the vendor
  engine re-flashes on EVERY bootloader landing and never boots the
  resident image - the driver models the same behavior;
- `Session::poll_cycle` / `poll_stream` - the steady-state poll set
  (0003/0004/0005/000d/0404/040f) and the 30 Hz 0x0205 stream poll;
- `IdongError::NeedsReenumeration` added to the error model.

Test suite: 41 unit + 5 mock_replay + 6 runtime_replay + 11
video_golden + 1 doc = 64 green, `cargo +nightly fmt --check` clean
for both crates, `cargo check --workspace` clean. The old PU/OD boot
model (`protocol/boot.rs`) is now explicitly documented as DISPROVEN
on the wire (0 PU/OD frames in ~12,000 records; the real flash path is
`RuntimeSession::flash_sequence`) - kept as the historical DLL-side
decode with its gate never opening.

## 6. Artifacts

- `tools/kit36/` - the consolidated kit (also zipped to the project
  downloads as idong100_kit36.zip)
- `scripts/make_kit36_dlls.py`, `scripts/verify_kit36_emu.py`,
  `scripts/make_kit36_bats.py`, `scripts/simulate_kit36.py` - the
  generation + proof chain
- `docs/DRIVER_STATUS.md` - the driver status report

## 7. Next

1. User: `patch_all36.bat` (admin) -> `verify_all36.bat` -> lobby (or
   `run_direct36.bat`). The hardware test should pass now.
2. Optional capture with a RUNNING GAME (not xEyeThirdLens) - the
   marker records / EP 0x03-0x83 traffic that no capture has shown
   yet (the last unknown of the protocol).
3. Bench day: P2.2 live read-only bring-up on Linux
   (docs/DRIVER_STATUS.md, missing #1).
