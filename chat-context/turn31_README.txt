idong keep-alive patch kit - turn 31 - 2026-11-25
===================================================

WHAT THIS FIXES
---------------
Your problem (turn 30/31): closing the upsell message box ("... upgrade to
Sports King PC ... 400-1818-520") makes the whole software exit, and the
tray menu items (R, G) do nothing.

Root cause (disassembled, xEyeUSB.dll 2.6.0.6): at startup the engine runs
an "edition check" - read registry HKLM\SOFTWARE\TOLTech\Location, load
<Location>\lobby\ConfigManagement.dll, call GetLobbyVer(). When that fails
(and on a standalone install it always fails - the installer never creates
the key or the file), the code shows that message box and then calls
MSVCR80.dll exit(-1). That is why the software dies when you press OK.

The turn-30 fix (registry value + tiny shim DLL) did not work on your
machine. This kit is the robust version: it patches the vendor DLL itself,
so it works no matter what the registry contains.

THE PATCH (4 bytes, nothing else changes)
-----------------------------------------
file offset 0x7520 (VA 0x10007520, the edition check):  55 8B EC ...  ->  31 C0 C3
    (xor eax,eax / ret - the check now always returns "valid, edition ok")
file offset 0x77d0 (VA 0x100077d0, upsell+close):      55           ->  C3
    (ret - the function that shows the box and calls exit does nothing)

Both offsets are inside the code of xEyeUSB.dll 2.6.0.6 (size 137584
bytes, sha256 f01838...8f1173). The patched file has sha256
e856624d79d354d4319d715d8e79cf93e7121a14dccdeb73bd086e11c8bdeb33.
Verified by CPU emulation (Unicorn) of the real bytes: original shows the
box + exit on every failure scenario (23/23), patched never shows it and
keeps running in all 23 scenarios; negative controls fail as designed.
The DLL is NOT Authenticode-signed (checked: no WINTRUST/IMAGEHLP imports,
no signature directory), so patching it does not break a signature.

KIT CONTENTS
------------
xEyeUSB\xEyeUSB.dll        patched DLL (install this)
xEyeUSB\xEyeUSB.dll.orig   pristine vendor reference (for verify/restore)
patch_idong_alive31.bat    installer - run as Administrator
restore_idong31.bat        undo - restores the vendor DLL (backup first,
                            else the kit reference)
verify_idong31.bat         read-only check: patched / unpatched / other build

HOW TO USE
----------
1. Unzip the kit anywhere, e.g. C:\idong_patch31.
2. Right-click patch_idong_alive31.bat -> Run as administrator.
   The .bat finds the install folder itself (C:\Program Files\TOLTech,
   C:\Program Files (x86)\TOLTech, C:/D:\TOLTech, plus the registry
   Location values). It refuses to touch any file that is not exactly the
   vendor 2.6.0.6 build, saves a backup (xEyeUSB.dll.idong31-backup),
   copies the patched DLL in, and verifies the result byte for byte.
   If it cannot find your install folder, open the .bat in Notepad and add
   the folder to the ROOTS line.
3. Start the idong software. No upsell box, no exit behind it; the tray
   menu works while the engine runs.
4. To undo: run restore_idong31.bat as Administrator.
5. To check the state at any time: run verify_idong31.bat (read-only).

LIMITS
------
- Only for xEyeUSB.dll 2.6.0.6 (both "alone" and "Lobby" installs use the
  identical DLL - byte-verified). The .bat detects a different build and
  changes nothing; send me such a file and I will build a matching patch.
- Verified by emulation + structural PE checks here; the sandbox cannot
  run 32-bit Windows binaries (no 32-bit Wine), so the final proof is the
  run on your Win7 machine.
- This removes vendor upsell/self-exit behaviour only; sensor function,
  USB traffic and the capture plan are untouched. The vendor software is
  provided to observe the device protocol; respect the vendor licence.
- The turn-30 registry entries on your machine are now irrelevant; if you
  want them gone: reg delete "HKLM\SOFTWARE\TOLTech" /f (Administrator) -
  the patched DLL never reads them.
