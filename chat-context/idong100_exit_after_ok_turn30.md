# The software closes after OK - root cause and fix - turn 30 - 2026-11-25

User report: after closing the upsell dialog with OK, the PC software
terminates entirely; the R and G menu items do nothing. Screenshot:
win7-afterdriver.install-menue.png (sensor-settings window + tray menu with
accelerators R/P/S/I/G/X; G entry grayed).

## What the menu is

The R/P/S/I/G/X menu is the sensor tray-app context menu:
R = run/start, P = pause, S = settings, I = info, G = Sports King (grayed
because the edition check failed), X = exit. The window in the screenshot is
the sensor-settings tool ("idong sensor - settings", device-info page, device
field showing a dash = no engine running, because the engine process is dead).

## Root cause (disassembly, xEyeUSB.dll v2.6.0.6; identical in v2.5.0.7)

The engine's device-init function (sub_6500) calls the edition check
sub_7520:

1. RegOpenKeyA(HKLM, "SOFTWARE\TOLTech")
2. RegQueryValueExA("Location") -> install path
3. LoadLibraryA(<Location>\lobby\ConfigManagement.dll)
4. GetProcAddress("GetLobbyVer") -> call GetLobbyVer()

Any failure (missing key / missing value / missing DLL / missing export /
negative return) returns -1/-2. On a negative result the init code calls
sub_77d0:

  FindWindowA("xEyezEngine2009")
  MessageBoxA(hwnd, "您购买的是爱动体感运动机(大众版)...请拨打咨询热线
                  400-1818-520。", "idong感应器引擎")
  MSVCR80.dll!exit(-1)     <- the ad message box is followed by exit(-1)

So the message box you see is not a notification - it is the death notice:
when you press OK, exit(-1) runs and the software closes. That is why nothing
else can be done and why the tray menu buttons do nothing (the engine is gone;
the settings tool shows a dash for the device).

Why the check fails on the Win7 machine: the standalone v2.6.0.6 /
v2.5.0.7 installs never create HKLM\SOFTWARE\TOLTech\Location and contain no
lobby\ConfigManagement.dll at all (verified across both NSIS trees - neither
xeyez_alone nor xeyez_Lobby has the file). The full Sports-King lobby
software carries ConfigManagement.dll, but with a different, newer
GetLobbyVer signature (4 pointer arguments, writes results via pointers,
returns void-ish) while the engine calls GetLobbyVer() with no arguments and
reads eax - so even copying the sportsking DLL would return garbage.

## The fix (delivered as a fix kit)

1. `download/lobby_shim/ConfigManagement.dll` - a hand-built 32-bit PE
   shim (1.5 KB) exporting exactly `int __cdecl GetLobbyVer(void)` returning 2
   (Sports King PC edition code; any value >= 0 passes the check). Build
   script: scripts/build_shim30.py; verified with LIEF (export present, code
   = `mov eax,2; ret`).
2. `download/fix_idong_alive30.bat` - run as Administrator on the Win7
   machine next to the lobby_shim folder:
   - sets HKLM\SOFTWARE\TOLTech\Location = the install dir (edit the
     INSTALLDIR line at the top if the engine is installed elsewhere - the
     .bat checks that xEyeUSB.dll is really there),
   - creates `<install>\lobby\` and copies the shim DLL into it.

After that, start the software again: the edition check succeeds, the ad +
exit(-1) path is never taken, the engine stays alive, and the tray menu items
(R/G) get a live engine to talk to.

## Impact on the driver project

- The USB capture of turn 28 stays valid: the runtime protocol (0701/0d00/
  0f04) was the engine polling while alive; it had not yet reached the
  DSP/FPGA/iso stage inside the 108 s window.
- With the software kept alive by this fix, the NEXT capture can finally run
  the session long enough to reach the camera stage: run the vendor app,
  wait until the live camera image appears, keep it running ~60 s, capture
  with the CAPTURE_KIT.
- Our own Rust driver never executes this code path (it is vendor-lobby
  bookkeeping, not sensor control), so nothing in the driver design changes.

## Code map (xEyeUSB.dll v2.6.0.6, image base 0x10000000)

| Function | VA | Role |
| --- | --- | --- |
| init step | 0x10006500 | reads xEyeUsb.ini BitFile, sets MCU version, classifier retry x3, edition check retry x3 |
| edition check | 0x10007520 | registry + LoadLibrary + GetLobbyVer, returns -1/-2 on failure |
| classifier | 0x100076d0 | device edition letter D/S/H/Y/K -> codes 0/1/2, -2 unknown |
| ad + exit | 0x100077d0 | FindWindow("xEyezEngine2009") + MessageBoxA + MSVCR80!exit(-1) |
| exit call sites | 0x10006847, 0x100068a6, 0x100068dd, 0x10006b5b, 0x10006ba6 | all reached only from failed checks |
| ad text | 0x10016060 / 0x100160d8 | "您购买的是爱动体感运动机(大众版)..." |
