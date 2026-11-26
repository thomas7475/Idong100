@echo off
REM ============================================================
REM  restore_all37.bat - undo the kit 37 unlock
REM  Restores every xEyeUSB.dll / xEyeRight.dll / xEyeWarrningUI.dll
REM  that has an .idong3*-backup next to it. Failing that, the vendor
REM  file is restored from the ref folder - a foreign file is never
REM  overwritten.
REM  Pure cmd.exe. ADMIN required. Kit 37.
REM ============================================================
setlocal enabledelayedexpansion
title idong100 - restore engine DLLs
cd /d "%~dp0"
echo === idong100 engine restore, kit 37 ===
echo.


fltmc >nul 2>&1
if errorlevel 1 (
  echo [ERR] Please right-click this file and "Run as administrator".
  echo Press any key to close this window...
  pause >nul
  exit /b 1
)


REM ---- 1. collect candidate install roots ----
set "TMPROOTS=%TEMP%\idong37r_roots.txt"
set "TMPDLLS=%TEMP%\idong37r_dlls.txt"
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
  echo [ERR] No install folder found. Nothing to restore.
  goto :done
)

REM ---- 2. bounded scan for the engine DLLs ----
echo Looking for the engine DLLs - a few seconds at most:
for /f "usebackq delims=" %%R in ("%TMPROOTS%") do (
  echo       %%R
  call :SCANROOT "%%R" "xEyeUSB.dll"
)
if not exist "%TMPDLLS%" (
  echo [ERR] No xEyeUSB.dll found. Nothing to restore.
  goto :done
)

REM ---- 3. restore each folder ----
for /f "usebackq delims=" %%F in ("%TMPDLLS%") do call :RESTOREFOLDER "%%F"
echo.
echo Done. The lines above say what was restored.
echo.
:done
echo Press any key to close this window...
pause >nul
del "%TMPROOTS%" 2>nul
del "%TMPDLLS%" 2>nul
exit /b 0

:RESTOREFOLDER  -- restore the three gate DLLs of one engine folder
set "U=%~1"
set "UD=%~dp1"
call :RESTOREDLL "%U%" "xEyeUSB.dll" "ref\xEyeUSB_vendor_v26.dll" "ref\xEyeUSB_vendor_v25.dll"
call :RESTOREDLL "!UD!xEyeRight.dll" "xEyeRight.dll" "ref\xEyeRight_vendor_v26.dll" "ref\xEyeRight_vendor_v25.dll"
call :RESTOREDLL "!UD!xEyeWarrningUI.dll" "xEyeWarrningUI.dll" "ref\xEyeWarrningUI_vendor_v26.dll" "ref\xEyeWarrningUI_vendor_v25.dll"
goto :eof

:RESTOREDLL  -- %1 dll, %2 short name, %3 vendor v26 ref, %4 vendor v25 ref
if not exist "%~1" goto :eof
set "BACKSET="
for %%B in ("%~1.idong3?-backup") do if not defined BACKSET if exist "%%B" set "BACKSET=%%B"
if defined BACKSET (
  copy /y "!BACKSET!" "%~1" >nul
  echo [restored] %~1 from !BACKSET!
  goto :eof
)
fc /b "%~1" "%~3" >nul 2>&1
if not errorlevel 1 (
  echo [note] %~2 is already the vendor file. Nothing to do.
  goto :eof
)
fc /b "%~1" "%~4" >nul 2>&1
if not errorlevel 1 (
  echo [note] %~2 is already the vendor v2.5 file. Nothing to do.
  goto :eof
)
echo [refuse] %~2 is not a vendor state this kit knows and has no
echo         backup. It was NOT touched.
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

