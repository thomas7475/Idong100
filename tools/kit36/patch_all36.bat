@echo off
REM ============================================================
REM  patch_all36.bat - idong sensor engine unlock, kit 36
REM  Pure cmd.exe - NO PowerShell. Run as ADMINISTRATOR.
REM
REM  WHAT THIS DOES: this kit ships the FINISHED patched DLLs in
REM  the 'patched' folder. This script finds your engine install,
REM  checks the state of each DLL and copies the right patched
REM  file over it (backup first, byte-verify after, roll back on
REM  any error). It can NOT byte-patch, so it can not corrupt a
REM  file: every step is a copy plus an fc /b comparison.
REM
REM  The two gates it removes (both proven on the wire):
REM    xEyeUSB.dll  - upsell exit + version-tag gate 'DSHY'
REM                  (this unit answers '000T' - letter T refused)
REM    xEyeRight.dll- license: the engine phones us.i-dong.com,
REM                  a server dead since years, so a stock install
REM                  can never be authorized. Patched: it always
REM                  reports authorized + fires the vendor's own
REM                  success notification (shown in emulation).
REM                  THIS is the gate behind the 'hardware not
REM                  authorized' box (sorrytext.bmp / DeviceID.bmp /
REM                  phone.bmp / email.bmp / bbs.bmp of
REM                  xEyeWarrningUI.dll = ShowDeviceIDInvalidWindow).
REM
REM  Accepted states per file (fc /b against the kit references):
REM    vendor -> patched, older patch -> patched, current -> skip,
REM    v2.5.0.7 xEyeUSB build -> reported (the xEyeRight patch
REM    DOES apply to a v2.5 engine), anything else -> REFUSED.
REM ============================================================
setlocal enabledelayedexpansion
title idong100 engine unlock - kit 36
cd /d "%~dp0"
echo === idong100 engine unlock, kit 36 ===
echo.

REM ---- 0. must run elevated ----
net session >nul 2>&1
if errorlevel 1 (
  echo [ERR] Please right-click this file and "Run as administrator".
  echo Press any key to close this window...
  pause >nul
  exit /b 1
)

REM ---- 0b. the kit folders must be complete ----
if not exist "patched\xEyeUSB_t33.dll"  goto :NOkit
if not exist "patched\xEyeRight_t34.dll"     goto :NOkit
if not exist "patched\xEyeRight_t34_v25.dll"  goto :NOkit
if not exist "ref\xEyeUSB_vendor_v26.dll"     goto :NOkit
if not exist "ref\xEyeUSB_t31.dll"           goto :NOkit
if not exist "ref\xEyeUSB_vendor_v25.dll"     goto :NOkit
if not exist "ref\xEyeRight_vendor_v26.dll"  goto :NOkit
if not exist "ref\xEyeRight_vendor_v25.dll"  goto :NOkit

REM ---- 1. collect candidate install roots (registry, services,
REM      Program Files, fixed drives; optional argument folder) ----
set "TMPROOTS=%TEMP%\idong36_roots.txt"
set "TMPDLLS=%TEMP%\idong36_dlls.txt"
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
  echo cmd line with the folder as argument:
  echo     patch_all36.bat "D:\my folder\xeyez_engine"
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
  echo DONE. Both gates are open now. Next:
  echo   1. start the lobby - it should get past the hardware test
  echo   2. or run run_direct36.bat - engine + camera viewer
  echo      without the lobby (the tray icon should reach
  echo      running within ~15 s)
  echo   3. run run_capture.bat for a USBPcap capture while the
  echo      camera runs, then check it with check_capture.bat
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

:NOkit
echo [ERR] This script must run from the complete kit folder
echo       (patched\ and ref\ next to it). Nothing was changed.
echo Press any key to close this window...
pause >nul
exit /b 1

:PROCESS  -- classify and patch one xEyeUSB.dll + its xEyeRight.dll
set "U=%~1"
set "UD=%~dp1"
set "USTATE=foreign"
fc /b "%U%" "patched\xEyeUSB_t33.dll" >nul 2>&1 && set "USTATE=t33"
if not "!USTATE!"=="t33" fc /b "%U%" "ref\xEyeUSB_t31.dll" >nul 2>&1 && set "USTATE=t31"
if "!USTATE!"=="foreign" fc /b "%U%" "ref\xEyeUSB_vendor_v26.dll" >nul 2>&1 && set "USTATE=vendor"
if "!USTATE!"=="foreign" fc /b "%U%" "ref\xEyeUSB_vendor_v25.dll" >nul 2>&1 && set "USTATE=vendor25"

if "!USTATE!"=="t33" (
  echo [skip] "%U%" is already fully patched.
  set /a NOK+=1
) else if "!USTATE!"=="t31" (
  call :PATCHFILE "%U%" "patched\xEyeUSB_t33.dll" "idong36"
  if !ERROROK! == 1 set /a NPATCHED+=1
) else if "!USTATE!"=="vendor" (
  call :PATCHFILE "%U%" "patched\xEyeUSB_t33.dll" "idong36"
  if !ERROROK! == 1 set /a NPATCHED+=1
) else if "!USTATE!"=="vendor25" (
  echo [note] "%U%" is the v2.5.0.7 engine build.
  echo        The upsell/tag patch targets the v2.6.0.6 build, so
  echo        this file is left as it is (the v2.5 engine has no
  echo        version-tag gate anyway). The license patch below
  echo        DOES apply to a v2.5 engine.
) else (
  echo [refuse] "%U%" is not a known build. Nothing changed.
  set /a NREFUSED+=1
)

REM ---- the sibling xEyeRight.dll (the license gate) ----
set "R=!UD!xEyeRight.dll"
if not exist "!R!" goto :eof
set "RSTATE=foreign"
fc /b "!R!" "patched\xEyeRight_t34.dll" >nul 2>&1 && set "RSTATE=t34"
if not "!RSTATE!"=="t34" fc /b "!R!" "patched\xEyeRight_t34_v25.dll" >nul 2>&1 && set "RSTATE=t34"
if "!RSTATE!"=="t34" (
  echo [skip] "!R!" is already patched.
  goto :eof
)
set "RREF="
set "RDLL="
fc /b "!R!" "ref\xEyeRight_vendor_v26.dll" >nul 2>&1 && ( set "RDLL=patched\xEyeRight_t34.dll" )
if not defined RDLL fc /b "!R!" "ref\xEyeRight_vendor_v25.dll" >nul 2>&1 && ( set "RDLL=patched\xEyeRight_t34_v25.dll" )
if defined RDLL (
  call :PATCHFILE "!R!" "!RDLL!" "idong36"
  goto :eof
)
echo [refuse] "!R!" is not a known build. Nothing changed.
goto :eof

:PATCHFILE  -- backup + copy the patched reference + verify + rollback
set "T=%~1"
set "SRC=%~2"
set "TAG=%~3"
set "ERROROK=0"
fc /b "%SRC%" "%T%" >nul 2>&1
if not errorlevel 1 (
  echo [ok] "%T%" already matches the kit file.
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
  echo [patched] "%T%"
  set "ERROROK=1"
)
goto :eof
