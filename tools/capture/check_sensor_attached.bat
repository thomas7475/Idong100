@echo off
REM ============================================================
REM idong100 P2 kit - sensor pre-flight check (cmd only)
REM Confirms the idong sensor is attached and enumerating before
REM running the vendor installer, which gives up after ~10 s if
REM none of the Philips VID_0471 device IDs are present and then
REM shows 安装驱动程序失败！. Run as Administrator.
REM ============================================================
setlocal enableextensions

echo.
echo === idong100 sensor pre-flight check ===
echo.

set FOUND=0
for /f "usebackq tokens=*" %%i in (`wmic path Win32_PnPEntity where "DeviceID like 'USB\\%%VID_0471%%'" get DeviceID 2^>nul ^| findstr /i "VID_0471"`) do (
  echo   FOUND: %%i
  set FOUND=1
)

if "%FOUND%"=="1" (
  echo.
  echo A Philips VID_0471 USB device is enumerated - the installer's
  echo device scan should find it and the driver stage can proceed.
) else (
  echo.
  echo NO Philips VID_0471 USB device found. The vendor installer will
  echo wait about 10 seconds and then fail with 安装驱动程序失败！
  echo Plug the idong sensor into a rear USB port ^(avoid hubs^), wait
  echo for the enumeration sound, then run this check again.
)
echo.
pause
endlocal
