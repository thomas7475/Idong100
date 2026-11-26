@echo off
REM ============================================================
REM  verify_all37.bat - are the engine DLLs unlocked right now?
REM  Read-only, no admin needed. Pure cmd.exe. Kit 37.
REM ============================================================
setlocal enabledelayedexpansion
title idong100 - verify engine state
cd /d "%~dp0"
echo === idong100 engine state, kit 37 ===
echo.

REM ---- 1. collect candidate install roots ----
set "TMPROOTS=%TEMP%\idong37v_roots.txt"
set "TMPDLLS=%TEMP%\idong37v_dlls.txt"
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
call :ADDFOLDER "%ProgramFiles%\Toltech"
call :ADDFOLDER "%ProgramFiles%\idong"
call :ADDFOLDER "%ProgramFiles%\i-dong"
if defined ProgramFiles(x86) (
  call :ADDFOLDER "%ProgramFiles(x86)%\TOLTech"
  call :ADDFOLDER "%ProgramFiles(x86)%\Toltech"
  call :ADDFOLDER "%ProgramFiles(x86)%\idong"
  call :ADDFOLDER "%ProgramFiles(x86)%\i-dong"
)
call :ADDFOLDER "%SystemDrive%\TOLTech"
call :ADDFOLDER "%SystemDrive%\Toltech"
call :ADDFOLDER "%SystemDrive%\xeyez_engine"
call :ADDFOLDER "D:\TOLTech"
call :ADDFOLDER "D:\xeyez_engine"
if not "%~1"=="" call :ADDFOLDER "%~1"


if not exist "%TMPROOTS%" (
  echo [ERR] No install folder found. Nothing to verify.
  echo       Reinstall the idong software, then run patch_all37.bat.
  goto :done
)

REM ---- 2. bounded scan for the engine DLL (prints every root) ----
echo Looking for xEyeUSB.dll - a few seconds at most:
for /f "usebackq delims=" %%R in ("%TMPROOTS%") do (
  echo       %%R
  call :SCANROOT "%%R" "xEyeUSB.dll"
)
if not exist "%TMPDLLS%" (
  echo [ERR] No xEyeUSB.dll under the install roots.
  goto :done
)
set /a UBAD=0
set /a RRIGHT=0
set /a WRIGHT=0
for /f "usebackq delims=" %%F in ("%TMPDLLS%") do call :CHECKONE "%%F"
echo.
if !UBAD! GTR 0 (
  echo RESULT: NOT UNLOCKED. Run patch_all37.bat as administrator.
) else if !RRIGHT! LSS 1 (
  echo RESULT: xEyeUSB is unlocked but no patched xEyeRight.dll was
  echo found next to it. The license call still fails - the engine
  echo stays at unauthorized. Run patch_all37.bat.
) else if !WRIGHT! LSS 1 (
  echo RESULT: xEyeUSB + xEyeRight are unlocked but the
  echo xEyeWarrningUI.dll box is still ACTIVE. Run patch_all37.bat.
) else (
  echo RESULT: all three gates open (version tag + license + box).
  echo The engine should accept the sensor. If the lobby still
  echo fails, make a capture with run_capture.bat and send it in.
)
:done
echo.
echo Press any key to close this window...
pause >nul
del "%TMPROOTS%" 2>nul
del "%TMPDLLS%" 2>nul
exit /b 0

:CHECKONE
set "U=%~1"
set "UD=%~dp1"
set "USTATE=FOREIGN"
fc /b "%U%" "patched\xEyeUSB_t33.dll" >nul 2>&1 && set "USTATE=UNLOCKED (turn-33 patch)"
if not "!USTATE!"=="UNLOCKED (turn-33 patch)" fc /b "%U%" "ref\xEyeUSB_t31.dll" >nul 2>&1 && set "USTATE=PARTIAL (turn-31, license still active)"
if not "!USTATE!"=="UNLOCKED (turn-33 patch)" if not "!USTATE!"=="PARTIAL (turn-31, license still active)" fc /b "%U%" "ref\xEyeUSB_vendor_v26.dll" >nul 2>&1 && set "USTATE=LOCKED (vendor v2.6.0.6)"
if not "!USTATE!"=="UNLOCKED (turn-33 patch)" if not "!USTATE!"=="PARTIAL (turn-31, license still active)" if not "!USTATE!"=="LOCKED (vendor v2.6.0.6)" fc /b "%U%" "ref\xEyeUSB_vendor_v25.dll" >nul 2>&1 && set "USTATE=VENDOR v2.5.0.7 BUILD"
echo xEyeUSB.dll : !USTATE!
echo             %U%
if not "!USTATE!"=="UNLOCKED (turn-33 patch)" if not "!USTATE!"=="VENDOR v2.5.0.7 BUILD" set /a UBAD+=1
set "R=!UD!xEyeRight.dll"
if exist "!R!" (
  set "RSTATE=FOREIGN"
  fc /b "!R!" "patched\xEyeRight_t34.dll" >nul 2>&1 && set "RSTATE=AUTHORIZED (patched)"
  if not "!RSTATE!"=="AUTHORIZED (patched)" fc /b "!R!" "patched\xEyeRight_t34_v25.dll" >nul 2>&1 && set "RSTATE=AUTHORIZED (patched)"
  if not "!RSTATE!"=="AUTHORIZED (patched)" fc /b "!R!" "ref\xEyeRight_vendor_v26.dll" >nul 2>&1 && set "RSTATE=LICENSE GATE ACTIVE (vendor v2.6)"
  if not "!RSTATE!"=="AUTHORIZED (patched)" if not "!RSTATE!"=="LICENSE GATE ACTIVE (vendor v2.6)" fc /b "!R!" "ref\xEyeRight_vendor_v25.dll" >nul 2>&1 && set "RSTATE=LICENSE GATE ACTIVE (vendor v2.5)"
  echo xEyeRight.dll: !RSTATE!
  echo             !R!
  if "!RSTATE!"=="AUTHORIZED (patched)" set /a RRIGHT+=1
) else (
  echo xEyeRight.dll: not found next to the engine DLL.
)
set "W=!UD!xEyeWarrningUI.dll"
if exist "!W!" (
  set "WSTATE=FOREIGN"
  fc /b "!W!" "patched\xEyeWarrningUI_t37.dll" >nul 2>&1 && set "WSTATE=BOX REMOVED (patched)"
  if not "!WSTATE!"=="BOX REMOVED (patched)" fc /b "!W!" "patched\xEyeWarrningUI_t37_v25.dll" >nul 2>&1 && set "WSTATE=BOX REMOVED (patched)"
  if not "!WSTATE!"=="BOX REMOVED (patched)" fc /b "!W!" "ref\xEyeWarrningUI_vendor_v26.dll" >nul 2>&1 && set "WSTATE=BOX ACTIVE (vendor v2.6)"
  if not "!WSTATE!"=="BOX REMOVED (patched)" if not "!WSTATE!"=="BOX ACTIVE (vendor v2.6)" fc /b "!W!" "ref\xEyeWarrningUI_vendor_v25.dll" >nul 2>&1 && set "WSTATE=BOX ACTIVE (vendor v2.5)"
  echo xEyeWarrningUI.dll: !WSTATE!
  echo             !W!
  if "!WSTATE!"=="BOX REMOVED (patched)" set /a WRIGHT+=1
) else (
  echo xEyeWarrningUI.dll: not found next to the engine DLL.
)
echo.
goto :eof

REM ================= helpers =================

:ONCE
findstr /i /x /l /c:"%~1" "%TMPDLLS%" >nul 2>&1
if errorlevel 1 >>"%TMPDLLS%" echo %~1
goto :eof

:ADDREG
for /f "tokens=2*" %%A in ('reg query "%~1" /v "%~2" 2^>nul ^| findstr /i "%~2"') do (
  if exist "%%B\" call :ADDROOT2 "%%B"
)
goto :eof

:ADDSVC
for /f "tokens=2*" %%A in ('reg query "HKLM\SYSTEM\CurrentControlSet\Services\%~1" /v ImagePath 2^>nul ^| findstr /i "ImagePath"') do (
  set "SVP=%%B"
  set "SVP=!SVP:"=!"
  if /i "!SVP:~0,4!"=="\??\" set "SVP=!SVP:~4!"
  if exist "!SVP!" for %%D in ("!SVP!") do if exist "%%~dpD" call :ADDROOT2 "%%~dpD"
)
goto :eof

:ADDFOLDER
if exist "%~1\" call :ADDROOT2 "%~1"
goto :eof

:ADDROOT2
if "%~1"=="" goto :eof
for %%T in ("%~1\.") do set "TF=%%~fT"
if "%~d1"=="%~1" goto :eof
if "%TF%"=="" goto :eof
if "%TF:~-1%"=="\" goto :eof
findstr /i /x /l /c:"%TF%" "%TMPROOTS%" >nul 2>&1
if errorlevel 1 >>"%TMPROOTS%" echo %TF%
goto :eof

:SCANROOT
rem -- %1 = one install root, %2 = the file to look for.
rem -- BOUNDED: this folder, its subfolders, and THEIR subfolders. No
rem -- recursion, no drive crawl. Progress is printed by the caller.
call :TRYPATH "%~1\%~2"
call :TRYPATH "%~1\Chs\%~2"
for /d %%D in ("%~1\*") do call :TRYDIR "%%~fD" "%~2"
for /d %%D in ("%~1\*") do for /d %%E in ("%%~fD\*") do call :TRYDIR "%%~fE" "%~2"
goto :eof

:TRYDIR
call :TRYPATH "%~1\%~2"
call :TRYPATH "%~1\Chs\%~2"
goto :eof

:TRYPATH
if not exist "%~1" goto :eof
findstr /i /x /l /c:"%~1" "%TMPDLLS%" >nul 2>&1
if errorlevel 1 >>"%TMPDLLS%" echo %~1
goto :eof

