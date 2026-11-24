@echo off
REM ============================================================
REM idong100 P2.3 kit - MANUAL DRIVER INSTALL FALLBACK (cmd only)
REM Use this if the vendor installer still fails after the
REM preinstall stub, or if you want to install the drivers by
REM hand. Run as Administrator.
REM ============================================================
setlocal enableextensions
set BASE=%~dp0
set DRV=%ProgramFiles(x86)%\TOLTech\xeyez_engine\DriverInstall

echo.
echo === idong100 manual driver install (fallback) ===
echo Source tree: %DRV%
echo.

if not exist "%DRV%" (
  echo [ERROR] Driver tree not found: %DRV%
  echo Run the patched vendor installer first so the files are on disk,
  echo then re-run this script.
  pause
  exit /b 1
)

REM 1) stage the xeyez driver INF + SYS + co-installer into a temp dir
set STAGE=%TEMP%\idong_drv
if exist "%STAGE%" rmdir /s /q "%STAGE%"
mkdir "%STAGE" >nul 2>&1
mkdir "%STAGE%\x64" >nul 2>&1

if exist "%DRV%\Win7\xeyezDriver64\xeyez.inf" (
  copy /y "%DRV%\Win7\xeyezDriver64\*.*" "%STAGE%\x64\" >nul
  echo Staged Win7 xeyezDriver64 into %%TEMP%%\idong_drv\x64
) else (
  echo [WARN] Win7\xeyezDriver64 not found - trying Win7\xeyezDriver
)

REM 2) pnputil add+install for the staged INF (Win10/11 has pnputil /add-driver)
where pnputil >nul 2>&1
if errorlevel 1 (
  echo [ERROR] pnputil not found ^(very old Windows^). Use Device Manager
  echo           manually: Update Driver -^> Browse -^> "%STAGE%\x64"
  pause
  exit /b 1
)

echo.
echo === Installing xeyez.inf via pnputil ===
pnputil /add-driver "%STAGE%\x64\xeyez.inf" /install /subdirs
echo pnputil exit code: %errorlevel%

REM 3) virtual HID drivers (vMouse / vKeyboard) if present
if exist "%DRV%\Win7\virtualDriver64\vMouse.inf" (
  echo.
  echo === Installing virtual HID drivers ===
  pnputil /add-driver "%DRV%\Win7\virtualDriver64\vMouse.inf" /install
  pnputil /add-driver "%DRV%\Win7\virtualDriver64\vKeyboard.inf" /install
  echo virtual driver pnputil exit: %errorlevel%
)

echo.
echo === Done. If pnputil reported 0 or 259 (BASCII_SUCCESS/NO_MORE_ITEMS), ===
echo === reboot once, then plug the sensor in and check Device Manager.    ===
echo.
echo If Windows refused the driver with a signature error, reboot with the
echo one-boot 'Disable driver signature enforcement' option:
echo   Settings -^> Recovery -^> Advanced startup -^> Troubleshoot -^>
echo   Advanced options -^> Startup Settings -^> Restart -^> press 7
echo then re-run this script.
pause
endlocal
