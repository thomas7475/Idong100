@echo off
setlocal enableextensions
setlocal enabledelayedexpansion
REM ============================================================
REM  idong100 P2.3 kit - MANUAL DRIVER INSTALL FALLBACK
REM  Pure cmd.exe, no PowerShell. Run as Administrator.
REM  Turn 24: Windows 7 syntax added. pnputil on Windows 7 does not
REM  know /add-driver, there it is:  pnputil -i -a xeyez.inf
REM  Use this only if the vendor installer did not install the
REM  driver and the sensor shows a yellow mark in Device Manager.
REM  Normally the vendor installer installs this driver by itself.
REM  The window stays open at every exit (user rule, turn 19).
REM ============================================================
set "PF86=%ProgramFiles(x86)%"
if not defined PF86 set "PF86=%ProgramFiles%"
set "DRV=!PF86!\TOLTech\xeyez_engine\DriverInstall"
set "STAGE=%SystemDrive%\idong_capture\work\drv"
set "NEWPNP=0"
set "VMAJ=6"
for /f "tokens=4 delims=[. " %%a in ('ver') do set "VMAJ=%%a"
if !VMAJ! GEQ 10 set "NEWPNP=1"

echo.
echo === idong100 manual driver install, fallback ===
if !NEWPNP! EQU 0 echo pnputil: Windows 7 syntax, this Windows reports major version !VMAJ!
if !NEWPNP! EQU 1 echo pnputil: Windows 10 syntax, this Windows reports major version !VMAJ!
if not exist "!DRV!" goto :no_tree
echo Source tree: !DRV!
echo.

REM 1) stage the xeyez driver files for this Windows into a work folder
set "SRCD=!DRV!\Win7\xeyezDriver64"
set "VDIR=!DRV!\Win7\virtualDriver64"
if /i "%PROCESSOR_ARCHITECTURE%"=="x86" set "SRCD=!DRV!\Win7\xeyezDriver"
if /i "%PROCESSOR_ARCHITECTURE%"=="x86" set "VDIR=!DRV!\Win7\virtualDriver"
if not exist "!SRCD!\xeyez.inf" set "SRCD=!DRV!\Win7\xeyezDriver"
if not exist "!SRCD!\xeyez.inf" goto :no_driver
if exist "!STAGE!" rmdir /s /q "!STAGE!"
md "!STAGE!"
copy /y "!SRCD!\*.*" "!STAGE!\" >nul
echo Staged !SRCD! into "!STAGE!"
echo.

REM 2) install the staged INF
echo === Installing xeyez.inf ===
call :pnpadd "!STAGE!\xeyez.inf"
echo pnputil exit code: !errorlevel!

REM 3) virtual HID drivers vMouse and vKeyboard, if the vendor files are there
if exist "!VDIR!\vMouse.inf" (
  echo.
  echo === Installing virtual HID drivers ===
  call :pnpadd "!VDIR!\vMouse.inf"
  call :pnpadd "!VDIR!\vKeyboard.inf"
)

echo.
echo === Done. Then restart Windows once, plug the sensor in and look at ===
echo === Device Manager. Run check_sensor_attached.bat to see the state. ===
echo.
echo If Windows refused the driver with a signature error: the vendor driver
echo is from 2008 and unsigned for 64-bit Windows 10 and 11. There use the
echo one-boot option 'Disable driver signature enforcement'. On Windows 7
echo 64-bit the driver installs normally.
echo.
goto :done

:no_tree
echo [ERROR] Driver tree not found: !DRV!
echo Run the patched vendor installer first so the files are on disk,
echo then re-run this script.
goto :done

:no_driver
echo [ERROR] No xeyez.inf under !DRV!\Win7 - nothing was installed.
goto :done

:done
pause
endlocal & exit /b 0

:pnpadd
if "!NEWPNP!"=="1" (
  pnputil /add-driver "%~1" /install
) else (
  pnputil -i -a "%~1"
)
goto :eof
