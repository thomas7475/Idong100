@echo off
REM ============================================================
REM  run_direct37.bat - run the camera WITHOUT the lobby
REM  Starts the engine (xeyez.exe) directly with the correct
REM  working directory, then offers xEyeThirdLens.exe (the
REM  standalone camera viewer). Needs patch_all37.bat applied
REM  once. Pure cmd.exe.
REM ============================================================
setlocal enabledelayedexpansion
title idong100 - direct run
cd /d "%~dp0"
echo === idong100 direct run ===
echo.

REM ---- 1. collect candidate install roots ----
set "TMPROOTS=%TEMP%\idong37d_roots.txt"
set "TMPDLLS=%TEMP%\idong37d_dlls.txt"
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
  echo [ERR] No install folder found.
  goto :done
)

REM ---- 2. bounded scan for the engine host xeyez.exe ----
echo Looking for xeyez.exe - a few seconds at most:
for /f "usebackq delims=" %%R in ("%TMPROOTS%") do (
  echo       %%R
  call :SCANROOT "%%R" "xeyez.exe"
)
if not exist "%TMPDLLS%" (
  echo [ERR] No xeyez.exe under the install roots.
  goto :done
)
set "ENGINEEXE="
set "ENGINECHS="
for /f "usebackq delims=" %%F in ("%TMPDLLS%") do (
  if not defined ENGINEEXE (
    set "ENGINECHS=%%~dpF"
    set "ENGINEEXE=%%F"
  )
)
if not defined ENGINEEXE (
  echo [ERR] No xeyez.exe found.
  goto :done
)
echo Engine: !ENGINEEXE!
echo.
echo The engine window shows its state; the tray icon should reach
echo running within ~15 s after the sensor is plugged in. If it shows
echo a not-authorized box instead, run verify_all37.bat first.
echo.
set "P3RDMOD=!ENGINECHS!xEyeThirdLens.exe"
if not exist "!P3RDMOD!" (
  echo [ERR] xEyeThirdLens.exe not found next to the engine host.
  goto :startengine
)
echo Starting the standalone camera viewer in 5 seconds. Close its
echo window any time - it does not stop the engine.
start "" "!P3RDMOD!"
ping -n 6 127.0.0.1 >nul
:startengine
echo Starting the engine now. Close its window to stop it.
start "" /d "!ENGINECHS!" "!ENGINEEXE!"
echo.
:done
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

