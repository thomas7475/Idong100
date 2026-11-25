@echo off
REM ============================================================
REM  predir.bat - make sure the capture folder is writable.
REM  Called by run_capture.bat, test_usbpcap.bat and
REM  check_capture.bat. Not meant to be run by hand, but it
REM  is safe to run: it creates a folder and deletes a probe
REM  file, nothing else. Pure cmd.exe, no PowerShell.
REM  No pause on purpose: the calling script stays open.
REM
REM  Why this exists (turn 25): C:\idong_capture was created
REM  by an elevated window. On Windows 7 the default rights of
REM  a folder created by an elevated process let a normal user
REM  read it but not write into it. A non-elevated window then
REM  cannot create run folders or capture files in it.
REM
REM  What it does, in order:
REM    1. creates C:\idong_capture when it does not exist
REM    2. writes and deletes a probe file in it
REM    3. when that fails and the window is elevated: makes the
REM       folder writable with icacls
REM    4. when that fails too: uses %USERPROFILE%\idong_capture
REM  Sets for the caller:
REM    CAPROOT   the folder to use
REM    PREDIRRC  0 = C:\idong_capture is writable, 1 = the
REM              USERPROFILE fallback folder is used, 2 = no
REM              writable folder at all
REM  The caller must show CAPROOT and, for 1 and 2, the
REM  explanation of predir.bat.
REM ============================================================
setlocal enabledelayedexpansion
set "PREF=%SystemDrive%\idong_capture"
set "CAPROOT=%PREF%"
set "PREDIRRC=0"
set "ADMIN="

if not exist "%PREF%" md "%PREF%" >nul 2>&1
if not exist "%PREF%\nul" goto :broken
copy /y nul "%PREF%\__probe.tmp" >nul 2>&1
if errorlevel 1 goto :not_writable
del /q "%PREF%\__probe.tmp" >nul 2>&1
goto :out

:broken
set "PREDIRRC=2"
goto :out

:not_writable
net session >nul 2>&1
if not errorlevel 1 set "ADMIN=1"
if defined ADMIN goto :repair
fsutil dirty query %SystemDrive% >nul 2>&1
if not errorlevel 1 set "ADMIN=1"
if not defined ADMIN goto :fallback

:repair
icacls "%PREF%" /grant "*S-1-5-32-545:(OI)(CI)M" >nul 2>&1
if errorlevel 1 icacls "%PREF%" /grant "Users:(OI)(CI)M" >nul 2>&1
copy /y nul "%PREF%\__probe.tmp" >nul 2>&1
if errorlevel 1 goto :fallback
del /q "%PREF%\__probe.tmp" >nul 2>&1
goto :out

:fallback
set "CAPROOT=%USERPROFILE%\idong_capture"
if not exist "!CAPROOT!" md "!CAPROOT!" >nul 2>&1
copy /y nul "!CAPROOT!\__probe.tmp" >nul 2>&1
if errorlevel 1 goto :no_folder
del /q "!CAPROOT!\__probe.tmp" >nul 2>&1
set "PREDIRRC=1"
goto :out

:no_folder
set "CAPROOT=%PREF%"
set "PREDIRRC=2"

:out
endlocal & set "CAPROOT=%CAPROOT%" & set "PREDIRRC=%PREDIRRC%"
exit /b 0
