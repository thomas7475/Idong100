# idong100 kit 36 - the one-folder engine unlock + capture kit (2026-11-27)

This kit replaces every earlier kit (turn 17-35). It contains EVERY
patched file and EVERY script you need. Nothing has to be downloaded
or generated anymore. Pure cmd.exe, no PowerShell, all .bat files
CRLF, all scripts keep their window open.

## What this kit fixes

The sensor connects, the lobby starts to load - and then a box says
"对不起，您的感应器硬件没有授权！请联系泰山在线！" ("Sorry, your sensor
hardware is not authorized!"). That box is NOT a hardware defect:

- The box is drawn by `xEyeWarrningUI.dll` (its images folder holds
  sorrytext.bmp, DeviceID.bmp, phone.bmp, email.bmp, bbs.bmp - the
  exact elements of your screenshot), called by the engine host
  `xeyez.exe` = `ShowDeviceIDInvalidWindow`.
- It appears when the license check in `xEyeRight.dll` fails. That
  module phones home to `us.i-dong.com` - a server that has been dead
  for years - so a stock install can NEVER pass this gate. It is the
  third gate; the earlier kits only removed the first two.
- The turn-34 kit had the fix, but shipped only the patch SCRIPT -
  its reference DLLs were never delivered, so the license patch could
  not be applied. THIS kit ships the finished patched DLLs as files.

Gates removed by the two patched DLLs (all proven on the wire):

| Gate | File | Symptom when it fails |
| --- | --- | --- |
| upsell ad + exit(-1) | xEyeUSB.dll | app closes itself right after the ad |
| version-tag 'DSHY' vs your sensor's '000T' | xEyeUSB.dll | lobby stuck at "Connecting to idong sensor" forever |
| license / dead server us.i-dong.com | xEyeRight.dll | "hardware not authorized" box, lobby hardware test fails |

## The flow - four steps, in this order

1. **Unlock.** Right-click `patch_all36.bat`, "Run as administrator".
   It finds your engine install (registry, services, Program Files,
   C:\ and D:\), checks each DLL state and copies the right patched
   file over it (backup first, byte-verify after, roll back on any
   error). It cannot corrupt a file: every step is a copy plus a
   byte comparison. Reinstalling the vendor software WIPES the patch
   (the installer carries the pristine DLLs) - just run it again.
2. **Verify.** Double-click `verify_all36.bat` (read-only). Both DLLs
   must say UNLOCKED / AUTHORIZED.
3. **Run.** Start the lobby as usual. OR run `run_direct36.bat`:
   engine + camera viewer (xEyeThirdLens.exe) without the lobby.
   The tray icon should reach "running" (引擎正常运行) within ~15 s.
   If a box appears again, note its text and run verify_all36.bat.
4. **Capture (optional).** `run_capture.bat` (admin) records the USB
   traffic (USBPcap must be installed once; it already is on your
   machine). `check_capture.bat` reads the capture and prints the
   PHASE the session reached (boot flash / session / EEPROM reads /
   device ID / UVC / video). Send the whole run folder in.

## Restore

`restore_all36.bat` (admin) puts every DLL back: from its
.idong36-backup, or from this kit's ref folder (only when the current
file is in a known patched state - a foreign file is never touched).

## Files

| File | What it is |
| --- | --- |
| patch_all36.bat | unlock both DLLs (admin, backup + verify + rollback) |
| verify_all36.bat | read-only state report |
| restore_all36.bat | undo the unlock |
| run_direct36.bat | engine + camera viewer, no lobby |
| run_capture.bat | USBPcap capture of one full session |
| check_capture.bat | phase report of a capture (double-click) |
| test_usbpcap.bat | is USBPcap installed and recording? |
| check_sensor_attached.bat | does Windows see the sensor as 0471:2600 / 0471:2500? |
| predir.bat | helper: finds a writable capture folder (self-test when double-clicked) |
| patched/xEyeUSB_t33.dll | xEyeUSB.dll v2.6.0.6 with the upsell + tag patches |
| patched/xEyeRight_t34.dll | xEyeRight.dll v2.6.0.6 with the license patch |
| patched/xEyeRight_t34_v25.dll | xEyeRight.dll v2.5.0.7 with the same patch |
| ref/*.dll | the vendor originals, for state detection and restore |

## The patched DLLs (what exactly was changed)

xEyeUSB.dll 2.6.0.6 (md5 00563d0f...) -> xEyeUSB_t33.dll
(md5 47f5bbee..., byte-identical to the turn-33 kit):

| offset | was | now | effect |
| --- | --- | --- | --- |
| 0x7520 | 55 8B EC | 31 C0 C3 | edition check returns "ok" |
| 0x77D0 | 55 | C3 | upsell box + exit(-1) never run |
| 0x16036 | 'H' (48) | 'T' (54) | "DSHY" -> "DSTY": your sensor's "000T" passes |

xEyeRight.dll 2.6.0.6 (md5 e94851c7...) -> xEyeRight_t34.dll
(md5 98d7a90e...): MakeFinalResult at file offset 0x927 now writes
final result = 1 (authorized) and jumps into the vendor's own success
notification tail (SendMessage 0x7E8). No new absolute address - the
vendor call keeps its offset and relocation entry. The same patch on
the 2.5.0.7 build (md5 dcc4f261...) -> xEyeRight_t34_v25.dll.

Emulation proof (unicorn, real bytes, section-mapped, SendMessageA
captured): patched v2.6 and v2.5 always end with final result 1 AND
fire the vendor success notification, for every initial state; the
vendor files follow their original logic. 16/16 states pass. The
patched DLLs keep their machine type, entry point, import and export
tables (LIEF structural check).

## Notes

- The v2.5.0.7 xEyeUSB.dll (a different build) is NOT patched by this
  kit - it has no version-tag gate anyway; only its xEyeRight.dll
  gets the license patch.
- A reinstall of the vendor software restores the pristine DLLs
  (your install zips prove it). Run patch_all36.bat again.
- The capture folder is `C:\idong_capture\runN` (or the fallback
  folder predir.bat picks and prints). Copy the whole run folder out.
- Windows 7: everything above was tested against its cmd behavior.
  On Windows 10/11 use "Windows Terminal" cmd or a real cmd.exe.
