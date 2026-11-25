@echo off
REM ============================================================
REM  check_gate.bat - idong100 gate probe status (turn 19)
REM  Pure cmd.exe - NO PowerShell (user rule, turn 18).
REM  Turn 19 fixes:
REM   - the German '")" kann syntaktisch...' crash: probe
REM     labels contain parentheses and broke the if/else
REM     block parsing. The :PROBE subroutine now prints the
REM     label BEFORE any parenthesised block; no () in labels.
REM   - truncated value display (tokens=1,2,* + %%C).
REM   - ends with pause so the window STAYS OPEN.
REM
REM  Reports what the gates read, in BOTH registry views:
REM  the platform mark (turn 18) AND the new OS-gate note.
REM
REM  Usage: right-click, Run as administrator
REM ============================================================
setlocal enabledelayedexpansion
title idong100 gate probe

echo === idong100 gate probe status - both registry views ===
echo.

set GATEPASS=0

REM ---- 1. platform mark, 64-bit view
call :PROBE "HKLM\SOFTWARE\TOLTech" "Location" "64-bit view"
call :PROBE "HKLM\SOFTWARE\TOLTech\xeyez_engine" "RootPath" "64-bit view"

REM ---- 2. platform mark, 32-bit view - WHAT THE 32-BIT GATE READS on x64
call :PROBE "HKLM\SOFTWARE\Wow6432Node\TOLTech" "Location" "32-bit view - gate"
call :PROBE "HKLM\SOFTWARE\Wow6432Node\TOLTech\xeyez_engine" "RootPath" "32-bit view - gate"
call :PROBE "HKLM\SOFTWARE\Wow6432Node\Toltech\xeyez_engine" "RootPath" "installer key"

echo.

REM ---- 3. OS bitness - which view matters for the 32-bit gate
if defined PROCESSOR_ARCHITEW6432 (
  echo [i] OS: 64-bit Windows - the 32-bit gate reads the Wow6432Node view.
  set VIEWNEEDED=Wow6432Node
) else if "!PROCESSOR_ARCHITECTURE!"=="AMD64" (
  echo [i] OS: 64-bit Windows - the 32-bit gate reads the Wow6432Node view.
  set VIEWNEEDED=Wow6432Node
) else (
  echo [i] OS: 32-bit Windows - no Wow6432Node, both views are the same.
  set VIEWNEEDED=same
)
echo.

REM ---- 4. OS-version gate NOTE - turn 19
echo [i] OS-version gate: the installers whitelist only XP 5.1 / Vista 6.0 /
echo     Win7 6.1. On Windows 8/10/11 they show the incompatibility dialog.
echo     FIX: use xeyez_setup_patched.exe + xeyez_alone_patched.dat from this
echo     kit - the gate is neutralised there. See SOLVE_DIALOG.md section
echo     'The OS-version gate - turn 19'.
echo.

REM ---- 5. %ProgramData%\i-dong folder probe - optional
set "ADDIR=%ProgramData%"
if not defined ADDIR set "ADDIR=%ALLUSERSPROFILE%\Application Data"
if exist "!ADDIR!\i-dong\" (
  echo [OK]   folder "!ADDIR!\i-dong" exists
) else (
  md "!ADDIR!\i-dong" >nul 2>&1
  if exist "!ADDIR!\i-dong\" (
    echo [FIXD] folder "!ADDIR!\i-dong" created for you
  ) else (
    echo [----] folder "!ADDIR!\i-dong" absent - run as admin to create
  )
)
echo.

REM ---- 6. virtual HID device nodes - informational
for %%D in (vMouse vKeyboard) do (
  if exist "\\.\%%D" (
    echo [OK]   \\.\%%D device node present
  ) else (
    echo [----] \\.\%%D absent - fine for standalone sensor install
  )
)
echo.

REM ---- 7. sensor driver service - exists only after install
sc query xeyez >nul 2>&1
if not errorlevel 1 (
  echo [OK]   xeyez driver service installed
  sc query xeyez | findstr /i "STATE"
) else (
  echo [----] xeyez service not installed yet - expected before install
)
echo.

REM ---- 8. patched installer files present?
echo --- patched installers - turn 19 ---
if exist "%~dp0xeyez_setup_patched.exe" (
  echo [OK]   xeyez_setup_patched.exe present
) else (
  if exist "%~dp0software\xeyez_setup_patched.exe" (
    echo [OK]   software\xeyez_setup_patched.exe present
  ) else (
    echo [----] xeyez_setup_patched.exe NOT found - copy it next to this script
  )
)
if exist "%~dp0xeyez_alone_patched.dat" (
  echo [OK]   xeyez_alone_patched.dat present
) else (
  if exist "%~dp0software\xeyez_alone_patched.dat" (
    echo [OK]   software\xeyez_alone_patched.dat present
  ) else (
    echo [----] xeyez_alone_patched.dat NOT found - copy it next to this script
  )
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
  echo RESULT: platform gate SATISFIED.
) else (
  echo RESULT: platform gate NOT satisfied - import platform_mark.reg first:
  echo    reg import platform_mark.reg     - as administrator
)
echo ============================================================
echo.
echo Press any key to close this window...
pause >nul
endlocal
exit /b 0

:PROBE
REM PROBE subroutine: key, value, view label - no parentheses in echo
reg query "%~1" /v "%~2" >nul 2>&1
if errorlevel 1 (
  echo [MISS] %~1 : %~2 - %~3
) else (
  echo [OK]   %~1 : %~2 - %~3
  for /f "skip=1 tokens=1,2,*" %%A in ('reg query "%~1" /v "%~2" 2^>nul ^| findstr /i "%~2"') do echo         = %%C
)
goto :eof
