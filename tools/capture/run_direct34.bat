@echo off
REM ============================================================
REM  run_direct34.bat - run the CAMERA software DIRECTLY
REM  Pure cmd.exe. Turn 34. No administrator needed.
REM
REM  You asked: "is there a way to run software for the camera
REM  direct?" - YES, three of them, and this script does them:
REM
REM    A. xeyez.exe      - the ENGINE itself. This is the program
REM                         that flashes and runs the sensor and
REM                         serves the camera data. It does not
REM                         need the lobby, the service or any
REM                         game. This is what we start here.
REM    B. xEyeThirdLens.exe - the vendor's own standalone camera
REM                         viewer (middle camera calibration +
REM                         live view). It connects to the running
REM                         engine and shows the camera picture.
REM                         Started on request below.
REM    C. run_capture.bat - USBPcap capture while it runs, so we
REM                         finally get the video stream on the
REM                         wire (the missing protocol piece).
REM
REM  The engine window is hidden (a tray icon) - look at the
REM  TRAY at the bottom right:
REM    connecting  = "idong传感器连接中"  (still trying)
REM    running     = "idong传感器运行中"  (SUCCESS - camera live)
REM ============================================================
setlocal enabledelayedexpansion
title idong100 - direct camera run
cd /d "%~dp0"
echo === idong100 direct camera run, turn 34 ===
echo.

REM ---- find the engine folder (Chs folder with xeyez.exe) ----
set "ENG="
for /f "tokens=2*" %%A in ('reg query "HKLM\SOFTWARE\Toltech\xeyez_engine" /v Location 2^>nul ^| findstr /i "Location"') do if exist "%%B\" set "ENG=%%B"
if not defined ENG for /f "tokens=2*" %%A in ('reg query "HKLM\SOFTWARE\Toltech\xeyez_engine" /v RootPath 2^>nul ^| findstr /i "RootPath"') do if exist "%%B\" set "ENG=%%B"
if not defined ENG for /f "tokens=2*" %%A in ('reg query "HKLM\SOFTWARE\TOLTech" /v Location 2^>nul ^| findstr /i "Location"') do if exist "%%B\" set "ENG=%%B"
if defined ENG if not exist "!ENG!\xeyez.exe" (
  for /f "delims=" %%F in ('dir /s /b "!ENG!\xeyez.exe" 2^>nul') do if not defined XEZ set "XEZ=%%F"
  if defined XEZ for %%D in ("!XEZ!") do set "ENG=%%~dpD"
)
if not defined ENG (
  for %%P in ("%ProgramFiles%\TOLTech" "%SystemDrive%\TOLTech" "D:\TOLTech") do (
    if not defined ENG if exist %%P (
      for /f "delims=" %%F in ('dir /s /b "%%~P\xeyez.exe" 2^>nul') do if not defined ENG for %%D in ("%%F") do set "ENG=%%~dpD"
    )
  )
)
if defined ENG if exist "!ENG!\Chs\xeyez.exe" set "ENG=!ENG!\Chs"
if not defined ENG (
  echo [ERR] could not find the engine ^(xeyez.exe^).
  echo Reinstall the idong software first, or edit this file and
  echo set ENG to the folder that contains xeyez.exe.
  goto :done
)
if not exist "!ENG!\xeyez.exe" (
  echo [ERR] no xeyez.exe in "!ENG!".
  goto :done
)
echo Engine folder:
echo   !ENG!
echo.

REM ---- warn about the DLL state without blocking ----
if exist "!ENG!\xEyeUSB.dll" (
  fc /b "!ENG!\xEyeUSB.dll" "xEyeUSB\xEyeUSB_t33.dll" >nul 2>&1
  if errorlevel 1 (
    echo [warning] xEyeUSB.dll in that folder is NOT in the turn-33
    echo           unlocked state. If the tray never reaches
    echo           "running", run patch_engine34.bat as administrator.
  ) else (
    echo [ok] xEyeUSB.dll is patched ^(turn 33^).
  )
)
if exist "!ENG!\xEyeRight.dll" (
  fc /b "!ENG!\xEyeRight.dll" "xEyeRight\xEyeRight_t34.dll" >nul 2>&1
  if not errorlevel 1 (
    echo [ok] xEyeRight.dll is patched ^(turn 34, license bypassed^).
  ) else (
    fc /b "!ENG!\xEyeRight.dll" "xEyeRight\xEyeRight_t34_v25.dll" >nul 2>&1
    if not errorlevel 1 (
      echo [ok] xEyeRight.dll is patched ^(turn 34, license bypassed^).
    ) else (
      echo [warning] xEyeRight.dll still carries the vendor license
      echo           gate. Run patch_engine34.bat as administrator.
    )
  )
)
echo.

REM ---- is the engine already running? ----
tasklist 2>nul | findstr /i "xeyez.exe" >nul
if not errorlevel 1 (
  echo [info] xeyez.exe is already running - not starting a second one.
  goto :viewer
)

echo Starting the engine ^(xeyez.exe^) directly...
start "idong engine" /D"!ENG!" "!ENG!\xeyez.exe"
echo   started. WAIT about 15 seconds, then look at the tray icon:
echo   - "connecting" = it keeps trying ^(see the warnings above^)
echo   - "running"    = SUCCESS - the camera is live
echo.

:viewer
if not exist "!ENG!\xEyeThirdLens.exe" (
  echo [info] this install has no xEyeThirdLens.exe viewer.
  goto :capture
)
set "ANS=y"
set /p "ANS=Start the vendor camera viewer xEyeThirdLens.exe now? [Y/n] "
if /i "!ANS!"=="n" goto :capture
start "idong third lens" /D"!ENG!" "!ENG!\xEyeThirdLens.exe"
echo   started. Its window shows the middle camera view/calibration.
echo   If it prints "Initialize pipe failed", the engine is not
echo   running yet - wait and start it again.
echo.

:capture
echo LAST STEP - capture the session while the camera runs:
echo   right-click run_capture.bat, "Run as administrator",
echo   let it record 60+ seconds while the viewer shows a picture,
echo   then check the result with check_wire34.bat.
echo That capture should finally contain the video stream.
:done
echo.
echo Press any key to close this window...
pause >nul
exit /b 0
