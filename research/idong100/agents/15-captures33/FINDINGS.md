# Turn 33: capture 7 + why the lobby never came up

Date: 2026-11-26. Input: idong_capture7.zip (300 s, kit v31 DLL), the
user's screenshot start-after9.png (the app stuck at "正在连接idong感应器
..."), and the full NSIS-extracted v2.6.0.6 install from turn 12.

## What capture 7 contains

| Phase | Time | Evidence |
| --- | --- | --- |
| Sensor already running (2500) | t=0 | dev 3 = 0471:2500, 361-B config |
| Session cycle + polls | t=29.9-57.1 | 8 full init cycles; the tag reads |
| 8051 flash #1 | t=46.1-49.0 | 0101/335 blocks/0201/0301/0401, echo ok |
| Camera open (UVC props) | t=52.7-52.9 | 25 GET_INFO/MIN/MAX/RES/DEF A1 requests |
| Spontaneous reboot -> 2600 | t=209.7 | device re-enumerates as bootloader (no flash) |
| 8051 flash #2 | t=216.0-218.5 | after version reply "2600" |
| Spontaneous reboot -> 2600 | t=255.7 | third boot; flash #3 at t=256.9 |

The sensor re-enters the 0471:2600 bootloader on its own every 31-157 s
(the key/launch state is volatile; cap2 showed the same). The engine
re-flashes it each time. Control-transfer analysis (URB functions):
SET_INTERFACE only ever selects alt 0 - no video alt was ever started.

## The root cause of the stuck "Connecting to idong sensor" screen

Full static analysis of xEyeUSB.dll 2.6.0.6 (capstone; RTTI classes
McuManager/McuManager25/DeviceManager/xEyeUSBManager):

1. `McuManager25::connect` (VA 0x10006500): SetFpgaFile ->
   SetLensBuffer -> `xeyezHal_StartFlow` (the observed flash+session
   wire traffic) -> session init -> GetMcuVersion ("2619") ->
   edition check (the turn-31 patched GetLobbyEdition, returns 0 =
   accepted) -> **version-tag loop (3 attempts)**.
2. The loop (VA 0x100076d0): `xeyezHal_DeviceVerRead(4)` =
   **`0x0107` EEPROM read at 0x0322, 5 B** -> "000T"+0xE4 (on the wire,
   3 reads per cycle, and in the T24C16 dump). The FOURTH byte is
   compared against the constant at VA 0x10016034 = **"DSHY"**
   (4th entry 0x00): 'D'->edition 0, 'S'->1, 'H'->2, 0x00->2. **'T'
   matches none -> -2 -> after 3 attempts: close device, connect
   returns false.**
3. The USB main thread post-loop (VA 0x1000ac70): mode==2 -> Sleep
   5000 ms -> stage back to 1 -> reconnect. **Result: the observed
   ~10 s retry cycle and the UI frozen at "detecting sensor"
   (state 2) forever.** (The state machine: GetxEyeUSBState ->
   state-update 0x1000b2c0; stage() returns 1 while stage==4 and the
   device-ID-check flag [0x1001c332]==0, or while stage==1.)
4. v2.5.0.7 has NO DeviceVerRead call and no "DSHY" constant - the
   gate is new in 2.6.0.6. This unit (letter 'T', likely "TaiShan"
   edition hardware) is refused by the 2.6 PC software.

## Why nothing was missing before this turn

The device-ID check that unlocks "running" (message 5 ->
check_device_id @0x1000a310 -> `xeyezHal_DeviceIDRead(32)` =
`0x0107` read at **0x0300, 33 B**, backup 0x0700) only runs AFTER the
connect succeeds - which it never did. That is why the 0x300 read is
absent from every capture so far, and why the lobby (games connect via
the TCP server on port 40000, `portlobby` in conf/version.xml) could
never appear. The EEPROM areas are intact on this unit:
- 0x0300/0x0700: "AB460DC8BA12C6840B49CD5786C329DD" + sum 0x50 (valid)
- 0x0322/0x0722: "000T" + 0xE4 (valid, but the letter 'T' is refused)

## The fix (kit idong100_engine33, in download/)

One byte of DLL data: **file offset 0x16036 'H'(0x48) -> 'T'(0x54)**,
turning the expected constant "DSHY" into "DSTY" so this unit's "000T"
matches (-> edition 2, the edition check is skipped). The turn-31
upsell bypass (offsets 0x7520, 0x77d0) is kept. Kit contents: three
reference DLLs (vendor md5 00563d0f..., t31 86709857..., t33
47f5bbee...), patch_engine33.bat (admin; finds the install, refuses
foreign builds, backs up, patches, verifies, rolls back),
restore_engine33.bat, verify_engine33.bat. Decision logic simulated
over all four states: vendor->patched, t31->patched, t33->noop,
foreign->refuse, restore->byte-identical.

## Other ways to run a video app (the user's question)

- **xEyeThirdLens.exe** (in the engine folder): a standalone
  middle-camera calibration/viewer - imports xEyeEngine.dll
  (InitxEyeEngine, GetxEyePipeData) and MiddleCameraCalib.dll. Needs
  the engine running, so it also needs the unlock patch.
- **Games**: the lobby is a TCP server (port 40000; portbuddy 40001);
  games connect as clients. pingbisetup.exe installs the ping-pong
  game. Once the lobby appears, any game exercises the video path.
- **v2.5.0.7**: no version-tag gate at all (would accept this unit
  as-is, but has the same upsell - the turn-31 patch targets the 2.6
  DLL bytes; the 2.5 DLL differs).
- **CapStill.exe** (analysis/capstill) is an OLE document tool, NOT a
  camera app - ignore it.
- **Our own driver**: everything except the video stream is now
  understood; the stream (ep 0x85 alt 1/2 or ep 0x03/0x83) is the one
  missing protocol piece, expected to appear in the next capture.

## Driver updates (this turn)

- `idong/src/protocol/runtime.rs`: `read_device_id()` (0107 @0x0300,
  33 B, backup @0x0700), `edition_tag_accepted()` (0107 @0x0322,
  4th byte vs DSHY/NUL), constants `EEPROM_DEVICE_ID_OFFSET`,
  `EEPROM_DEVICE_ID_BACKUP_OFFSET`, `EEPROM_DEVICE_ID_LEN`,
  `EDITION_LETTERS`; doc-comments now explain the vendor gate.
- `idong/tests/runtime_replay.rs`: `turn33_engine_gate_reads` -
  replays the exact 0107 request bytes ("000T"/"000D", the 32-char
  device ID with checksum 0x50, the backup fallback). 49 tests green
  (Rust 1.100.0 installed in-sandbox).

## Next step for the user

1. Unzip the kit, right-click patch_engine33.bat, Run as
   administrator.
2. Start the idong software - the eye tray icon should switch to
   "running" and the lobby should appear.
3. Start a game (or xEyeThirdLens.exe) so the camera view runs.
4. Capture ~60 s with the kit's run_capture.bat - that session should
   finally contain the video stream / marker records.
