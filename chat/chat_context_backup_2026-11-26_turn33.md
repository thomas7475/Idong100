# Chat context backup - turn 33 - 2026-11-26

Project: idong100 (Toltech / i-dong "idong Sensor V2.6.0.6", USB 0471:2600
boot -> 0471:2500 running). Repo:
https://github.com/thomas7475/Idong100.git (main 7a320b8 at turn 33
start, project/state 58b85aa, chat/context 00999df). Phase 2: USBPcap
capture kit + Rust driver; goal: the camera as 3D scanner.

## User message of turn 33

The repo link + a fresh PAT + "i got this latest capture and i installed
the idong software but i do not get the lobby to lunch it stays at the
screen that says Connecting to idong sensor... research this why we no
not get future are there other ways to start a app, game that uses video
so we can capture the usb traffic, if you make modification to the
software make it in a robust way." Uploads: idong_capture7.zip (300 s),
start-after9.png (the stuck screen), this backup chain.

Standing rules: English only; zips + chat backup in download/ every turn;
PAT never printed or stored; pure cmd .bat, no PowerShell.

## What turn 33 found

1. cap7 decoded (scripts/usbpcap deep dive): 8 full session-init cycles,
   3x complete 8051 flash (t=46/216/257), the sensor spontaneously
   re-enumerates as 0471:2600 by itself every 31-157 s (t=209.7, 255.7;
   same in cap2), engine re-flashes each time. UVC property queries
   (A1 GET_INFO/MIN/MAX/RES/DEF) after every re-enum = the app DID open
   the camera via UVC; SET_INTERFACE alt 0 only (no stream ever).
   2500 composite: if0 VC + if1 VS (ep 0x85 iso alt1/2) + if2/3 audio
   (ep 0x84 iso) + if4 vendor (ep 0x02/0x82 7f55 pipes + 0x03/0x83 512B,
   unused so far).

2. THE ROOT CAUSE of the stuck "Connecting to idong sensor" screen
   (static analysis of xEyeUSB.dll 2.6.0.6, capstone + RTTI):
   McuManager25::connect (VA 0x10006500) -> ... -> version loop (3x)
   calls xeyezHal_DeviceVerRead(4) = 0107 EEPROM read @0x0322 (5 B,
   "000T"+E4) and compares byte 4 against the constant "DSHY" at VA
   0x10016034. Accepted: D, S, H, 0x00. This unit's 'T' fails all 3
   attempts -> connect false -> USB main thread post-loop Sleep(5000) +
   stage back to 1 -> the observed 10 s retry cycle -> the UI stays at
   "detecting sensor" forever. v2.5.0.7 has NO such call (the gate is
   new in 2.6.0.6; the unit is likely a "TaiShan" edition, letter T).
   All other engine paths verified OK on this machine: special-device
   detection (AutoPreDetectDevice -> CheckSpecialDevice HWID table
   0666/0668/2100/2600/2500&MI_04), edition check (turn-31 patched
   GetLobbyEdition -> 0 = accepted), session init, GetMcuVersion "2619".

3. The device-ID check that unlocks "running" (state 7): only runs on
   MESSAGE 5 after connect succeeds - never reached so far. It is
   xeyezHal_DeviceIDRead(32) = 0107 read @0x0300 (33 B, backup @0x0700,
   byte-sum checksum). The unit's EEPROM dump proves it intact:
   "AB460DC8BA12C6840B49CD5786C329DD" + 0x50 at both 0x300 and 0x700.

4. "FPGA blobs" decoded: the 30fps/*.dat files are TEXT register tables
   ("0x06,0x02\r\n0x07,0x81\r\n...") = the observed 0x0103/0x0203 8-B
   chunks and 0x0104/0x0204 CMOS tables. No bitstream is ever uploaded.

## The fix (robust, as requested)

Kit download/idong100_engine33 (in idong100_kit33.zip with the turn-32
capture kit): ONE byte of DLL data - xEyeUSB.dll file offset 0x16036
'H'(0x48) -> 'T'(0x54) (constant "DSHY" -> "DSTY", so "000T" matches ->
edition 2, the edition check is skipped, connect succeeds). The turn-31
upsell bypass (0x7520, 0x77d0) is kept. Three reference DLLs (vendor
md5 00563d0f..., t31 86709857..., t33 47f5bbee...); patch_engine33.bat
(admin; registry/standard-path install discovery, dir /s /b; state
detection by fc /b against all three refs; refuse foreign; backup
.idong33-backup; verify; rollback), restore_engine33.bat,
verify_engine33.bat. Pure cmd, CRLF, ASCII. Decision logic simulated
over all four states: vendor->patched, t31->patched, t33->noop,
foreign->refuse, restore->byte-identical vendor.

## Other ways to run a video app (user's question)

- xEyeThirdLens.exe (engine folder): standalone middle-camera
  calibration/viewer (xEyeEngine Init/GetxEyePipeData +
  MiddleCameraCalib) - needs the engine running (so also the patch).
- Games: the lobby is a TCP server (portlobby 40000, portbuddy 40001);
  pingbisetup.exe installs the ping-pong game.
- v2.5.0.7: no version gate (would accept this unit as-is) but has the
  same upsell; the turn-31 patch bytes target the 2.6 DLL only.
- CapStill.exe is an OLE document tool, NOT a camera app.

## Driver + repo updates

- idong/src/protocol/runtime.rs: read_device_id() (0107@0x0300 33 B,
  backup 0x0700), edition_tag_accepted() (0107@0x0322, byte 4 vs
  DSHY/NUL), constants EEPROM_DEVICE_ID_*, EDITION_LETTERS.
- idong/tests/runtime_replay.rs: turn33_engine_gate_reads (exact 0107
  request bytes, "000T" vs "000D", the 32-char ID + checksum 0x50,
  backup fallback). 49 tests green (Rust 1.100.0 installed in-sandbox).
- docs/PROTOCOL.md "Turn 33" section;
  research/idong100/agents/15-captures33/FINDINGS.md.
- Branch project/state-turn33 (fb3364c) -> merge into main like turn 32.

## Next step for the user

1. Unzip idong100_kit33.zip (e.g. C:\idong_engine33), right-click
   patch_engine33.bat -> Run as administrator.
2. Start the idong software - the tray eye should go "running" and the
   lobby should appear. (If a Chinese box about hardware appears at
   any point, tell me - that would be the 0666 freeze, not expected.)
3. Start a game or xEyeThirdLens.exe so the camera view runs.
4. run_capture.bat ~60 s while the video is visible; send the folder.
   Expect: the 0107 read @0x300 (device ID), the FPGA/table registers,
   and finally the ep 0x85 video stream / marker records.

Tools this turn (scripts/): cap7_deep.py/cap7_deep2.py/cap7_deep3.py
(capture decode), eng33.py/usbstate33*.py/xref33.py/flag33.py/
vercheck33.py/callers33.py/check33.py/detect33.py/detect2_33.py/
drvchk33*.py/devid33.py/tail33.py/pipeline33.py/statecallers33.py/
strdump33.py (engine static analysis), kit33_build.py/kit33_bats.py
(the kit).
