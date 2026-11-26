@echo off
REM ============================================================
REM  restore_engine34.bat - undo the turn-34 unlock
REM  Restores every xEyeUSB.dll / xEyeRight.dll that has a
REM  .idong34-backup next to it. Pure cmd.exe. ADMIN required.
REM ============================================================
setlocal enabledelayedexpansion
title idong100 - restore engine DLLs
cd /d "%~dp0"
echo === idong100 engine restore, turn 34 ===
echo.

net session >nul 2>&1
if errorlevel 1 (
  echo [ERR] Please right-click this file and "Run as administrator".
  pause >nul
  exit /b 1
)

set "TMPROOTS=%TEMP%\idong34r_roots.txt"
set "TMPDLLS=%TEMP%\idong34r_dlls.txt"
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
call :ADDFOLDER "D:\TOLTech"
if not "%~1"=="" call :ADDFOLDER "%~1"

if exist "%TMPROOTS%" (
  for /f "usebackq delims=" %%R in ("%TMPROOTS%") do (
    for /f "delims=" %%F in ('dir /s /b "%%R\xEyeUSB.dll" 2^>nul') do call :ONCE "%%F"
    for /f "delims=" %%F in ('dir /s /b "%%R\xEyeRight.dll" 2^>nul') do call :ONCE "%%F"
  )
)
if not exist "%TMPDLLS%" (
  echo [ERR] no engine DLLs found - nothing to restore.
  goto :done
)
set /a N=0
for /f "usebackq delims=" %%F in ("%TMPDLLS%") do call :RESTORE "%%F"
echo Restored !N! file^(s^).
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

:RESTORE
if exist "%~1.idong34-backup" (
  copy /y "%~1.idong34-backup" "%~1" >nul
  echo restored "%~1"
  set /a N+=1
)
goto :eof
