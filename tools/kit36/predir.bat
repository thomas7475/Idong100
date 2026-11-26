@echo off
REM ============================================================
REM  predir.bat - find a writable folder for the captures.
REM  Called by run_capture.bat, test_usbpcap.bat and
REM  check_capture.bat before they write anything. Pure cmd.exe,
REM  no PowerShell.
REM
REM  Hand run (double-click or from a cmd window) works too:
REM  it then prints a self-test, the rights of the folders it
REM  tried, and it WAITS for a key before it closes.
REM
REM  Why this exists (turn 25): C:\idong_capture was created by
REM  an elevated window, and on Windows 7 such a folder is by
REM  default not writable for a normal window. Turn 26 added:
REM  more fallback folders, the probe errors are collected and
REM  shown, and every script tells you how to copy its window
REM  text so it can be sent as a screenshot-free report.
REM
REM  Turn 27 removed the folder existence checks completely:
REM  "if exist folder\nul" reported folders as missing on this
REM  Windows (FAT32/exFAT drive, seen in VMs) even right after
REM  md had created them, so all four candidates were rejected
REM  with "cannot create the folder" although the folders were
REM  there. A real file write is now the only judge: md, then
REM  copy nul into the folder. Only the write decides.
REM
REM  Folders tried, in this order:
REM    1. SystemDrive\idong_capture    C:\idong_capture
REM    2. USERPROFILE\idong_capture     C:\Users\name\idong_capture
REM    3. LOCALAPPDATA\idong_capture    C:\Users\name\AppData\Local\idong_capture
REM    4. TEMP\idong_capture            C:\Users\name\AppData\...\Temp\idong_capture
REM  3 and 4 are per-user folders that are writable by design,
REM  they exist on every Windows 7 profile, so one of the four
REM  always works even when C:\ itself rejects folder creation.
REM
REM  The first folder that accepts a probe file becomes CAPROOT.
REM  When the first candidate fails and this window is elevated,
REM  C:\idong_capture gets an icacls repair attempt first.
REM
REM  A folder "exists" for this script only when a file can be
REM  written into it. There is no existence check on purpose.
REM
REM  Sets for the caller:
REM    CAPROOT   folder to use
REM    PREDIRRC  0 = C:\idong_capture, 1 = a fallback folder is
REM              used, 2 = nothing was writable (all 4 failed)
REM    PRENOTE   one-line reason when PREDIRRC is 1 or 2
REM  The caller must print CAPROOT and PRENOTE.
REM ============================================================
setlocal enabledelayedexpansion
set "CALLED=0"
if "%~1"=="/called" set "CALLED=1"

set "C1=%SystemDrive%\idong_capture"
set "C2=%USERPROFILE%\idong_capture"
set "C3=%LOCALAPPDATA%\idong_capture"
set "C4=%TEMP%\idong_capture"
set "CAPROOT="
set "PREDIRRC=2"
set "PRENOTE="
set "PROBES="

if not "%CALLED%"=="1" (
  echo === idong100 - predir self test ===
  echo.
  echo This checks where the captures can be stored.
  echo Nothing is sent to the sensor.
  echo.
)

rem ---------- candidate 1: C:\idong_capture ----------
set "W=%C1%"
call :try
if defined CAPROOT goto :picked

rem ---------- repair attempt when elevated ----------
fsutil dirty query %SystemDrive% >nul 2>&1
if errorlevel 1 goto :no_elev
echo       C:\idong_capture is not writable, this window IS elevated:
echo       repairing its rights with icacls, one attempt...
icacls "%C1%" /grant "*S-1-5-32-545:(OI)(CI)M" >nul 2>&1
if errorlevel 1 icacls "%C1%" /grant "Users:(OI)(CI)M" >nul 2>&1
set "W=%C1%"
call :try
if defined CAPROOT goto :picked
echo       icacls repair failed too. The current rights of the folder are:
icacls "%C1%"
echo.
:no_elev

rem ---------- candidates 2, 3, 4 ----------
set "W=%C2%"
call :try
if defined CAPROOT goto :picked
set "W=%C3%"
call :try
if defined CAPROOT goto :picked
set "W=%C4%"
call :try
if defined CAPROOT goto :picked

rem ---------- nothing was writable ----------
set "PREDIRRC=2"
set "PRENOTE=none of the four candidate folders accepted a file"
if "%CALLED%"=="1" goto :hand_end
echo [ERR] no writable folder found. Tried, with the errors:
echo.
echo   1. %C1%   - %PROBES_C1%
echo   2. %C2%   - %PROBES_C2%
echo   3. %C3%   - %PROBES_C3%
echo   4. %C4%   - %PROBES_C4%
echo.
echo   The errors from the probe writes, in the order tried:
echo   %PROBES%
echo.
echo   Folders 3 and 4 always exist and are writable on Windows 7.
echo   When all four fail, something is broken in the user profile
echo   or the disk is full or corrupt. To use another drive instead:
echo       run_capture.bat 120 D:\caps
echo   To repair C:\idong_capture by hand, in an elevated cmd:
echo       icacls C:\idong_capture /grant Users:(OI)(CI)M
echo.
goto :wait_end

:picked
if "%W%"=="%C1%" set "PREDIRRC=0"
if not "%W%"=="%C1%" set "PREDIRRC=1"
if "%PREDIRRC%"=="1" set "PRENOTE=fallback folder used: !W!"
if "%CALLED%"=="1" goto :hand_end
if "%PREDIRRC%"=="0" echo       OK: C:\idong_capture is writable.
if "%PREDIRRC%"=="1" echo       OK: fallback folder !W! is writable.
goto :wait_end

rem ============ subroutines, called with W set ============
:try
md "%W%" >nul 2>&1
copy /y nul "%W%\predir_probe.tmp" >nul 2>&1
if errorlevel 1 (
  call :note "cannot write a file into it"
  goto :eof
)
del /q "%W%\predir_probe.tmp" >nul 2>&1
set "CAPROOT=%W%"
goto :eof

:note
set "PROBES=!PROBES! [!W!: %~1]"
if not defined PROBES_C1 if "!W!"=="%C1%" set "PROBES_C1=%~1"
if not defined PROBES_C2 if "!W!"=="%C2%" set "PROBES_C2=%~1"
if not defined PROBES_C3 if "!W!"=="%C3%" set "PROBES_C3=%~1"
if not defined PROBES_C4 if "!W!"=="%C4%" set "PROBES_C4=%~1"
goto :eof

:wait_end
echo.
echo Result: CAPROOT is "!CAPROOT!"   PREDIRRC is !PREDIRRC!.
if "!PREDIRRC!"=="2" echo Result: no writable folder at all.
echo.
echo How to copy this text out of the window:
echo   1. right-click the title bar of this window
echo   2. Bearbeiten - Markieren  (English: Edit - Select All)
echo   3. press Enter  (that copies the whole window text)
echo   4. paste it into a text file or an e-mail
echo.
pause
exit /b 0

:hand_end
endlocal & set "CAPROOT=%CAPROOT%" & set "PREDIRRC=%PREDIRRC%" & set "PRENOTE=%PRENOTE%"
exit /b 0
