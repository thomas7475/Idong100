@echo off
REM ============================================================
REM  restore_all36.bat - undo the kit 36 unlock
REM  Restores every xEyeUSB.dll / xEyeRight.dll that has a
REM  .idong36-backup next to it. Failing that, the vendor file
REM  is restored from the ref folder (byte state must match a
REM  known one - a foreign file is never overwritten).
REM  Pure cmd.exe. ADMIN required.
REM ============================================================
setlocal enabledelayedexpansion
title idong100 - restore engine DLLs
cd /d "%~dp0"
echo === idong100 engine restore, kit 36 ===
echo.

net session >nul 2>&1
if errorlevel 1 (
  echo [ERR] Please right-click this file and Run as administrator.
  pause >nul
  exit /b 1
)

REM ---- 1. collect candidate install roots (registry, services,
REM      Program Files, fixed drives; optional argument folder) ----
set "TMPROOTS=%TEMP%\idong36r_roots.txt"
set "TMPDLLS=%TEMP%\idong36r_dlls.txt"
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
  echo [ERR] No install folder found. Nothing to restore.
  goto :done
)
for /f "usebackq delims=" %%R in ("%TMPROOTS%") do (
  for /f "delims=" %%F in ('dir /s /b "%%R\xEyeUSB.dll" 2^>nul ^| findstr /v /i "backup"') do call :ONCE "%%F"
  for /f "delims=" %%F in ('dir /s /b "%%R\xEyeRight.dll" 2^>nul ^| findstr /v /i "backup"') do call :ONCE "%%F"
)
if not exist "%TMPDLLS%" (
  echo [ERR] No engine DLLs found.
  goto :done
)
for /f "usebackq delims=" %%F in ("%TMPDLLS%") do call :RESTOREONE "%%F"
:done
echo.
echo Press any key to close this window...
pause >nul
del "%TMPROOTS%" 2>nul
del "%TMPDLLS%" 2>nul
exit /b 0

:RESTOREONE
set "T=%~1"
if exist "%T%.idong36-backup" (
  copy /y "%T%.idong36-backup" "%T%" >nul
  fc /b "%T%" "%T%.idong36-backup" >nul 2>&1 || goto :rbfail
  echo [restored] "%T%" from its .idong36-backup
  goto :eof
)
REM no backup: restore from the ref folder when the state matches
set "REFDLL="
fc /b "%T%" "patched\xEyeUSB_t33.dll" >nul 2>&1 && set "REFDLL=ref\xEyeUSB_vendor_v26.dll"
if not defined REFDLL fc /b "%T%" "patched\xEyeRight_t34.dll" >nul 2>&1 && set "REFDLL=ref\xEyeRight_vendor_v26.dll"
if not defined REFDLL fc /b "%T%" "patched\xEyeRight_t34_v25.dll" >nul 2>&1 && set "REFDLL=ref\xEyeRight_vendor_v25.dll"
if defined REFDLL (
  copy /y "%REFDLL%" "%T%" >nul
  fc /b "%T%" "%REFDLL%" >nul 2>&1 || goto :rbfail
  echo [restored] "%T%" from the kit reference
  goto :eof
)
echo [skip] "%T%" is not in a patched state (nothing to undo).
goto :eof
:rbfail
echo [ERR] restore verification failed for "%T%" - the file was NOT
echo       replaced safely. Compare by hand with fc /b before retrying.
goto :eof

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
