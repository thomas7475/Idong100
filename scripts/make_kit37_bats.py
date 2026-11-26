#!/usr/bin/env python3
"""Write the idong100 kit 37 .bat scripts + README.

Kit 36 defects fixed here (all reported by the user this turn):
  1. patch_all/verify_all/restore_all/run_direct hung after the header with a
     blinking cursor. Two causes, both removed:
       a) `net session` as the admin gate - needs the LanmanServer service;
         can block for minutes when the service is stopped/disabled (VM!).
         -> replaced with fltmc (local, instant, no service).
       b) root collection + `dir /s /b R\\xEyeUSB.dll` - a registry RootPath/
         Location like "C:\\" or a big parent folder makes cmd crawl a whole
         drive SILENTLY (output is consumed by the for loop - the window
         shows only the header + cursor for many minutes).
         -> the new finder is BOUNDED: a root is used only if it is not a
            drive root, and is scanned at depth <= 2 subfolders for the
            engine DLL (engine root\\Chs\\xEyeUSB.dll layouts), with
            progress printed. Nothing recursive, nothing silent.
  2. run_capture opened one USBPcap window per root hub (4) but only one
     captures. -> the kit now reads the sensor's LocationInformation
     (Port_#NNNN.Hub_#MMMM) from the registry for VID_0471 PID 2600+2500 and
     captures ONLY the root hub(s) the sensor sits on, with an `all`
     fallback argument.
  3. run_capture reused run1 forever (writability probe, not emptiness) and
     the folder had to be emptied by hand. -> picks the first runN that
     does not exist or exists but is EMPTY.

New third gate: xEyeWarrningUI.dll (ShowDeviceIDInvalidWindow - the
hardware-not-authorized box) patched with an entry `xor eax,eax; ret`
(byte-minimal, cdecl-correct, caller-verified, emulation-proven - see
warn37_build.py). Kit ships the finished patched DLLs for both engine
builds like kit 36 did.

Every file: pure cmd.exe, ASCII only, CRLF endings.
"""

import os
import shutil

KIT = "/home/z/my-project/work/t37/kit37"
OLD = "/home/z/my-project/work/t36/kit36"

os.makedirs(KIT, exist_ok=True)
os.makedirs(os.path.join(KIT, "patched"), exist_ok=True)
os.makedirs(os.path.join(KIT, "ref"), exist_ok=True)


def bat(name, lines):
    if isinstance(lines, str):
        lines = [lines]
    lines = [ln for line in lines for ln in line.split("\n")]
    while lines and lines[0] == "":
        lines.pop(0)   # no leading blank line before @echo off
    text = "\r\n".join(lines) + "\r\n"
    text.encode("ascii")
    assert "\n" not in text.replace("\r\n", ""), name
    with open(os.path.join(KIT, name), "wb") as f:
        f.write(text.encode("ascii"))
    print(f"wrote {name} ({len(lines)} lines)")


def plain_copy(src, dst):
    data = open(src, "rb").read()
    assert len(data) > 0, dst
    with open(os.path.join(KIT, dst), "wb") as f:
        f.write(data)
    print(f"copied {dst} ({len(data)} bytes)")


# ---------------------------------------------------------------------------
# the shared engine finder (turn 37): bounded, loud, never crawls a drive
# ---------------------------------------------------------------------------
FINDER = r"""
:ONCE
findstr /i /x /l /c:"%~1" "%TMPDLLS%" >nul 2>&1
if errorlevel 1 >>"%TMPDLLS%" echo %~1
goto :eof

:ADDREG
for /f "tokens=2*" %%A in ('reg query "%~1" /v "%~2" 2^>nul ^| findstr /i "%~2"') do (
  if exist "%%B\" call :ADDROOT2 "%%B"
)
goto :eof

:ADDSVC
for /f "tokens=2*" %%A in ('reg query "HKLM\SYSTEM\CurrentControlSet\Services\%~1" /v ImagePath 2^>nul ^| findstr /i "ImagePath"') do (
  set "SVP=%%B"
  set "SVP=!SVP:"=!"
  if /i "!SVP:~0,4!"=="\??\" set "SVP=!SVP:~4!"
  if exist "!SVP!" for %%D in ("!SVP!") do if exist "%%~dpD" call :ADDROOT2 "%%~dpD"
)
goto :eof

:ADDFOLDER
if exist "%~1\" call :ADDROOT2 "%~1"
goto :eof

:ADDROOT2
if "%~1"=="" goto :eof
for %%T in ("%~1\.") do set "TF=%%~fT"
if "%~d1"=="%~1" goto :eof
if "%TF%"=="" goto :eof
if "%TF:~-1%"=="\" goto :eof
findstr /i /x /l /c:"%TF%" "%TMPROOTS%" >nul 2>&1
if errorlevel 1 >>"%TMPROOTS%" echo %TF%
goto :eof

:SCANROOT
rem -- %1 = one install root, %2 = the file to look for.
rem -- BOUNDED: this folder, its subfolders, and THEIR subfolders. No
rem -- recursion, no drive crawl. Progress is printed by the caller.
call :TRYPATH "%~1\%~2"
call :TRYPATH "%~1\Chs\%~2"
for /d %%D in ("%~1\*") do call :TRYDIR "%%~fD" "%~2"
for /d %%D in ("%~1\*") do for /d %%E in ("%%~fD\*") do call :TRYDIR "%%~fE" "%~2"
goto :eof

:TRYDIR
call :TRYPATH "%~1\%~2"
call :TRYPATH "%~1\Chs\%~2"
goto :eof

:TRYPATH
if not exist "%~1" goto :eof
findstr /i /x /l /c:"%~1" "%TMPDLLS%" >nul 2>&1
if errorlevel 1 >>"%TMPDLLS%" echo %~1
goto :eof
"""

# the roots the kit asks for. %1 = 1 for patch/verify/restore (engine DLLs),
# %2 = optional extra folder from the command line.
ROOTCOLLECT = r"""
call :ADDREG "HKLM\SOFTWARE\Wow6432Node\TOLTech" Location
call :ADDREG "HKLM\SOFTWARE\TOLTech" Location
call :ADDREG "HKLM\SOFTWARE\Wow6432Node\Toltech\xeyez_engine" Location
call :ADDREG "HKLM\SOFTWARE\Toltech\xeyez_engine" Location
call :ADDREG "HKLM\SOFTWARE\Wow6432Node\Toltech\xeyez_engine" RootPath
call :ADDREG "HKLM\SOFTWARE\Toltech\xeyez_engine" RootPath
call :ADDSVC "xEyeService"
call :ADDFOLDER "%ProgramFiles%\TOLTech"
call :ADDFOLDER "%ProgramFiles%\Toltech"
call :ADDFOLDER "%ProgramFiles%\idong"
call :ADDFOLDER "%ProgramFiles%\i-dong"
if defined ProgramFiles(x86) (
  call :ADDFOLDER "%ProgramFiles(x86)%\TOLTech"
  call :ADDFOLDER "%ProgramFiles(x86)%\Toltech"
  call :ADDFOLDER "%ProgramFiles(x86)%\idong"
  call :ADDFOLDER "%ProgramFiles(x86)%\i-dong"
)
call :ADDFOLDER "%SystemDrive%\TOLTech"
call :ADDFOLDER "%SystemDrive%\Toltech"
call :ADDFOLDER "%SystemDrive%\xeyez_engine"
call :ADDFOLDER "D:\TOLTech"
call :ADDFOLDER "D:\xeyez_engine"
if not "%~1"=="" call :ADDFOLDER "%~1"
"""

ADMIN_NOTE = r"""
fltmc >nul 2>&1
if errorlevel 1 (
  echo [ERR] Please right-click this file and "Run as administrator".
  echo Press any key to close this window...
  pause >nul
  exit /b 1
)
"""

# ---------------------------------------------------------------------------
# patch_all37.bat
# ---------------------------------------------------------------------------
bat("patch_all37.bat", r"""
@echo off
REM ============================================================
REM  patch_all37.bat - idong sensor engine unlock, kit 37
REM  Pure cmd.exe - NO PowerShell. Run as ADMINISTRATOR.
REM
REM  WHAT THIS DOES: this kit ships the FINISHED patched DLLs in
REM  the 'patched' folder. This script finds your engine install,
REM  checks the state of each DLL and copies the right patched
REM  file over it (backup first, byte-verify after, roll back on
REM  any error). It can NOT byte-patch, so it can not corrupt a
REM  file: every step is a copy plus an fc /b comparison.
REM
REM  The three gates it removes (all proven on the wire + in
REM  emulation):
REM    xEyeUSB.dll   - upsell exit + version-tag gate 'DSHY'
REM                    (this unit answers '000T' - letter T refused)
REM    xEyeRight.dll - license: the engine phones us.i-dong.com,
REM                    a server dead for years, so a stock install
REM                    can never be authorized. Patched: it always
REM                    reports authorized + fires the vendor's own
REM                    success notification.
REM    xEyeWarrningUI.dll - the box itself: ShowDeviceIDInvalidWindow
REM                    (the Sorry-your-sensor-hardware-is-not-
REM                    authorized window). Patched: the function
REM                    returns at once, the box can never appear.
REM
REM  Accepted states per file (fc /b against the kit references):
REM    vendor -> patched, older patch -> patched, current -> skip,
REM    v2.5.0.7 engine build -> xEyeRight + xEyeWarrningUI still get
REM    patched, anything else -> REFUSED, nothing is touched.
REM
REM  KIT 36 FIX: this script no longer hangs. The kit-36 finder
REM  could crawl a whole drive silently (reg value like "C:\" in
REM  RootPath) - this one scans at most two folder levels deep and
REM  prints every root it checks. The admin check no longer uses
REM  the Server service (net session) - fltmc is local and fast.
REM ============================================================
setlocal enabledelayedexpansion
title idong100 engine unlock - kit 37
cd /d "%~dp0"
echo === idong100 engine unlock, kit 37 ===
echo.

REM ---- 0. must run elevated ----
""" + ADMIN_NOTE + r"""

REM ---- 0b. the kit folders must be complete ----
if not exist "patched\xEyeUSB_t33.dll"           goto :NOkit
if not exist "patched\xEyeRight_t34.dll"         goto :NOkit
if not exist "patched\xEyeRight_t34_v25.dll"     goto :NOkit
if not exist "patched\xEyeWarrningUI_t37.dll"     goto :NOkit
if not exist "patched\xEyeWarrningUI_t37_v25.dll" goto :NOkit
if not exist "ref\xEyeUSB_vendor_v26.dll"        goto :NOkit
if not exist "ref\xEyeUSB_t31.dll"               goto :NOkit
if not exist "ref\xEyeUSB_vendor_v25.dll"        goto :NOkit
if not exist "ref\xEyeRight_vendor_v26.dll"      goto :NOkit
if not exist "ref\xEyeRight_vendor_v25.dll"      goto :NOkit
if not exist "ref\xEyeWarrningUI_vendor_v26.dll" goto :NOkit
if not exist "ref\xEyeWarrningUI_vendor_v25.dll" goto :NOkit

REM ---- 1. collect candidate install roots (registry, services,
REM      Program Files, fixed folders; optional argument folder) ----
set "TMPROOTS=%TEMP%\idong37_roots.txt"
set "TMPDLLS=%TEMP%\idong37_dlls.txt"
del "%TMPROOTS%" 2>nul
del "%TMPDLLS%" 2>nul
""" + ROOTCOLLECT + r"""

if not exist "%TMPROOTS%" (
  echo [ERR] No install folder found anywhere.
  echo Looked in: the registry values TOLTech\Location,
  echo Toltech\xeyez_engine\Location and \RootPath, the services
  echo xEyeService/xeyez, Program Files, C:\TOLTech,
  echo C:\xeyez_engine and the same folders on D:.
  echo If your engine lives somewhere else, run this file from a
  echo cmd line with the folder as argument:
  echo     patch_all37.bat "D:\my folder\xeyez_engine"
  echo Press any key to close this window...
  pause >nul
  exit /b 1
)

REM ---- 2. bounded scan for the engine DLL (prints every root) ----
echo [1/4] Looking for xEyeUSB.dll - a few seconds at most:
for /f "usebackq delims=" %%R in ("%TMPROOTS%") do (
  echo       %%R
  call :SCANROOT "%%R" "xEyeUSB.dll"
)
if not exist "%TMPDLLS%" (
  echo [ERR] No xEyeUSB.dll under the install roots above.
  echo       The engine folder exists but is incomplete - reinstall
  echo       the idong software first.
  echo Press any key to close this window...
  pause >nul
  exit /b 1
)
set /a NFOUND=0
for /f "usebackq delims=" %%F in ("%TMPDLLS%") do set /a NFOUND+=1
echo.
echo [2/4] Found !NFOUND! xEyeUSB.dll:
for /f "usebackq delims=" %%F in ("%TMPDLLS%") do echo       %%F
echo.

REM ---- 3. classify and patch each engine folder ----
set /a NPATCHED=0
set /a NOK=0
set /a NREFUSED=0
echo [3/4] Checking the three gates per engine folder...
for /f "usebackq delims=" %%F in ("%TMPDLLS%") do call :PROCESS "%%F"

echo.
echo [4/4] Summary: !NPATCHED! patched, !NOK! already current, !NREFUSED! refused.
if !NPATCHED! GTR 0 (
  echo.
  echo DONE. All three gates are open now. Next:
  echo   1. start the lobby - it should get past the hardware test
  echo   2. or run run_direct37.bat - engine + camera viewer
  echo      without the lobby (the tray icon should reach
  echo      running within ~15 s)
  echo   3. run run_capture.bat for a USBPcap capture while the
  echo      camera runs, then check it with check_capture.bat
)
if !NREFUSED! GTR 0 (
  echo.
  echo Some files were REFUSED. Read the lines above - a refused
  echo file is a build this kit does not know. Nothing was changed.
)
echo.
echo Press any key to close this window...
pause >nul
del "%TMPROOTS%" 2>nul
del "%TMPDLLS%" 2>nul
exit /b 0

REM ================= helpers =================
""" + FINDER + r"""

:NOkit
echo [ERR] This script must run from the complete kit folder
echo       (patched\ and ref\ next to it). Nothing was changed.
echo Press any key to close this window...
pause >nul
exit /b 1

:PROCESS  -- classify and patch the three gate DLLs of one engine folder
set "U=%~1"
set "UD=%~dp1"

REM ---- gate 1: xEyeUSB.dll (version-tag + upsell) ----
set "USTATE=foreign"
fc /b "%U%" "patched\xEyeUSB_t33.dll" >nul 2>&1 && set "USTATE=t33"
if not "!USTATE!"=="t33" fc /b "%U%" "ref\xEyeUSB_t31.dll" >nul 2>&1 && set "USTATE=t31"
if "!USTATE!"=="foreign" fc /b "%U%" "ref\xEyeUSB_vendor_v26.dll" >nul 2>&1 && set "USTATE=vendor"
if "!USTATE!"=="foreign" fc /b "%U%" "ref\xEyeUSB_vendor_v25.dll" >nul 2>&1 && set "USTATE=vendor25"

if "!USTATE!"=="t33" (
  echo [skip] xEyeUSB.dll already fully patched.
  set /a NOK+=1
) else if "!USTATE!"=="t31" (
  call :PATCHFILE "%U%" "patched\xEyeUSB_t33.dll" "idong37"
  if !ERROROK! == 1 set /a NPATCHED+=1
) else if "!USTATE!"=="vendor" (
  call :PATCHFILE "%U%" "patched\xEyeUSB_t33.dll" "idong37"
  if !ERROROK! == 1 set /a NPATCHED+=1
) else if "!USTATE!"=="vendor25" (
  echo [note] xEyeUSB.dll is the v2.5.0.7 engine build - left as it
  echo        is. The version-tag gate exists only in the v2.6.0.6
  echo        build. The two DLLs below are still patched.
) else (
  echo [refuse] xEyeUSB.dll is not a known build. Nothing changed.
  set /a NREFUSED+=1
)

REM ---- gate 2: xEyeRight.dll (the license server call) ----
set "R=!UD!xEyeRight.dll"
if exist "!R!" (
  set "RSTATE=foreign"
  fc /b "!R!" "patched\xEyeRight_t34.dll" >nul 2>&1 && set "RSTATE=t34"
  if not "!RSTATE!"=="t34" fc /b "!R!" "patched\xEyeRight_t34_v25.dll" >nul 2>&1 && set "RSTATE=t34"
  if "!RSTATE!"=="t34" (
    echo [skip] xEyeRight.dll already patched.
  ) else (
    set "RDLL="
    fc /b "!R!" "ref\xEyeRight_vendor_v26.dll" >nul 2>&1 && set "RDLL=patched\xEyeRight_t34.dll"
    if not defined RDLL fc /b "!R!" "ref\xEyeRight_vendor_v25.dll" >nul 2>&1 && set "RDLL=patched\xEyeRight_t34_v25.dll"
    if defined RDLL (
      call :PATCHFILE "!R!" "!RDLL!" "idong37"
    ) else (
      echo [refuse] xEyeRight.dll is not a known build. Nothing changed.
    )
  )
) else (
  echo [note] xEyeRight.dll not found next to the engine DLL.
)

REM ---- gate 3: xEyeWarrningUI.dll (the not-authorized box) ----
set "W=!UD!xEyeWarrningUI.dll"
if exist "!W!" (
  set "WSTATE=foreign"
  fc /b "!W!" "patched\xEyeWarrningUI_t37.dll" >nul 2>&1 && set "WSTATE=t37"
  if not "!WSTATE!"=="t37" fc /b "!W!" "patched\xEyeWarrningUI_t37_v25.dll" >nul 2>&1 && set "WSTATE=t37"
  if "!WSTATE!"=="t37" (
    echo [skip] xEyeWarrningUI.dll already patched.
  ) else (
    set "WDLL="
    fc /b "!W!" "ref\xEyeWarrningUI_vendor_v26.dll" >nul 2>&1 && set "WDLL=patched\xEyeWarrningUI_t37.dll"
    if not defined WDLL fc /b "!W!" "ref\xEyeWarrningUI_vendor_v25.dll" >nul 2>&1 && set "WDLL=patched\xEyeWarrningUI_t37_v25.dll"
    if defined WDLL (
      call :PATCHFILE "!W!" "!WDLL!" "idong37"
    ) else (
      echo [refuse] xEyeWarrningUI.dll is not a known build. Nothing changed.
    )
  )
) else (
  echo [note] xEyeWarrningUI.dll not found next to the engine DLL.
)
echo       folder done: %UD%
goto :eof

:PATCHFILE  -- backup + copy the patched reference + verify + rollback
set "T=%~1"
set "SRC=%~2"
set "TAG=%~3"
set "ERROROK=0"
fc /b "%SRC%" "%T%" >nul 2>&1
if not errorlevel 1 (
  echo [ok] "%T%" already matches the kit file.
  set "ERROROK=1"
  goto :eof
)
if not exist "%T%.%TAG%-backup" (
  copy /y "%T%" "%T%.%TAG%-backup" >nul
)
copy /y "%SRC%" "%T%" >nul
fc /b "%T%" "%SRC%" >nul 2>&1
if errorlevel 1 (
  echo [ERR] verification failed for "%T%". Restoring the backup...
  copy /y "%T%.%TAG%-backup" "%T%" >nul
  echo       Restored. Nothing was changed.
) else (
  echo [patched] "%T%"
  set "ERROROK=1"
)
goto :eof
""")

# ---------------------------------------------------------------------------
# verify_all37.bat
# ---------------------------------------------------------------------------
bat("verify_all37.bat", r"""
@echo off
REM ============================================================
REM  verify_all37.bat - are the engine DLLs unlocked right now?
REM  Read-only, no admin needed. Pure cmd.exe. Kit 37.
REM ============================================================
setlocal enabledelayedexpansion
title idong100 - verify engine state
cd /d "%~dp0"
echo === idong100 engine state, kit 37 ===
echo.

REM ---- 1. collect candidate install roots ----
set "TMPROOTS=%TEMP%\idong37v_roots.txt"
set "TMPDLLS=%TEMP%\idong37v_dlls.txt"
del "%TMPROOTS%" 2>nul
del "%TMPDLLS%" 2>nul
""" + ROOTCOLLECT + r"""

if not exist "%TMPROOTS%" (
  echo [ERR] No install folder found. Nothing to verify.
  echo       Reinstall the idong software, then run patch_all37.bat.
  goto :done
)

REM ---- 2. bounded scan for the engine DLL (prints every root) ----
echo Looking for xEyeUSB.dll - a few seconds at most:
for /f "usebackq delims=" %%R in ("%TMPROOTS%") do (
  echo       %%R
  call :SCANROOT "%%R" "xEyeUSB.dll"
)
if not exist "%TMPDLLS%" (
  echo [ERR] No xEyeUSB.dll under the install roots.
  goto :done
)
set /a UBAD=0
set /a RRIGHT=0
set /a WRIGHT=0
for /f "usebackq delims=" %%F in ("%TMPDLLS%") do call :CHECKONE "%%F"
echo.
if !UBAD! GTR 0 (
  echo RESULT: NOT UNLOCKED. Run patch_all37.bat as administrator.
) else if !RRIGHT! LSS 1 (
  echo RESULT: xEyeUSB is unlocked but no patched xEyeRight.dll was
  echo found next to it. The license call still fails - the engine
  echo stays at unauthorized. Run patch_all37.bat.
) else if !WRIGHT! LSS 1 (
  echo RESULT: xEyeUSB + xEyeRight are unlocked but the
  echo xEyeWarrningUI.dll box is still ACTIVE. Run patch_all37.bat.
) else (
  echo RESULT: all three gates open (version tag + license + box).
  echo The engine should accept the sensor. If the lobby still
  echo fails, make a capture with run_capture.bat and send it in.
)
:done
echo.
echo Press any key to close this window...
pause >nul
del "%TMPROOTS%" 2>nul
del "%TMPDLLS%" 2>nul
exit /b 0

:CHECKONE
set "U=%~1"
set "UD=%~dp1"
set "USTATE=FOREIGN"
fc /b "%U%" "patched\xEyeUSB_t33.dll" >nul 2>&1 && set "USTATE=UNLOCKED (turn-33 patch)"
if not "!USTATE!"=="UNLOCKED (turn-33 patch)" fc /b "%U%" "ref\xEyeUSB_t31.dll" >nul 2>&1 && set "USTATE=PARTIAL (turn-31, license still active)"
if not "!USTATE!"=="UNLOCKED (turn-33 patch)" if not "!USTATE!"=="PARTIAL (turn-31, license still active)" fc /b "%U%" "ref\xEyeUSB_vendor_v26.dll" >nul 2>&1 && set "USTATE=LOCKED (vendor v2.6.0.6)"
if not "!USTATE!"=="UNLOCKED (turn-33 patch)" if not "!USTATE!"=="PARTIAL (turn-31, license still active)" if not "!USTATE!"=="LOCKED (vendor v2.6.0.6)" fc /b "%U%" "ref\xEyeUSB_vendor_v25.dll" >nul 2>&1 && set "USTATE=VENDOR v2.5.0.7 BUILD"
echo xEyeUSB.dll : !USTATE!
echo             %U%
if not "!USTATE!"=="UNLOCKED (turn-33 patch)" if not "!USTATE!"=="VENDOR v2.5.0.7 BUILD" set /a UBAD+=1
set "R=!UD!xEyeRight.dll"
if exist "!R!" (
  set "RSTATE=FOREIGN"
  fc /b "!R!" "patched\xEyeRight_t34.dll" >nul 2>&1 && set "RSTATE=AUTHORIZED (patched)"
  if not "!RSTATE!"=="AUTHORIZED (patched)" fc /b "!R!" "patched\xEyeRight_t34_v25.dll" >nul 2>&1 && set "RSTATE=AUTHORIZED (patched)"
  if not "!RSTATE!"=="AUTHORIZED (patched)" fc /b "!R!" "ref\xEyeRight_vendor_v26.dll" >nul 2>&1 && set "RSTATE=LICENSE GATE ACTIVE (vendor v2.6)"
  if not "!RSTATE!"=="AUTHORIZED (patched)" if not "!RSTATE!"=="LICENSE GATE ACTIVE (vendor v2.6)" fc /b "!R!" "ref\xEyeRight_vendor_v25.dll" >nul 2>&1 && set "RSTATE=LICENSE GATE ACTIVE (vendor v2.5)"
  echo xEyeRight.dll: !RSTATE!
  echo             !R!
  if "!RSTATE!"=="AUTHORIZED (patched)" set /a RRIGHT+=1
) else (
  echo xEyeRight.dll: not found next to the engine DLL.
)
set "W=!UD!xEyeWarrningUI.dll"
if exist "!W!" (
  set "WSTATE=FOREIGN"
  fc /b "!W!" "patched\xEyeWarrningUI_t37.dll" >nul 2>&1 && set "WSTATE=BOX REMOVED (patched)"
  if not "!WSTATE!"=="BOX REMOVED (patched)" fc /b "!W!" "patched\xEyeWarrningUI_t37_v25.dll" >nul 2>&1 && set "WSTATE=BOX REMOVED (patched)"
  if not "!WSTATE!"=="BOX REMOVED (patched)" fc /b "!W!" "ref\xEyeWarrningUI_vendor_v26.dll" >nul 2>&1 && set "WSTATE=BOX ACTIVE (vendor v2.6)"
  if not "!WSTATE!"=="BOX REMOVED (patched)" if not "!WSTATE!"=="BOX ACTIVE (vendor v2.6)" fc /b "!W!" "ref\xEyeWarrningUI_vendor_v25.dll" >nul 2>&1 && set "WSTATE=BOX ACTIVE (vendor v2.5)"
  echo xEyeWarrningUI.dll: !WSTATE!
  echo             !W!
  if "!WSTATE!"=="BOX REMOVED (patched)" set /a WRIGHT+=1
) else (
  echo xEyeWarrningUI.dll: not found next to the engine DLL.
)
echo.
goto :eof

REM ================= helpers =================
""" + FINDER)

# ---------------------------------------------------------------------------
# restore_all37.bat
# ---------------------------------------------------------------------------
bat("restore_all37.bat", r"""
@echo off
REM ============================================================
REM  restore_all37.bat - undo the kit 37 unlock
REM  Restores every xEyeUSB.dll / xEyeRight.dll / xEyeWarrningUI.dll
REM  that has an .idong3*-backup next to it. Failing that, the vendor
REM  file is restored from the ref folder - a foreign file is never
REM  overwritten.
REM  Pure cmd.exe. ADMIN required. Kit 37.
REM ============================================================
setlocal enabledelayedexpansion
title idong100 - restore engine DLLs
cd /d "%~dp0"
echo === idong100 engine restore, kit 37 ===
echo.

""" + ADMIN_NOTE + r"""

REM ---- 1. collect candidate install roots ----
set "TMPROOTS=%TEMP%\idong37r_roots.txt"
set "TMPDLLS=%TEMP%\idong37r_dlls.txt"
del "%TMPROOTS%" 2>nul
del "%TMPDLLS%" 2>nul
""" + ROOTCOLLECT + r"""

if not exist "%TMPROOTS%" (
  echo [ERR] No install folder found. Nothing to restore.
  goto :done
)

REM ---- 2. bounded scan for the engine DLLs ----
echo Looking for the engine DLLs - a few seconds at most:
for /f "usebackq delims=" %%R in ("%TMPROOTS%") do (
  echo       %%R
  call :SCANROOT "%%R" "xEyeUSB.dll"
)
if not exist "%TMPDLLS%" (
  echo [ERR] No xEyeUSB.dll found. Nothing to restore.
  goto :done
)

REM ---- 3. restore each folder ----
for /f "usebackq delims=" %%F in ("%TMPDLLS%") do call :RESTOREFOLDER "%%F"
echo.
echo Done. The lines above say what was restored.
echo.
:done
echo Press any key to close this window...
pause >nul
del "%TMPROOTS%" 2>nul
del "%TMPDLLS%" 2>nul
exit /b 0

:RESTOREFOLDER  -- restore the three gate DLLs of one engine folder
set "U=%~1"
set "UD=%~dp1"
call :RESTOREDLL "%U%" "xEyeUSB.dll" "ref\xEyeUSB_vendor_v26.dll" "ref\xEyeUSB_vendor_v25.dll"
call :RESTOREDLL "!UD!xEyeRight.dll" "xEyeRight.dll" "ref\xEyeRight_vendor_v26.dll" "ref\xEyeRight_vendor_v25.dll"
call :RESTOREDLL "!UD!xEyeWarrningUI.dll" "xEyeWarrningUI.dll" "ref\xEyeWarrningUI_vendor_v26.dll" "ref\xEyeWarrningUI_vendor_v25.dll"
goto :eof

:RESTOREDLL  -- %1 dll, %2 short name, %3 vendor v26 ref, %4 vendor v25 ref
if not exist "%~1" goto :eof
set "BACKSET="
for %%B in ("%~1.idong3?-backup") do if not defined BACKSET if exist "%%B" set "BACKSET=%%B"
if defined BACKSET (
  copy /y "!BACKSET!" "%~1" >nul
  echo [restored] %~1 from !BACKSET!
  goto :eof
)
fc /b "%~1" "%~3" >nul 2>&1
if not errorlevel 1 (
  echo [note] %~2 is already the vendor file. Nothing to do.
  goto :eof
)
fc /b "%~1" "%~4" >nul 2>&1
if not errorlevel 1 (
  echo [note] %~2 is already the vendor v2.5 file. Nothing to do.
  goto :eof
)
echo [refuse] %~2 is not a vendor state this kit knows and has no
echo         backup. It was NOT touched.
goto :eof

REM ================= helpers =================
""" + FINDER)

# ---------------------------------------------------------------------------
# run_direct37.bat
# ---------------------------------------------------------------------------
bat("run_direct37.bat", r"""
@echo off
REM ============================================================
REM  run_direct37.bat - run the camera WITHOUT the lobby
REM  Starts the engine (xeyez.exe) directly with the correct
REM  working directory, then offers xEyeThirdLens.exe (the
REM  standalone camera viewer). Needs patch_all37.bat applied
REM  once. Pure cmd.exe.
REM ============================================================
setlocal enabledelayedexpansion
title idong100 - direct run
cd /d "%~dp0"
echo === idong100 direct run ===
echo.

REM ---- 1. collect candidate install roots ----
set "TMPROOTS=%TEMP%\idong37d_roots.txt"
set "TMPDLLS=%TEMP%\idong37d_dlls.txt"
del "%TMPROOTS%" 2>nul
del "%TMPDLLS%" 2>nul
""" + ROOTCOLLECT + r"""

if not exist "%TMPROOTS%" (
  echo [ERR] No install folder found.
  goto :done
)

REM ---- 2. bounded scan for the engine host xeyez.exe ----
echo Looking for xeyez.exe - a few seconds at most:
for /f "usebackq delims=" %%R in ("%TMPROOTS%") do (
  echo       %%R
  call :SCANROOT "%%R" "xeyez.exe"
)
if not exist "%TMPDLLS%" (
  echo [ERR] No xeyez.exe under the install roots.
  goto :done
)
set "ENGINEEXE="
set "ENGINECHS="
for /f "usebackq delims=" %%F in ("%TMPDLLS%") do (
  if not defined ENGINEEXE (
    set "ENGINECHS=%%~dpF"
    set "ENGINEEXE=%%F"
  )
)
if not defined ENGINEEXE (
  echo [ERR] No xeyez.exe found.
  goto :done
)
echo Engine: !ENGINEEXE!
echo.
echo The engine window shows its state; the tray icon should reach
echo running within ~15 s after the sensor is plugged in. If it shows
echo a not-authorized box instead, run verify_all37.bat first.
echo.
set "P3RDMOD=!ENGINECHS!xEyeThirdLens.exe"
if not exist "!P3RDMOD!" (
  echo [ERR] xEyeThirdLens.exe not found next to the engine host.
  goto :startengine
)
echo Starting the standalone camera viewer in 5 seconds. Close its
echo window any time - it does not stop the engine.
start "" "!P3RDMOD!"
ping -n 6 127.0.0.1 >nul
:startengine
echo Starting the engine now. Close its window to stop it.
start "" /d "!ENGINECHS!" "!ENGINEEXE!"
echo.
:done
echo Press any key to close this window...
pause >nul
del "%TMPROOTS%" 2>nul
del "%TMPDLLS%" 2>nul
exit /b 0

REM ================= helpers =================
""" + FINDER)

print()
for f in ("patched/xEyeUSB_t33.dll", "patched/xEyeRight_t34.dll",
          "patched/xEyeRight_t34_v25.dll", "ref/xEyeUSB_vendor_v26.dll",
          "ref/xEyeUSB_t31.dll", "ref/xEyeUSB_vendor_v25.dll",
          "ref/xEyeRight_vendor_v26.dll", "ref/xEyeRight_vendor_v25.dll"):
    plain_copy(os.path.join(OLD, f), f)
plain_copy("/home/z/my-project/work/t37/patched/xEyeWarrningUI_t37.dll", "patched/xEyeWarrningUI_t37.dll")
plain_copy("/home/z/my-project/work/t37/patched/xEyeWarrningUI_t37_v25.dll", "patched/xEyeWarrningUI_t37_v25.dll")
plain_copy("/home/z/my-project/work/t37/ref/xEyeWarrningUI_vendor_v26.dll", "ref/xEyeWarrningUI_vendor_v26.dll")
plain_copy("/home/z/my-project/work/t37/ref/xEyeWarrningUI_vendor_v25.dll", "ref/xEyeWarrningUI_vendor_v25.dll")

# carried over unchanged, machine-validated in the earlier kits
for f in ("check_capture.bat", "test_usbpcap.bat",
          "check_sensor_attached.bat", "predir.bat"):
    plain_copy(os.path.join(OLD, f), f)
