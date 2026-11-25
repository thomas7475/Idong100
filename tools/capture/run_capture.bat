@echo off
setlocal
set "SELF=%~dp0"
set "ARG=%~1"
setlocal enabledelayedexpansion
title idong100 - capture the sensor boot
REM ============================================================
REM  run_capture.bat - record the complete boot of the idong sensor
REM  Turn 24 rewrite. Pure cmd.exe, no PowerShell.
REM
REM  The sensor powers up as USB 0471:2600, the boot loader. The vendor
REM  program then downloads the MCU program, the sensor comes back as
REM  0471:2500 and gets the DSP and FPGA upload. That download is on
REM  the wire only after a power cycle, so this script:
REM    1. starts USBPcap on EVERY USB root hub
REM    2. shows you when the sensor is seen as 2600 and as 2500
REM    3. stops when you press a key, or after 120 seconds
REM    4. scans the files with check_capture.bat
REM
REM  Usage:  run_capture.bat            stop with a key, or after 120 s
REM          run_capture.bat 300        stop with a key, or after 300 s
REM  Output folder: C:\idong_capture\runN   N grows with every run
REM  Files in it: USBPcap1.pcap, USBPcap2.pcap ... one per root hub,
REM  plus session.txt, sensor_states.txt and check_report.txt
REM  Best: right-click this file, Run as administrator.
REM  The window stays open at every exit (user rule, turn 19).
REM  Nothing in this script sends a byte to the sensor.
REM ============================================================

set "CAPROOT=%SystemDrive%\idong_capture"
set "PREDIRRC=0"
set "TMPD=%CAPROOT%\work"
set "SECS=120"
set "RC=1"
set "EXE="
set "VER="
set "ADMIN="
set "VEND=0"
set "FILES="
set "KEYSTOP=0"
set "TO=0"
set "STARTED=0"
set "RUNDIR="
set /a NHUB=0
set /a EL=0
set /a N=1

echo === idong100 - capture the sensor boot ===
echo.

rem --- prepare the capture folder (turn 25): C:\idong_capture, or its repair,
rem --- or the USERPROFILE fallback. CAPROOT and PREDIRRC come from predir.bat.
call "%~dp0predir.bat"
set "TMPD=!CAPROOT!\work"
if "!PREDIRRC!"=="2" goto :no_workdir
if "!PREDIRRC!"=="1" goto :fallback_note
echo Capture folder: C:\idong_capture
goto :folder_ready
:fallback_note
echo NOTE: C:\idong_capture is not writable in this window. Windows often makes
echo       such a folder read-only for normal windows after an elevated window
echo       created it. This run writes into the fallback folder:
echo       "!CAPROOT!"
echo       Right-click this script, Run as administrator, once: that repairs
echo       C:\idong_capture for your user account.
:folder_ready
echo.

if not "%ARG%"=="" (
  for /f "delims=0123456789" %%c in ("!ARG!") do goto :bad_arg
  if not "!ARG:~3!"=="" goto :bad_arg
  set /a "SECS=1!ARG! %% 10000"
  if !SECS! LSS 10 set "SECS=10"
  if !SECS! GTR 900 set "SECS=900"
)
if not exist "!TMPD!" md "!TMPD!"
if not exist "!TMPD!\nul" goto :no_workdir

REM ---------- 1. USBPcap ----------
echo [1/5] USBPcap
for %%d in ("%ProgramFiles%\USBPcap" "%ProgramFiles%\USBPcap1" "%ProgramFiles(x86)%\USBPcap" "%ProgramFiles(x86)%\USBPcap1" "%ProgramFiles%\Wireshark" "%ProgramFiles(x86)%\Wireshark") do (
  if not defined EXE if exist "%%~d\USBPcapCMD.exe" set "EXE=%%~d\USBPcapCMD.exe"
)
if not defined EXE (
  for /f "tokens=2*" %%a in ('reg query "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\USBPcap" /v UninstallString 2^>nul ^| findstr /i "UninstallString"') do (
    for %%p in (%%b) do if not defined EXE if exist "%%~dpUSBPcapCMD.exe" set "EXE=%%~dpUSBPcapCMD.exe"
  )
)
if not defined EXE goto :no_exe
echo       found: !EXE!
"!EXE!" --extcap-version > "!TMPD!\version.txt" 2>&1
for /f "tokens=2,3 delims={=}" %%a in ('findstr /i "version" "!TMPD!\version.txt"') do (
  if /i "%%a"=="version" set "VER=%%b"
)
if not defined VER goto :no_version
echo       version !VER!

REM ---------- 2. root hubs ----------
echo [2/5] USB root hubs
"!EXE!" --extcap-interfaces > "!TMPD!\ifaces.txt" 2>&1
set /a NHUB=0
for /f "usebackq tokens=1,3,5 delims={=} " %%a in ("!TMPD!\ifaces.txt") do (
  if /i "%%a"=="interface" (
    set /a NHUB+=1
    set "HUBDEV_!NHUB!=%%b"
    set "HUBNAME_!NHUB!=%%c"
  )
)
if !NHUB! EQU 0 goto :no_hub
set /a K=0
:hubshow
set /a K+=1
for %%k in (!K!) do echo       !HUBNAME_%%k!   listen as  !HUBDEV_%%k!
if !K! LSS !NHUB! goto :hubshow

REM ---------- 3. vendor program ----------
echo [3/5] Vendor program
set "VEND=0"
tasklist /fi "imagename eq xeyez.exe" 2>nul | findstr /i "xeyez.exe" >nul && set "VEND=1"
tasklist /fi "imagename eq xeyezMonitor.exe" 2>nul | findstr /i "xeyezMonitor.exe" >nul && set "VEND=1"
tasklist /fi "imagename eq xEyeService.exe" 2>nul | findstr /i "xEyeService.exe" >nul && set "VEND=1"
if "!VEND!"=="1" goto :vend_ok
echo       NOT running: no xeyez.exe, xeyezMonitor.exe or xEyeService.exe.
echo       Only the vendor program downloads the sensor program. Without it the
echo       sensor stays at 0471:2600 and there is no upload to record.
echo.
echo       Start the vendor program now, or start it after the capture is running.
pause
goto :elev

:vend_ok
echo       running.

:elev
echo [4/5] Start the capture
net session >nul 2>&1
if not errorlevel 1 set "ADMIN=1"
if defined ADMIN goto :is_admin
fsutil dirty query %SystemDrive% >nul 2>&1
if not errorlevel 1 set "ADMIN=1"
:is_admin
if not defined ADMIN goto :not_admin
echo       this window runs as administrator: USBPcap starts without asking.
goto :mkrun

:not_admin
echo       this window does NOT run as administrator.
echo       Windows shows a UAC prompt for EVERY root hub, !NHUB! prompt(s) in total.
echo       Click Yes on each one. Next time: right-click this file, Run as administrator.

:mkrun
set /a N=1
:mkrun2
if not exist "!CAPROOT!\run!N!" goto :mkrun3
set /a N+=1
goto :mkrun2
:mkrun3
set "RUNDIR=!CAPROOT!\run!N!"
md "!RUNDIR!"
if not exist "!RUNDIR!\nul" goto :no_rundir
> "!RUNDIR!\session.txt" echo idong100 sensor boot capture
>> "!RUNDIR!\session.txt" echo Started       : %DATE% %TIME%
>> "!RUNDIR!\session.txt" echo Computer      : %COMPUTERNAME%
>> "!RUNDIR!\session.txt" echo Windows       : %OS% %PROCESSOR_ARCHITECTURE%
>> "!RUNDIR!\session.txt" echo USBPcap       : !EXE!
>> "!RUNDIR!\session.txt" echo USBPcap ver   : !VER!
>> "!RUNDIR!\session.txt" echo Elevated      : !ADMIN!
>> "!RUNDIR!\session.txt" echo Root hubs     : !NHUB!
>> "!RUNDIR!\session.txt" echo Vendor running: !VEND!
>> "!RUNDIR!\session.txt" echo Time limit    : !SECS! s

REM USBPcap runs in its own minimized window per root hub, so that it does not
REM read the keys pressed in this window and does not mix its text into ours.
taskkill /f /im USBPcapCMD.exe >nul 2>&1
set /a K=0
:startloop
set /a K+=1
for %%k in (!K!) do set "DEV=!HUBDEV_%%k!"
start "idong capture !K!" /min cmd /c start "" /wait "!EXE!" -d !DEV! -A --inject-descriptors -o "!RUNDIR!\USBPcap!K!.pcap"
if !K! LSS !NHUB! goto :startloop

set /a W=0
:waitfiles
ping -n 2 127.0.0.1 >nul
set /a STARTED=0
for %%f in ("!RUNDIR!\*.pcap") do set /a STARTED+=1
if !STARTED! GEQ !NHUB! goto :filesok
set /a W+=1
if !W! LSS 30 goto :waitfiles
:filesok
if !STARTED! EQU 0 goto :not_started

REM ---------- 5. the session ----------
set "BASE=0"
call :elapsed
set /a BASE=EL
set "KEYSTOP=0"
set "TO=0"
timeout /t 1 >nul 2>&1
if not errorlevel 1 set "TO=1"
echo.
echo Capturing now on !NHUB! root hub(s). USBPcap runs in its own minimized
echo window on the task bar and shows nothing here.
echo.
echo   1. UNPLUG the sensor now. In a VM: disconnect it in the USB menu of
echo      the VM, or pull the cable on the host computer.
echo   2. PLUG IT IN again. In a VM: connect it to the VM again. The VM may
echo      ask twice: once for 0471:2600 and once for 0471:2500.
echo   3. Keep the vendor program running. It must be open BEFORE the sensor
echo      is plugged in again, it alone sends the program download. If the
echo      camera picture does not come back: close and restart the vendor
echo      program while this capture is still running.
echo   4. Watch the sensor lines below. They are sampled every few seconds,
echo      the scan at the end reads the files and is exact.
echo   5. When the camera works again, press a key here to stop.
echo      Without a key the capture stops after !SECS! seconds.
echo.
> "!RUNDIR!\sensor_states.txt" echo %TIME:~0,8%   capture is running
set "LASTX="
set /a NEXT=15

:watch
call :elapsed
if !EL! GEQ !SECS! goto :watch_end
set "CUR=sensor: not present"
wmic path Win32_PnPEntity get DeviceID /value 2>nul > "!TMPD!\dev.txt"
findstr /i "VID_0471&PID_2600" "!TMPD!\dev.txt" >nul && set "CUR=sensor: 0471:2600 boot loader"
findstr /i "VID_0471&PID_2500" "!TMPD!\dev.txt" >nul && set "CUR=sensor: 0471:2500 running"
if "!CUR!"=="!LASTX!" goto :watch_same
echo %TIME:~0,8%   !CUR!
>> "!RUNDIR!\sensor_states.txt" echo %TIME:~0,8%   !CUR!
set "LASTX=!CUR!"
:watch_same
if !EL! LSS !NEXT! goto :wait
echo %TIME:~0,8%   ... !EL! s of !SECS! s, press a key to stop
set /a NEXT=EL+15
:wait
if "!TO!"=="1" goto :wait_to
ping -n 2 127.0.0.1 >nul
goto :watch
:wait_to
timeout /t 1 >nul 2>&1
if not errorlevel 1 goto :watch
set "KEYSTOP=1"

:watch_end
echo.
if "!KEYSTOP!"=="1" echo You pressed a key: stopping after !EL! s.
if "!KEYSTOP!"=="0" echo Time limit reached: stopping after !EL! s.
echo Stopping USBPcap and waiting for the files.
taskkill /f /im USBPcapCMD.exe >nul 2>&1
ping -n 3 127.0.0.1 >nul
>> "!RUNDIR!\session.txt" echo Stopped        : %DATE% %TIME%
>> "!RUNDIR!\session.txt" echo Stopped after  : !EL! s
>> "!RUNDIR!\session.txt" echo Stopped by key : !KEYSTOP!

echo.
echo [5/5] Files
set /a STARTED=0
set "FILES="
for %%f in ("!RUNDIR!\*.pcap") do (
  set /a STARTED+=1
  set "FILES=!FILES! "%%~ff""
)
if defined FILES for %%f in (!FILES!) do echo   %%~nxf   %%~zf bytes
echo.
echo Captured files are stored in:
echo     "!RUNDIR!"
echo.
if !STARTED! EQU 0 goto :not_started
if not exist "!SELF!check_capture.bat" goto :no_scanner
echo Scan of the capture:
cscript //nologo //E:JScript "!SELF!check_capture.bat" --report "!RUNDIR!\check_report.txt" !FILES!
set "RC=!errorlevel!"
echo.
echo The same text is saved as "!RUNDIR!\check_report.txt".
echo.
echo ---------------- WHAT TO DO NOW ----------------
if "!RC!"=="0" goto :hint_pass
if "!RC!"=="2" goto :hint_partial
if "!RC!"=="1" goto :hint_fail
echo The scanner could not read the files. Send the whole folder "!RUNDIR!"
echo to the analysis host together with the message above.
goto :done

:hint_pass
echo PASS. Copy the whole folder "!RUNDIR!" to the analysis host.
echo Look at the Sensor line above: it must show 0471:2600 AND 0471:2500.
echo If 0471:2600 is "not seen", run this script again and unplug the sensor
echo AFTER the capture has started.
goto :done

:hint_partial
echo PARTIAL. Some sensor frames are in the files, but the boot is cut off.
echo Run this script again, start the vendor program before plugging the
echo sensor in, and wait longer before you press a key.
goto :done

:hint_fail
echo FAIL. Read the Verdict lines above. The usual reasons:
echo   - the vendor program was not running, so nothing was sent to the sensor
echo   - the capture started after the sensor was already at 0471:2500:
echo     unplug and plug it in again while this script is capturing
echo   - USBPcap did not record anything: run test_usbpcap.bat
goto :done

:elapsed
set "NOWX=%TIME%"
set "NOWX=%NOWX: =0%"
set /a EL=(1%NOWX:~0,2%-100)*3600+(1%NOWX:~3,2%-100)*60+(1%NOWX:~6,2%-100)-BASE
if !EL! LSS 0 set /a EL+=86400
goto :eof

:no_workdir
echo [ERR] no writable capture folder: neither C:\idong_capture nor the
echo       USERPROFILE fallback could be written. Run this script with
echo       right-click, Run as administrator, once: that repairs the
echo       rights of C:\idong_capture for your user account.
goto :done

:no_rundir
echo [ERR] cannot create the folder "!RUNDIR!".
goto :done

:no_exe
echo       not found. USBPcap is not installed in this Windows.
echo       Run software\USBPcapSetup-1.5.4.0.exe inside this Windows,
echo       then RESTART the VM, then run test_usbpcap.bat, then this script.
goto :done

:no_version
echo       found, but it does not answer. Its message:
type "!TMPD!\version.txt"
goto :done

:no_hub
echo       none. USBPcap finds no USB root hub to listen to:
type "!TMPD!\ifaces.txt"
echo       USBPcap is installed but its driver is not active in this Windows.
echo       RESTART the VM after installing USBPcap, then run test_usbpcap.bat.
goto :done

:not_started
echo.
echo [ERR] USBPcapCMD created no file, so nothing was captured.
taskkill /f /im USBPcapCMD.exe >nul 2>&1
echo Run test_usbpcap.bat. It finds the reason and shows how to fix it.
if defined ADMIN goto :done
echo Most likely you did not click Yes on the UAC prompts.
echo Next time: right-click this file, Run as administrator.
goto :done

:no_scanner
echo [ERR] check_capture.bat is not next to this script. Put it into the same
echo       folder and double-click it: it scans "!CAPROOT!" by itself.
set "RC=3"
goto :done

:bad_arg
echo [ERR] unknown argument. The only argument is a number of seconds, for example:  run_capture.bat 300
goto :done

:done
echo.
echo Exit code !RC!   0 = PASS   2 = PARTIAL   1 = FAIL   3 = error
echo.
pause
endlocal & exit /b %RC%
