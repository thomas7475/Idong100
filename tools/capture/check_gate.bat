@echo off
REM ============================================================
REM  check_gate.bat - idong100 platform-gate probe status (turn 18)
REM  Pure cmd.exe script - NO PowerShell anywhere (per user rule).
REM  Run as Administrator inside the Windows VM.
REM
REM  Reports exactly what the xeyez_setup.exe gate reads, in BOTH
REM  registry views, because the gate (VersionCheck.dll) is 32-bit:
REM  on 64-bit Windows it reads HKLM\SOFTWARE\Wow6432Node\TOLTech.
REM  v1 of the fix wrote only the 64-bit view - that is why the
REM  dialog came back. This script shows both views so you can see
REM  which one is missing on YOUR machine.
REM
REM  Usage: right-click -> Run as administrator
REM         (or from an elevated cmd: check_gate.bat)
REM ============================================================
setlocal enabledelayedexpansion
title idong100 gate probe

echo === idong platform-gate probe status (both registry views) ===
echo.

set GATEPASS=0

REM ---- 1. the registry mark, 64-bit view (what a 64-bit reader sees)
call :PROBE "HKLM\SOFTWARE\TOLTech" "Location" "64-bit view"
call :PROBE "HKLM\SOFTWARE\TOLTech\xeyez_engine" "RootPath" "64-bit view"

REM ---- 2. the registry mark, 32-bit view (WHAT THE GATE READS on x64)
call :PROBE "HKLM\SOFTWARE\Wow6432Node\TOLTech" "Location" "32-bit view (gate)"
call :PROBE "HKLM\SOFTWARE\Wow6432Node\TOLTech\xeyez_engine" "RootPath" "32-bit view (gate)"
call :PROBE "HKLM\SOFTWARE\Wow6432Node\Toltech\xeyez_engine" "RootPath" "installer's own key"

echo.

REM ---- 3. OS bitness (which view matters for the 32-bit gate)
if defined PROCESSOR_ARCHITEW6432 (
  echo [i] OS: 64-bit Windows - the 32-bit gate reads the Wow6432Node view.
  set VIEWNEEDED=Wow6432Node
) else if "%PROCESSOR_ARCHITECTURE%"=="AMD64" (
  echo [i] OS: 64-bit Windows - the 32-bit gate reads the Wow6432Node view.
  set VIEWNEEDED=Wow6432Node
) else (
  echo [i] OS: 32-bit Windows - no Wow6432Node, both views are the same.
  set VIEWNEEACHED=
  set VIEWNEEDED=same
)
echo.

REM ---- 4. %CommonAppData%\i-dong folder (secondary probe; optional)
set "ADDIR=%ProgramData%"
if not defined ADDIR set "ADDIR=%ALLUSERSPROFILE%\Application Data"
if exist "!ADDIR!\i-dong\" (
  echo [OK]   folder "!ADDIR!\i-dong" exists
) else (
  md "!ADDIR!\i-dong" >nul 2>&1
  if exist "!ADDIR!\i-dong\" (
    echo [FIXD] folder "!ADDIR!\i-dong" created for you (optional probe^)
  ) else (
    echo [----] folder "!ADDIR!\i-dong" absent (optional - run as admin to create^)
  )
)
echo.

REM ---- 5. virtual HID device nodes (informational only)
for %%D in (vMouse vKeyboard) do (
  if exist "\\.\%%D" (
    echo [OK]   \\.\%%D device node present
  ) else (
    echo [----] \\.\%%D absent - fine for standalone sensor install
  )
)
echo.

REM ---- 6. sensor driver service (exists only after install)
sc query xeyez >nul 2>&1
if not errorlevel 1 (
  echo [OK]   xeyez driver service installed
  sc query xeyez | findstr /i "STATE"
) else (
  echo [----] xeyez service not installed yet - expected before install
)
echo.

REM ---- verdict: the view that matters must carry Location
if "!VIEWNEEDED!"=="Wow6432Node" (
  reg query "HKLM\SOFTWARE\Wow6432Node\TOLTech" /v "Location" >nul 2>&1
  if not errorlevel 1 set GATEPASS=1
) else (
  reg query "HKLM\SOFTWARE\TOLTech" /v "Location" >nul 2>&1
  if not errorlevel 1 set GATEPASS=1
)

echo ============================================================
if "!GATEPASS!"=="1" (
  echo RESULT: launcher gate SATISFIED - xeyez_setup.exe should proceed.
) else (
  echo RESULT: gate NOT satisfied - import platform_mark_v2.reg first:
  echo    reg import platform_mark_v2.reg     ^(as administrator^)
)
echo ============================================================
pause
endlocal
exit /b 0

:PROBE  -- key, value, view label
reg query "%~1" /v "%~2" >nul 2>&1
if errorlevel 1 (
  echo [MISS] %~1 : %~2   ^(%~3^)
) else (
  echo [OK]   %~1 : %~2   ^(%~3^)
  for /f "tokens=2,3*" %%A in ('reg query "%~1" /v "%~2" 2^>nul ^| findstr /i "%~2"') do echo        = %%C
)
goto :eof
