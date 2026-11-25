@echo off
setlocal
title idong100 - sensor state
REM ============================================================
REM  check_sensor_attached.bat - which idong sensor state is present?
REM  Turn 24 rewrite. Pure cmd.exe, no PowerShell.
REM
REM  0471:2600  the sensor is in its boot loader. It was just plugged
REM             in and waits for the vendor program.
REM  0471:2500  the vendor program has downloaded the sensor program,
REM             the sensor is running.
REM  nothing    the sensor is not plugged in, or the VM does not pass
REM             it through to this Windows.
REM
REM  The vendor installer needs the sensor present: without any VID_0471
REM  device it stops after about 10 seconds with a message box whose
REM  Chinese title says that the driver installation failed.
REM  The window stays open at every exit (user rule, turn 19).
REM ============================================================

set "RC=1"
set "S2600="
set "S2500="

echo === idong100 - sensor state ===
echo.
echo Devices with VID 0471 that this Windows sees now:
wmic path Win32_PnPEntity get DeviceID /value 2>nul | findstr /i "VID_0471"
if errorlevel 1 echo   none
echo.
wmic path Win32_PnPEntity get DeviceID /value 2>nul | findstr /i "VID_0471&PID_2600" >nul && set "S2600=1"
wmic path Win32_PnPEntity get DeviceID /value 2>nul | findstr /i "VID_0471&PID_2500" >nul && set "S2500=1"
if defined S2600 goto :s2600
if defined S2500 goto :s2500

echo Result: NO sensor found.
echo   Plug the sensor in. In a VM, also connect it to the VM in the USB menu.
echo   Then run this check again.
goto :done

:s2600
set "RC=0"
echo Result: the sensor is present as 0471:2600, the boot loader.
echo   The vendor program can download the sensor program now. It is normal
echo   to see this state for a few seconds after plugging the sensor in.
if defined S2500 goto :both
goto :done

:s2500
set "RC=0"
echo Result: the sensor is present as 0471:2500, its program is loaded.
echo   To capture the program download: unplug the sensor and plug it in
echo   again while run_capture.bat is capturing.
goto :done

:both
echo   Both 0471:2600 and 0471:2500 are present: two sensors, or a second
echo   device with the same vendor ID.

:done
echo.
pause
endlocal & exit /b %RC%
