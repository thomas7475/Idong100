@echo off
REM ============================================================
REM  check_unlock34.bat - the full engine state on one screen
REM  Read-only diagnostic. Pure cmd.exe (+wmic). Turn 34.
REM
REM  Everything the engine needs to find the camera:
REM    1. the kernel driver service (xeyez.sys)
REM    2. the sensor plugged in (0471:2600 / 0471:2500)
REM    3. the registry values the engine and driver read
REM    4. the engine host service
REM    5. the engine DLLs and their patch state
REM ============================================================
setlocal enabledelayedexpansion
title idong100 - full unlock state check
cd /d "%~dp0"
echo === idong100 full state check, turn 34 ===
echo.
set "PROB=0"

echo --- 1. kernel driver service XEYEZ (xeyez.sys) ---
sc query XEYEZ 2>nul | findstr /i "STATE"
if errorlevel 1 (
  echo [!!] the XEYEZ driver service is NOT INSTALLED.
  echo      That alone makes every app unable to find the camera.
  echo      Reinstall the idong software with the sensor plugged
  echo      in, or run install_drivers_manual.bat from the kit.
  set "PROB=1"
) else (
  if exist "%SystemRoot%\System32\drivers\xeyez.sys" (
    echo       driver file present: %SystemRoot%\System32\drivers\xeyez.sys
  ) else (
    echo [!!] XEYEZ service exists but xeyez.sys is missing.
    set "PROB=1"
  )
)
echo.

echo --- 2. sensor present (USB VID 0471) ---
set "SEEN=0"
for /f "tokens=*" %%L in ('wmic path Win32_PnPEntity where "DeviceID like 'USB\\VID_0471%%'" get DeviceID /format:list 2^>nul ^| findstr /i "DeviceID"') do (
  set "SEEN=1"
  echo       %%L
)
if "!SEEN!"=="0" (
  echo [!!] no 0471 device found. The sensor is unplugged, or the
  echo      VM is not passing it through. Check the VM USB menu
  echo      ^(connect BOTH devices if Windows shows two^).
  set "PROB=1"
)
echo.

echo --- 3. registry values the engine and driver read ---
call :SHOWREG "HKLM\SOFTWARE\Wow6432Node\TOLTech" Location
call :SHOWREG "HKLM\SOFTWARE\TOLTech" Location
call :SHOWREG "HKLM\SOFTWARE\Wow6432Node\Toltech\xeyez_engine" Location
call :SHOWREG "HKLM\SOFTWARE\Toltech\xeyez_engine" Location
call :SHOWREG "HKLM\SOFTWARE\Wow6432Node\Toltech\xeyez_engine" RootPath
call :SHOWREG "HKLM\SOFTWARE\Toltech\xeyez_engine" RootPath
echo.

echo --- 4. engine host service xEyeService ---
sc query xEyeService 2>nul | findstr /i "STATE"
if errorlevel 1 echo       (not installed - not required for run_direct34.bat)
echo.

echo --- 5. engine DLL patch state ---
set "TMPROOTS=%TEMP%\idong34c_roots.txt"
set "TMPDLLS=%TEMP%\idong34c_dlls.txt"
del "%TMPROOTS%" 2>nul
del "%TMPDLLS%" 2>nul
call :ADDREG "HKLM\SOFTWARE\Wow6432Node\TOLTech" Location
call :ADDREG "HKLM\SOFTWARE\TOLTech" Location
call :ADDREG "HKLM\SOFTWARE\Wow6432Node\Toltech\xeyez_engine" Location
call :ADDREG "HKLM\SOFTWARE\Toltech\xeyez_engine" Location
call :ADDREG "HKLM\SOFTWARE\Wow6432Node\Toltech\xeyez_engine" RootPath
call :ADDREG "HKLM\SOFTWARE\Toltech\xeyez_engine" RootPath
call :ADDSVC "xEyeService"
call :ADDFOLDER "%ProgramFiles%\TOLTech"
if defined ProgramFiles(x86) call :ADDFOLDER "%ProgramFiles(x86)%\TOLTech"
call :ADDFOLDER "%SystemDrive%\TOLTech"
call :ADDFOLDER "%SystemDrive%\xeyez_engine"
call :ADDFOLDER "D:\TOLTech"
call :ADDFOLDER "D:\xeyez_engine"

set "NDLL=0"
if exist "%TMPROOTS%" (
  for /f "usebackq delims=" %%R in ("%TMPROOTS%") do (
    for /f "delims=" %%F in ('dir /s /b "%%R\xEyeUSB.dll" 2^>nul') do call :ONCE "%%F"
  )
)
set "UBAD=0"
if exist "%TMPDLLS%" (
  for /f "usebackq delims=" %%F in ("%TMPDLLS%") do set /a NDLL+=1
  for /f "usebackq delims=" %%F in ("%TMPDLLS%") do call :CHECKONE "%%F"
)
if "!NDLL!"=="0" (
  echo [!!] no xEyeUSB.dll found - the engine is not installed.
  set "PROB=1"
)
echo.

if "!PROB!"=="1" (
  echo ============================================
  echo RESULT: problems found above - fix the lines
  echo marked [!!] first, then re-run this check.
) else (
  if "!UBAD!" GTR 0 (
    echo ============================================
    echo RESULT: driver OK, sensor OK, but the engine DLLs are
    echo NOT unlocked. Run patch_engine34.bat as administrator.
  ) else (
    echo ============================================
    echo RESULT: driver OK, sensor OK, DLLs unlocked. The engine
    echo should accept the sensor. If the app still stays at
    echo "Connecting to idong sensor", run run_direct34.bat and
    echo make a capture - the wire will show where it stops.
  )
)
del "%TMPROOTS%" 2>nul
del "%TMPDLLS%" 2>nul
echo.
echo Press any key to close this window...
pause >nul
exit /b 0

REM ================= helpers =================
:SHOWREG
reg query "%~1" /v "%~2" >nul 2>&1
if errorlevel 1 (
  echo       (missing: %~1 ^| %~2)
) else (
  reg query "%~1" /v "%~2" 2>nul | findstr /i "%~2"
)
goto :eof

:ONCE
findstr /i /x /l /c:"%~1" "%TMPDLLS%" >nul 2>&1
if errorlevel 1 >>"%TMPDLLS%" echo %~1
goto :eof

:ADDFOLDER
if exist "%~1\" (
  findstr /i /x /l /c:"%~1" "%TMPROOTS%" >nul 2>&1
  if errorlevel 1 >>"%TMPROOTS%" echo %~1
)
goto :eof

:ADDREG
for /f "tokens=2*" %%A in ('reg query "%~1" /v "%~2" 2^>nul ^| findstr /i "%~2"') do (
  if exist "%%B\" (
    findstr /i /x /l /c:"%%B" "%TMPROOTS%" >nul 2>&1
    if errorlevel 1 >>"%TMPROOTS%" echo %%B
  )
)
goto :eof

:ADDSVC
for /f "tokens=2*" %%A in ('reg query "HKLM\SYSTEM\CurrentControlSet\Services\%~1" /v ImagePath 2^>nul ^| findstr /i "ImagePath"') do (
  set SVP=%%B
  set SVP=!SVP:"=!
  for %%D in ("!SVP!") do (
    if exist "%%~dpD" (
      findstr /i /x /l /c:"%%~dpD" "%TMPROOTS%" >nul 2>&1
      if errorlevel 1 >>"%TMPROOTS%" echo %%~dpD
    )
  )
)
goto :eof

:CHECKONE
set "U=%~1"
set "UD=%~dp1"
set "USTATE=FOREIGN"
fc /b "%U%" "xEyeUSB\xEyeUSB_t33.dll" >nul 2>&1 && set "USTATE=TURN-33 (unlocked)"
if not "!USTATE!"=="TURN-33 (unlocked)" fc /b "%U%" "xEyeUSB\xEyeUSB_t31.dll" >nul 2>&1 && set "USTATE=TURN-31 (partial)"
if "!USTATE!"=="FOREIGN" fc /b "%U%" "xEyeUSB\xEyeUSB_vendor.dll" >nul 2>&1 && set "USTATE=VENDOR (locked)"
if "!USTATE!"=="FOREIGN" fc /b "%U%" "xEyeUSB\xEyeUSB_vendor_v25.dll" >nul 2>&1 && set "USTATE=V2.5.0.7 BUILD (locked)"
echo xEyeUSB.dll : !USTATE!
echo             %U%
if not "!USTATE!"=="TURN-33 (unlocked)" set /a UBAD+=1
set "R=!UD!xEyeRight.dll"
if exist "!R!" (
  set "RSTATE=FOREIGN"
  fc /b "!R!" "xEyeRight\xEyeRight_t34.dll" >nul 2>&1 && set "RSTATE=TURN-34 (authorized)"
  if not "!RSTATE!"=="TURN-34 (authorized)" fc /b "!R!" "xEyeRight\xEyeRight_t34_v25.dll" >nul 2>&1 && set "RSTATE=TURN-34 (authorized)"
  if "!RSTATE!"=="FOREIGN" fc /b "!R!" "xEyeRight\xEyeRight_vendor.dll" >nul 2>&1 && set "RSTATE=VENDOR (license gate active)"
  if "!RSTATE!"=="FOREIGN" fc /b "!R!" "xEyeRight\xEyeRight_vendor_v25.dll" >nul 2>&1 && set "RSTATE=VENDOR (license gate active)"
  echo xEyeRight.dll: !RSTATE!
  echo             !R!
  if not "!RSTATE!"=="TURN-34 (authorized)" set /a UBAD+=1
) else (
  echo xEyeRight.dll: not found next to the engine DLL.
)
echo.
goto :eof
