# Chat context backup - turn 36, 2026-11-27

## Project state (one paragraph)

idong100 reverse engineering. The vendor stack (lobby + games) is
unlocked layer by layer; the Rust driver (`idong` + `idong-transport`)
implements the fully-decoded wire protocol with 64 golden tests green.
The wire protocol is COMPLETE (turn 35 captured the video plane;
turn 32/33 pinned boot flash + session + gates). The user's machine:
sensor connects, lobby starts to load, the "hardware test" fails with
the authorization box - SOLVED this turn (see below). The 3D-scanner
goal (marker records / depth) still needs one game-session capture.

## The dialog of this turn (start-after12.png)

"对不起，您的感应器硬件没有授权！请联系泰山在线！" (Sorry, your sensor
hardware is not authorized!) - identified as
`xEyeWarrningUI.dll!ShowDeviceIDInvalidWindow`: the engine's
`Chs\images\` holds the exact dialog elements
(ShiBieQiWeiShouQuan2.bmp, sorrytext.bmp, DeviceID.bmp, phone.bmp,
email.bmp, bbs.bmp - title/orange line/phone/e-mail/forum of the
screenshot, one to one). Trigger: the license gate in xEyeRight.dll
(MakeFinalResult can never report authorized - the server
us.i-dong.com is dead). Root cause of the FAILED fix delivery: the
turn-34 kit shipped only patch SCRIPTS - its reference DLLs
(xEyeUSB\, xEyeRight\ subfolders) were never committed to the repo,
so patch_engine34.bat could not run from the repo. The user's current
kit state: xEyeUSB t33 applied (that is why the sensor connects and
the lobby loads), xEyeRight unpatched (that is why the hardware test
fails).

## What was delivered this turn

1. **tools/kit36** (also idong100_kit36.zip in the project
   downloads): the consolidated kit with the FINISHED PATCHED DLLS:
   - patched/xEyeUSB_t33.dll (md5 47f5bbee... = the historical turn-33
     file, verified)
   - patched/xEyeRight_t34.dll (md5 98d7a90e... - the license fix:
     MakeFinalResult at 0x927 always final result 1 + the vendor's own
     0x7E8 success notification, reloc-safe)
   - patched/xEyeRight_t34_v25.dll (the same on the 2.5.0.7 build)
   - ref/: 5 vendor/intermediate references for state detection
   - patch_all36.bat (admin; robust finder; state check via fc /b;
     copy + backup + verify + rollback), verify_all36.bat,
     restore_all36.bat, run_direct36.bat + the capture tools
     (run_capture, check_capture, test_usbpcap,
     check_sensor_attached, predir)
   - proofs: unicorn emulation 16/16 (patched always authorized +
     notification; vendor follows original logic), LIEF structural
     check, decision simulation 11/11, CRLF + ASCII assertions
2. **Rust driver**: `idong/src/session.rs` (NEW) - Session::probe /
   connect / recover / poll_cycle / poll_stream + the
   IdongError::NeedsReenumeration hand-off; the self-reboot recovery
   re-flashes on every bootloader landing exactly like the vendor
   engine (the sensor self-reboots every 31-157 s). PU/OD boot model
   (protocol/boot.rs) explicitly marked DISPROVEN on the wire.
   64 tests green, nightly-fmt clean, workspace check clean.
3. **docs/DRIVER_STATUS.md** (NEW): the how-far/what-is-missing
   report. Missing: P2.2 live bring-up (bench day), reenum transport
   wiring, V4L2/usbfs video backend, the marker-record trigger (never
   on the wire in any capture - next capture should run a GAME), P4
   ROS 2 node.

## The user's next steps

1. Download idong100_kit36.zip, unzip, run patch_all36.bat as admin,
   then verify_all36.bat.
2. Start the lobby - the hardware test should pass now (the engine
   host stops showing the not-authorized window once xEyeRight.dll
   reports authorized).
3. Optional but valuable: one USBPcap capture with a RUNNING GAME
   (not xEyeThirdLens) - the marker records / EP 0x03-0x83 traffic
   are the last unknowns of the protocol.

## Driver quick status (details in docs/DRIVER_STATUS.md)

- protocol: COMPLETE (boot flash, session, EEPROM, CMOS, 0x0205
  stream poll, UVC video raster + probe/commit)
- core: DONE (64 tests) | transport: code done, hardware untested
- missing: P2.2 bring-up, reenum wiring, video backend, marker
  trigger, ROS 2 node

## Branch state

main @ turn 36 (state commit 3e29dca + this backup on chat/context).
Research agents: 18-turn36 (FINDINGS.md) is the full evidence chain.
