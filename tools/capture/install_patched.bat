@echo off
REM ============================================================
REM  install_patched.bat - run the GATE-FREE installers (turn 19)
REM  Pure cmd.exe - NO PowerShell. Run as administrator.
REM
REM  Why: xeyez_setup.exe / xeyez_alone.dat carry an OS-version
REM  gate in VersionCheck.dll that whitelists ONLY Windows XP
REM  5.1, Vista 6.0 and Win7 6.1. On Windows 8/10/11 it shows
REM  'idong sensor software is incompatible with your OS' and
REM  exits. Compatibility mode CANNOT fix it - the gate reads
REM  _winmajor/_winminor from MSVCR80.dll, not GetVersionEx
REM  directly, so the compat layer's lie never reaches it.
REM
REM  The patched copies in this kit have the gate neutralised
REM  (4 conditional jumps rewritten; every path returns 0 -
REM  the same value the gate returns on Windows XP).
REM
REM  This script:
REM    1. backs up the original xeyez_setup.exe / xeyez_alone.dat
REM    2. swaps in the patched copies
REM    3. runs the patched launcher
REM ============================================================
setlocal enabledelayedexpansion
title idong100 patched install

cd /d "%~dp0"
echo === idong100 patched installer swap - turn 19 ===
echo.

if not exist "xeyez_setup_patched.exe" (
  if exist "software\xeyez_setup_patched.exe" (
    copy /y "software\xeyez_setup_patched.exe" . >nul
  ) else (
    echo [ERR] xeyez_setup_patched.exe not found in this folder or software\
    echo Press any key to close this window...
    pause >nul
    exit /b 1
  )
)
if not exist "xeyez_alone_patched.dat" (
  if exist "software\xeyez_alone_patched.dat" (
    copy /y "software\xeyez_alone_patched.dat" . >nul
  ) else (
    echo [ERR] xeyez_alone_patched.dat not found in this folder or software\
    echo Press any key to close this window...
    pause >nul
    exit /b 1
  )
)

echo [1/3] backing up the original installers...
if exist "xeyez_setup.exe" (
  if not exist "xeyez_setup.original.exe" (
    copy /y "xeyez_setup.exe" "xeyez_setup.original.exe" >nul
    echo       xeyez_setup.exe backed up as xeyez_setup.original.exe
  ) else (
    echo       backup already exists - keeping it
  )
) else (
  echo       no original xeyez_setup.exe here - nothing to back up
)
if exist "xeyez_alone.dat" (
  if not exist "xeyez_alone.original.dat" (
    copy /y "xeyez_alone.dat" "xeyez_alone.original.dat" >nul
    echo       xeyez_alone.dat backed up as xeyez_alone.original.dat
  ) else (
    echo       backup already exists - keeping it
  )
) else (
  echo       no original xeyez_alone.dat here - nothing to back up
)
echo.

echo [2/3] swapping in the patched installers...
copy /y "xeyez_setup_patched.exe" "xeyez_setup.exe" >nul
copy /y "xeyez_alone_patched.dat" "xeyez_alone.dat" >nul
echo       done - xeyez_setup.exe and xeyez_alone.dat are now gate-free
echo.

echo [3/3] launch the patched installer...
echo.
echo  NOTES before you continue:
echo   - import platform_mark.reg FIRST if you have not already
echo     - reg import platform_mark.reg
echo   - the patched installer no longer shows the OS dialog
echo   - if Windows flags the unsigned patched exe, click
echo     'More info - Run anyway' - the vendor's own signature
echo     no longer matches the modified file
echo   - if the 2008-era driver is refused on Win10/11 x64, use
echo     the one-boot 'Disable driver signature enforcement'
echo     route - README_CAPTURE.md section 4
echo.
echo Press any key to start xeyez_setup.exe...
pause >nul
start "" xeyez_setup.exe
echo.
echo  Installer launched. Complete the vendor install flow
echo  - driver, service, tray - then continue with
echo  README_CAPTURE.md section 3 - attach the sensor.
echo.
echo Press any key to close this window...
pause >nul
endlocal
exit /b 0
