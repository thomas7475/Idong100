@echo off
REM ============================================================
REM  run_direct36.bat - run the camera WITHOUT the lobby
REM  Starts the engine (xeyez.exe) directly with the correct
REM  working directory, then offers xEyeThirdLens.exe (the
REM  standalone camera viewer). Needs patch_all36.bat applied
REM  once. Pure cmd.exe.
REM ============================================================
setlocal enabledelayedexpansion
title idong100 - direct run
cd /d "%~dp0"
echo === idong100 direct run ===
echo.

REM ---- 1. collect candidate install roots (registry, services,
REM      Program Files, fixed drives; optional argument folder) ----
set "TMPROOTS=%TEMP%\idong36d_roots.txt"
set "TMPDLLS=%TEMP%\idong36d_dlls.txt"
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
  echo [ERR] No install folder found.
  goto :done
)
set "ENGINEEXE="
set "ENGINECHS="
for /f "usebackq delims=" %%R in ("%TMPROOTS%") do (
  if not defined ENGINEEXE (
    for /f "delims=" %%F in ('dir /s /b "%%R\xeyez.exe" 2^>nul') do (
      if not defined ENGINEEXE (
        set "ENGINECHS=%%~dpF"
        set "ENGINEEXE=%%F"
      )
    )
  )
)
if not defined ENGINEEXE (
  echo [ERR] No xeyez.exe under the install roots.
  goto :done
)
echo Engine: !ENGINEEXE!
echo.
echo The engine window shows its state; the tray icon should reach
echo running within ~15 s after the sensor is plugged in. If it shows
echo a not-authorized box instead, run verify_all36.bat.
echo.
set "P3RDMOD=!ENGINECHS!xEyeThirdLens.exe"
if not exist "!P3RDMOD!" (
  echo [note] xEyeThirdLens.exe not found in the engine folder -
  echo        starting the engine only.
  start "" /d "!ENGINECHS!" xeyez.exe
  goto :done
)
echo Choice:
echo   1 = engine + camera viewer (xEyeThirdLens.exe)
echo   2 = engine only
echo   anything else = nothing starts
set /p CHOICE="1/2: "
if "!CHOICE!"=="1" (
  start "" /d "!ENGINECHS!" xeyez.exe
  echo Waiting 8 s for the engine to settle...
  ping -n 9 127.0.0.1 >nul
  start "" /d "!ENGINECHS!" xEyeThirdLens.exe
) else if "!CHOICE!"=="2" (
  start "" /d "!ENGINECHS!" xeyez.exe
) else (
  echo Nothing started.
)
:done
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
