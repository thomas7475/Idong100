@echo off
setlocal
set "SELF=%~dp0"
set "ARG=%~1"
setlocal enabledelayedexpansion
title idong100 - USBPcap self test
REM ============================================================
REM  test_usbpcap.bat - is USBPcap installed and does it record?
REM  Turn 24. Pure cmd.exe, no PowerShell.
REM
REM  1. finds USBPcapCMD.exe and prints its version
REM  2. checks the USBPcap driver service and the hub class filter
REM  3. lists the USB root hubs USBPcap can listen to
REM  4. shows whether the sensor is seen as 0471:2600 or 0471:2500
REM  5. records 20 seconds from EVERY root hub while you move the
REM     mouse or plug the sensor in
REM  6. reads the files back with the check_capture.bat scanner
REM
REM  Usage:  test_usbpcap.bat            20 seconds
REM          test_usbpcap.bat 60         60 seconds
REM          a key press ends the test early
REM
REM  Best: right-click this file, Run as administrator. Without that
REM  Windows shows a UAC prompt for every root hub - click Yes.
REM  Nothing is sent to the sensor by this test.
REM  The window stays open at every exit (user rule, turn 19).
REM ============================================================

set "CAPROOT=%SystemDrive%\idong_capture"
set "PREDIRRC=0"
set "TMPD=%CAPROOT%\work"
set "OUTD=%CAPROOT%\usbpcap_test"
set "SECS=20"
set "RC=1"
set "EXE="
set "VER="
set "SVC="
set "ADMIN="
set "FILES="
set "TO=0"
set "SCANRC=9"
set "STARTED=0"
set "SZ=0"
set /a NHUB=0
set /a GOODN=0

echo === idong100 - USBPcap self test ===
echo.
echo This takes about one minute.
echo.

rem --- prepare the capture folder (turn 25): C:\idong_capture, or its repair,
rem --- or the USERPROFILE fallback. CAPROOT and PREDIRRC come from predir.bat.
call "%~dp0predir.bat"
set "TMPD=!CAPROOT!\work"
set "OUTD=!CAPROOT!\usbpcap_test"
if "!PREDIRRC!"=="2" goto :no_workdir
if "!PREDIRRC!"=="1" goto :fallback_note
echo Test folder: C:\idong_capture
goto :folder_ready
:fallback_note
echo NOTE: C:\idong_capture is not writable in this window. This test writes
echo       into the fallback folder "!CAPROOT!" instead. Right-click this
echo       script, Run as administrator, once: that repairs C:\idong_capture
echo       for your user account.
:folder_ready
echo.

if not "%ARG%"=="" (
  for /f "delims=0123456789" %%c in ("!ARG!") do goto :bad_arg
  if not "!ARG:~3!"=="" goto :bad_arg
  set /a "SECS=1!ARG! %% 10000"
  if !SECS! LSS 5 set "SECS=5"
  if !SECS! GTR 900 set "SECS=900"
)
if not exist "!TMPD!" md "!TMPD!"
if not exist "!TMPD!\nul" goto :no_workdir

REM ---------- 1. the program ----------
echo [1/6] USBPcapCMD.exe
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

REM ---------- 2. the driver ----------
echo [2/6] USBPcap driver
set "SVC="
for /f "tokens=3 delims=: " %%a in ('sc query USBPcap 2^>nul ^| findstr /i "STATE"') do set "SVC=%%a"
if not defined SVC echo       driver service USBPcap: not installed
if defined SVC echo       driver service USBPcap: !SVC!
reg query "HKLM\SYSTEM\CurrentControlSet\Control\Class\{36fc9e60-c465-11cf-8056-444553540000}" /v UpperFilters 2>nul | findstr /i "USBPcap" >nul
if errorlevel 1 goto :no_filter
if not defined SVC goto :no_filter
echo       USB hub class filter: present

REM ---------- 3. the root hubs ----------
echo [3/6] USB root hubs
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

REM ---------- 4. the sensor ----------
echo [4/6] Sensor as this Windows sees it now
set "S2600="
set "S2500="
wmic path Win32_PnPEntity get DeviceID /value 2>nul > "!TMPD!\dev.txt"
findstr /i "VID_0471" "!TMPD!\dev.txt"
if errorlevel 1 echo       no device with VID 0471 - the sensor is unplugged.
findstr /i "VID_0471&PID_2600" "!TMPD!\dev.txt" >nul && set "S2600=1"
findstr /i "VID_0471&PID_2500" "!TMPD!\dev.txt" >nul && set "S2500=1"
if defined S2600 goto :cam2600
if defined S2500 goto :cam2500
goto :elev

:cam2600
echo       0471:2600 = boot loader. The sensor waits for the vendor program
echo       to download its program. Normally it changes to 0471:2500 within seconds.
goto :elev

:cam2500
echo       0471:2500 = the sensor runs its program. The download is over.
echo       To capture a download, unplug the sensor and plug it in while capturing.

:elev
echo [5/6] Recording !SECS! seconds from every root hub
net session >nul 2>&1
if not errorlevel 1 set "ADMIN=1"
if defined ADMIN goto :is_admin
fsutil dirty query %SystemDrive% >nul 2>&1
if not errorlevel 1 set "ADMIN=1"
:is_admin
if not defined ADMIN goto :not_admin
echo       this window runs as administrator: USBPcap starts without asking.
goto :startcap

:not_admin
echo       this window does NOT run as administrator.
echo       Windows shows a UAC prompt for EVERY root hub, !NHUB! prompt(s) in total.
echo       Click Yes on each one. Next time: right-click this file, Run as administrator.

:startcap
if not exist "!OUTD!" md "!OUTD!"
del /q "!OUTD!\*.pcap" 2>nul
REM No --inject-descriptors here: with it an idle root hub also gets a file larger
REM than the 24 byte header, and the size would no longer prove that packets arrived.
REM USBPcap runs in its own minimized window per root hub, so that it does not
REM read the keys pressed in this window.
taskkill /f /im USBPcapCMD.exe >nul 2>&1
set /a K=0
:startloop
set /a K+=1
for %%k in (!K!) do set "DEV=!HUBDEV_%%k!"
start "idong USBPcap test !K!" /min cmd /c start "" /wait "!EXE!" -d !DEV! -A -o "!OUTD!\test!K!.pcap"
if !K! LSS !NHUB! goto :startloop
set /a W=0
:waitfiles
ping -n 2 127.0.0.1 >nul
set /a STARTED=0
for %%f in ("!OUTD!\*.pcap") do set /a STARTED+=1
if !STARTED! GEQ !NHUB! goto :filesok
set /a W+=1
if !W! LSS 30 goto :waitfiles
:filesok
if !STARTED! EQU 0 goto :not_started
echo.
echo   Recording now. MOVE THE MOUSE inside this Windows.
echo   If the sensor is plugged in: unplug it and plug it in again.
echo   If Windows asks you with a UAC prompt: click Yes.
echo   A key press ends the test early.
echo.
set "TO=0"
timeout /t 1 >nul 2>&1
if not errorlevel 1 set "TO=1"
set /a T=0
:waitloop
if !T! GEQ !SECS! goto :waitend
if "!TO!"=="1" goto :waitloop_to
ping -n 2 127.0.0.1 >nul
goto :waitloop_next
:waitloop_to
timeout /t 1 >nul 2>&1
if not errorlevel 1 goto :waitloop_next
goto :waitend
:waitloop_next
<nul set /p "=."
set /a T+=1
goto :waitloop
:waitend
echo.
echo   Stopping USBPcap.
taskkill /f /im USBPcapCMD.exe >nul 2>&1
ping -n 3 127.0.0.1 >nul

REM ---------- 6. the result ----------
echo.
echo [6/6] Result
set /a K=0
:evalloop
set /a K+=1
for %%k in (!K!) do set "DEV=!HUBNAME_%%k!"
set "F=!OUTD!\test!K!.pcap"
if not exist "!F!" goto :eval_missing
set "SZ=0"
for %%f in ("!F!") do set "SZ=%%~zf"
set "FILES=!FILES! "!F!""
echo   !DEV!   !SZ! bytes
if !SZ! LEQ 24 goto :evalnext
set /a GOODN+=1
goto :evalnext
:eval_missing
echo   !DEV!   no file was created
:evalnext
if !K! LSS !NHUB! goto :evalloop
echo   A file of 0 bytes was never opened. Exactly 24 bytes is an empty capture.
echo.
set "SCANRC=9"
if not defined FILES goto :verdict
if not exist "!SELF!check_capture.bat" goto :verdict
echo Packet scan of the test files:
cscript //nologo //E:JScript "!SELF!check_capture.bat" --info !FILES!
set "SCANRC=!errorlevel!"
echo.

:verdict
echo ================= RESULT =================
set "OK=0"
if "!SCANRC!"=="0" set "OK=1"
if "!SCANRC!"=="9" if !GOODN! GTR 0 set "OK=1"
if "!OK!"=="1" goto :ver_ok
if !STARTED! EQU 0 goto :ver_none
echo USBPcap opens the root hubs but recorded NO USB packet.
echo Nothing happened on the USB bus during the test, or USBPcap is
echo not loaded in this Windows. Move the mouse, plug a USB device in
echo and run this test again. If it stays like this: restart the VM.
set "RC=1"
goto :done

:ver_none
echo USBPcapCMD did not create any capture file.
if defined ADMIN goto :ver_none2
echo You did not click Yes on the UAC prompts.
echo Right-click this file and use Run as administrator.
goto :ver_none3
:ver_none2
echo Run USBPcapCMD by hand to read its error message. In a command prompt:
echo   "!EXE!" -d !HUBDEV_1! -A -o "!OUTD!\manual.pcap"
taskkill /f /im USBPcapCMD.exe >nul 2>&1
:ver_none3
if not defined SVC goto :ver_none4
if /i "!SVC!"=="RUNNING" goto :done
echo The USBPcap driver service is !SVC!, it must be RUNNING. Restart the VM.
goto :done
:ver_none4
echo The USBPcap driver service is not installed: install USBPcap again.
goto :done

:ver_ok
echo USBPcap WORKS in this Windows.
echo !GOODN! of !NHUB! root hub files contain USB packets.
echo Test files are stored in:  "!OUTD!"
echo You can delete them. A real capture goes to "!CAPROOT!\runN".
set "RC=0"
goto :done

:no_workdir
echo [ERR] cannot create the folder "!CAPROOT!". Create it by hand, or give
echo       your user account the right to write there, then run this test again.
goto :done

:no_exe
echo       not found. USBPcap is not installed in this Windows.
echo       Run software\USBPcapSetup-1.5.4.0.exe inside this Windows,
echo       then RESTART the VM, then run this test again.
goto :done

:no_version
echo       found, but it does not answer. Its message:
type "!TMPD!\version.txt"
goto :done

:no_filter
if defined SVC goto :no_filter2
echo       The USBPcap driver is not installed. Run software\USBPcapSetup-1.5.4.0.exe
echo       inside this Windows, then RESTART the VM, then run this test again.
goto :done
:no_filter2
if /i not "!SVC!"=="RUNNING" goto :no_filter3
echo       The USB hub class filter for USBPcap is missing.
echo       USBPcap is only half installed: install it again and RESTART the VM.
goto :done
:no_filter3
echo       The USBPcap driver service is !SVC!. Restart the VM and run this test again.
goto :done

:no_hub
echo       none. USBPcap finds no USB root hub to listen to.
echo       Its message:
type "!TMPD!\ifaces.txt"
echo       USBPcap is installed but its driver is not active in this Windows.
echo       RESTART the VM after installing USBPcap, then run this test again.
if /i not "!SVC!"=="RUNNING" if defined SVC echo       Driver service state now: !SVC!
goto :done

:not_started
echo.
echo [ERR] USBPcapCMD created no file.
taskkill /f /im USBPcapCMD.exe >nul 2>&1
if defined ADMIN goto :ver_none2
echo       You did not click Yes on the UAC prompts.
echo       Right-click this file and use Run as administrator.
goto :done

:bad_arg
echo [ERR] unknown argument. The only argument is a number of seconds, for example:  test_usbpcap.bat 60
goto :done

:done
echo.
echo Exit code !RC!   0 = USBPcap works   1 = problem, read the lines above
echo.
pause
endlocal & exit /b %RC%
