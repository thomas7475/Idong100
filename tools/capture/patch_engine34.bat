@echo off
REM ============================================================
REM  patch_engine34.bat - idong sensor engine unlock, turn 34
REM  Pure cmd.exe - NO PowerShell. Run as ADMINISTRATOR.
REM
REM  WHY YOUR PATCH MAY HAVE "NOT WORKED" (turn 34 findings):
REM  1. REINSTALLING THE SOFTWARE RESTORES THE UNPATCHED DLL.
REM     The install folder you sent us contains the PRISTINE
REM     vendor xEyeUSB.dll - if that is what is installed now,
REM     the turn-33 patch is gone. This script re-applies it.
REM  2. THE INSTALL FINDER WAS TOO NARROW. The turn-33 kit only
REM     looked in one registry value and C:\TOLTech. This one
REM     checks every known registry value, the service path,
REM     Program Files, TOLTech/xeyez_engine folders and both
REM     drives, and can take a folder as argument.
REM  3. THE VENDOR LICENSE CHECK IS DEAD. The engine also asks
REM     xEyeRight.dll whether the sensor is AUTHORIZED. That
REM     module phones home to us.i-dong.com - a server that has
REM     been gone for years - so a stock install can never be
REM     authorized. NEW in turn 34: this kit also patches
REM     xEyeRight.dll so it always reports "authorized" and
REM     fires the vendor's own success notification.
REM
REM  WHAT IT PATCHES (after backing up, byte-verified):
REM    xEyeUSB.dll   vendor/turn-31 -> turn-33 state
REM                  (upsell bypass + version-tag 'DSHY'->'DSTY')
REM    xEyeRight.dll vendor -> turn-34 state (license always OK)
REM
REM  States accepted / refused per file (fc /b against reference
REM  copies): vendor -> patch, older-patched -> patch, current ->
REM  nothing to do, v2.5.0.7 build -> reported, not patched,
REM  anything else -> REFUSED (foreign build protection).
REM ============================================================
setlocal enabledelayedexpansion
title idong100 engine unlock - turn 34
cd /d "%~dp0"
echo === idong100 engine unlock - turn 34 ===
echo.

REM ---- 0. must run elevated ----
net session >nul 2>&1
if errorlevel 1 (
  echo [ERR] Please right-click this file and "Run as administrator".
  echo Press any key to close this window...
  pause >nul
  exit /b 1
)

set "TMPROOTS=%TEMP%\idong34_roots.txt"
set "TMPDLLS=%TEMP%\idong34_dlls.txt"
del "%TMPROOTS%" 2>nul
del "%TMPDLLS%" 2>nul

REM ---- 1. collect candidate install roots ----
call :ADDREG "HKLM\SOFTWARE\Wow6432Node\TOLTech" Location
call :ADDREG "HKLM\SOFTWARE\TOLTech" Location
call :ADDREG "HKLM\SOFTWARE\Wow6432Node\Toltech\xeyez_engine" Location
call :ADDREG "HKLM\SOFTWARE\Toltech\xeyez_engine" Location
call :ADDREG "HKLM\SOFTWARE\Wow6432Node\Toltech\xeyez_engine" RootPath
call :ADDREG "HKLM\SOFTWARE\Toltech\xeyez_engine" RootPath
call :ADDSVC "xEyeService"
call :ADDSVC "xeyez"
call :ADDFOLDER "%ProgramFiles%\TOLTech"
call :ADDFOLDER "%ProgramFiles%\idong"
call :ADDFOLDER "%ProgramFiles%\i-dong"
if defined ProgramFiles(x86) (
  call :ADDFOLDER "%ProgramFiles(x86)%\TOLTech"
  call :ADDFOLDER "%ProgramFiles(x86)%\idong"
  call :ADDFOLDER "%ProgramFiles(x86)%\i-dong"
)
call :ADDFOLDER "%SystemDrive%\TOLTech"
call :ADDFOLDER "%SystemDrive%\xeyez_engine"
call :ADDFOLDER "D:\TOLTech"
call :ADDFOLDER "D:\xeyez_engine"
if not "%~1"=="" call :ADDFOLDER "%~1"

if not exist "%TMPROOTS%" (
  echo [ERR] No install folder found anywhere.
  echo Looked in: the registry values TOLTech\Location,
  echo Toltech\xeyez_engine\Location and \RootPath, the services
  echo xEyeService/xeyez, Program Files, C:\TOLTech,
  echo C:\xeyez_engine and the same folders on D:.
  echo If your engine lives somewhere else, run this file from a
  cmd line with the folder as argument:
  echo     patch_engine34.bat "D:\my folder\xeyez_engine"
  echo Press any key to close this window...
  pause >nul
  exit /b 1
)

echo [1/4] Install roots found:
for /f "usebackq delims=" %%R in ("%TMPROOTS%") do echo       %%R
echo.

REM ---- 2. find every xEyeUSB.dll under those roots ----
for /f "usebackq delims=" %%R in ("%TMPROOTS%") do (
  for /f "delims=" %%F in ('dir /s /b "%%R\xEyeUSB.dll" 2^>nul') do call :ONCE "%%F"
)
if not exist "%TMPDLLS%" (
  echo [ERR] No xEyeUSB.dll under the install roots above.
  echo       The engine folder exists but is incomplete - reinstall
  echo       the idong software first.
  echo Press any key to close this window...
  pause >nul
  exit /b 1
)
set /a NFOUND=0
for /f "usebackq delims=" %%F in ("%TMPDLLS%") do set /a NFOUND+=1
echo [2/4] Found !NFOUND! xEyeUSB.dll:
for /f "usebackq delims=" %%F in ("%TMPDLLS%") do echo       %%F
echo.

REM ---- 3. classify and patch each DLL ----
set /a NPATCHED=0
set /a NOK=0
set /a NREFUSED=0
for /f "usebackq delims=" %%F in ("%TMPDLLS%") do call :PROCESS "%%F"

echo.
echo [4/4] Summary: !NPATCHED! patched, !NOK! already current, !NREFUSED! refused.
if !NPATCHED! GTR 0 (
  echo.
  echo DONE. The engine DLLs are unlocked now. Next:
  echo   1. run run_direct34.bat  - starts the engine directly,
  echo      no lobby needed, and shows the camera viewer
  echo   2. the tray icon should switch to "running" within ~15 s
  echo   3. run run_capture.bat for a USBPcap capture while the
  echo      camera runs, then check it with check_wire34.bat
)
if !NREFUSED! GTR 0 (
  echo.
  echo Some files were REFUSED. Read the lines above - a refused
  echo file is a build this kit does not know. Nothing was changed.
)
echo.
echo Press any key to close this window...
pause >nul
del "%TMPROOTS%" 2>nul
del "%TMPDLLS%" 2>nul
exit /b 0

REM ================= helpers =================

:ONCE  -- add a DLL path to the list unless already there
findstr /i /x /l /c:"%~1" "%TMPDLLS%" >nul 2>&1
if errorlevel 1 >>"%TMPDLLS%" echo %~1
goto :eof

:ADDFOLDER  -- add a folder root if it exists
if exist "%~1\" (
  findstr /i /x /l /c:"%~1" "%TMPROOTS%" >nul 2>&1
  if errorlevel 1 >>"%TMPROOTS%" echo %~1
)
goto :eof

:ADDREG  -- add the folder named by a registry value
for /f "tokens=2*" %%A in ('reg query "%~1" /v "%~2" 2^>nul ^| findstr /i "%~2"') do (
  if exist "%%B\" (
    findstr /i /x /l /c:"%%B" "%TMPROOTS%" >nul 2>&1
    if errorlevel 1 >>"%TMPROOTS%" echo %%B
  )
)
goto :eof

:ADDSVC  -- add the folder of a service executable
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

:PROCESS  -- classify and patch one xEyeUSB.dll (+ its xEyeRight.dll)
set "U=%~1"
set "UD=%~dp1"
set "USTATE=foreign"
fc /b "%U%" "xEyeUSB\xEyeUSB_t33.dll" >nul 2>&1 && set "USTATE=t33"
if not "!USTATE!"=="t33" fc /b "%U%" "xEyeUSB\xEyeUSB_t31.dll" >nul 2>&1 && set "USTATE=t31"
if "!USTATE!"=="foreign" fc /b "%U%" "xEyeUSB\xEyeUSB_vendor.dll" >nul 2>&1 && set "USTATE=vendor"
if "!USTATE!"=="foreign" fc /b "%U%" "xEyeUSB\xEyeUSB_vendor_v25.dll" >nul 2>&1 && set "USTATE=vendor25"

if "!USTATE!"=="t33" (
  echo [skip] "%U%" is already fully patched ^(turn 33^).
  set /a NOK+=1
) else if "!USTATE!"=="t31" (
  call :PATCHFILE "%U%" "xEyeUSB\xEyeUSB_t33.dll" "idong34"
  if !ERROROK! == 1 set /a NPATCHED+=1
) else if "!USTATE!"=="vendor" (
  call :PATCHFILE "%U%" "xEyeUSB\xEyeUSB_t33.dll" "idong34"
  if !ERROROK! == 1 set /a NPATCHED+=1
) else if "!USTATE!"=="vendor25" (
  echo [refuse] "%U%" is the v2.5.0.7 engine build.
  echo          This kit unlocks the v2.6.0.6 build. If this is the
  echo          engine you actually run, install the v2.6.0.6 package
  echo          ^(its folder is called xeyez_engine^) and run this
  echo          script again.
  set /a NREFUSED+=1
) else (
  echo [refuse] "%U%" is not a known build. Nothing changed.
  set /a NREFUSED+=1
)

REM ---- the sibling xEyeRight.dll (the license check) ----
set "R=!UD!xEyeRight.dll"
if not exist "!R!" goto :eof
set "RSTATE=foreign"
fc /b "!R!" "xEyeRight\xEyeRight_t34.dll" >nul 2>&1 && set "RSTATE=t34"
if not "!RSTATE!"=="t34" fc /b "!R!" "xEyeRight\xEyeRight_t34_v25.dll" >nul 2>&1 && set "RSTATE=t34"
if "!RSTATE!"=="t34" (
  echo [skip] "!R!" is already patched ^(turn 34^).
  goto :eof
)
set "RREF="
fc /b "!R!" "xEyeRight\xEyeRight_vendor.dll" >nul 2>&1 && set "RREF=xEyeRight\xEyeRight_t34.dll"
if not defined RREF fc /b "!R!" "xEyeRight\xEyeRight_vendor_v25.dll" >nul 2>&1 && set "RREF=xEyeRight\xEyeRight_t34_v25.dll"
if defined RREF (
  call :PATCHFILE "!R!" "!RREF!" "idong34"
  goto :eof
)
echo [refuse] "!R!" is not a known build. Nothing changed.
goto :eof

:PATCHFILE  -- backup + copy reference + verify + rollback
set "T=%~1"
set "SRC=%~2"
set "TAG=%~3"
set "ERROROK=0"
fc /b "%SRC%" "%T%" >nul 2>&1
if not errorlevel 1 (
  echo [ok] "%T%" already matches the reference.
  set "ERROROK=1"
  goto :eof
)
if not exist "%T%.%TAG%-backup" (
  copy /y "%T%" "%T%.%TAG%-backup" >nul
)
copy /y "%SRC%" "%T%" >nul
fc /b "%T%" "%SRC%" >nul 2>&1
if errorlevel 1 (
  echo [ERR] verification failed for "%T%". Restoring the backup...
  copy /y "%T%.%TAG%-backup" "%T%" >nul
  echo       Restored. Nothing was changed.
) else (
  echo [patched] "%T%"  ^(backup: %TAG%-backup^)
  set "ERROROK=1"
)
goto :eof
