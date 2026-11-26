@echo off
REM ============================================================
REM  verify_engine34.bat - are the engine DLLs patched right now?
REM  Read-only. Pure cmd.exe. Turn 34.
REM  Shows every found xEyeUSB.dll / xEyeRight.dll and its state.
REM ============================================================
setlocal enabledelayedexpansion
title idong100 - verify engine state
cd /d "%~dp0"
echo === idong100 engine state, turn 34 ===
echo.

set "TMPROOTS=%TEMP%\idong34v_roots.txt"
set "TMPDLLS=%TEMP%\idong34v_dlls.txt"
del "%TMPROOTS%" 2>nul
del "%TMPDLLS%" 2>nul
call :ADDREG "HKLM\SOFTWARE\Wow6432Node\TOLTech" Location
call :ADDREG "HKLM\SOFTWARE\TOLTech" Location
call :ADDREG "HKLM\SOFTWARE\Wow6432Node\Toltech\xeyez_engine" Location
call :ADDREG "HKLM\SOFTWARE\Toltech\xeyez_engine" Location
call :ADDREG "HKLM\SOFTWARE\Wow6432Node\Toltech\xeyez_engine" RootPath
call :ADDREG "HKLM\SOFTWARE\Toltech\xeyez_engine" RootPath
call :ADDSVC "xEyeService"
call :ADDSVC "xeyez"
call :ADDFOLDER "%ProgramFiles%\TOLTech"
if defined ProgramFiles(x86) call :ADDFOLDER "%ProgramFiles(x86)%\TOLTech"
call :ADDFOLDER "%SystemDrive%\TOLTech"
call :ADDFOLDER "%SystemDrive%\xeyez_engine"
call :ADDFOLDER "D:\TOLTech"
call :ADDFOLDER "D:\xeyez_engine"
if not "%~1"=="" call :ADDFOLDER "%~1"

if not exist "%TMPROOTS%" (
  echo [ERR] No install folder found. Nothing to verify.
  echo       Reinstall the idong software, then run patch_engine34.bat.
  goto :done
)
for /f "usebackq delims=" %%R in ("%TMPROOTS%") do (
  for /f "delims=" %%F in ('dir /s /b "%%R\xEyeUSB.dll" 2^>nul') do call :ONCE "%%F"
)
if not exist "%TMPDLLS%" (
  echo [ERR] No xEyeUSB.dll under the install roots.
  goto :done
)
set /a UBAD=0
for /f "usebackq delims=" %%F in ("%TMPDLLS%") do call :CHECKONE "%%F"
echo.
if !UBAD! GTR 0 (
  echo RESULT: NOT UNLOCKED. Run patch_engine34.bat as administrator.
) else (
  echo RESULT: all found engine DLLs are unlocked. The engine should
  echo accept the sensor. If the app still says "Connecting to idong
  echo sensor", make a capture and send it in.
)
:done
echo.
echo Press any key to close this window...
pause >nul
del "%TMPROOTS%" 2>nul
del "%TMPDLLS%" 2>nul
exit /b 0

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
echo xEyeUSB.dll : %USTATE%
echo             %U%
if not "!USTATE!"=="TURN-33 (unlocked)" set /a UBAD+=1
set "R=!UD!xEyeRight.dll"
if exist "!R!" (
  set "RSTATE=FOREIGN"
  fc /b "!R!" "xEyeRight\xEyeRight_t34.dll" >nul 2>&1 && set "RSTATE=TURN-34 (authorized)"
  if not "!RSTATE!"=="TURN-34 (authorized)" fc /b "!R!" "xEyeRight\xEyeRight_t34_v25.dll" >nul 2>&1 && set "RSTATE=TURN-34 (authorized)"
  fc /b "!R!" "xEyeRight\xEyeRight_vendor.dll" >nul 2>&1 && set "RSTATE=VENDOR (license gate active)"
  if not "!RSTATE!"=="TURN-34 (authorized)" if "!RSTATE!"=="FOREIGN" fc /b "!R!" "xEyeRight\xEyeRight_vendor_v25.dll" >nul 2>&1 && set "RSTATE=VENDOR (license gate active)"
  echo xEyeRight.dll: !RSTATE!
  echo             !R!
) else (
  echo xEyeRight.dll: not found next to the engine DLL.
)
echo.
goto :eof
