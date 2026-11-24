@echo off
REM ============================================================
REM  check_capture.bat - idong100 P2.3 capture sanity check (turn 18)
REM
REM  Pure cmd.exe - NO PowerShell anywhere (user rule, turn 18).
REM  The exact byte counting is done by a tiny embedded VBScript run
REM  via cscript (ships with every Windows since 98 - it is not
REM  PowerShell). The .bat writes the .vbs to %TEMP% at runtime.
REM
REM  Scans a USBPcap pcapng for the vendor protocol markers:
REM    - PU magic 50 55 7f 7f (host->device boot frames)
REM    - OD magic 4f 44 7f 7f (device->host boot frames)
REM    - "Ready to load program!" ASCII (type-2 ready reply)
REM
REM  Usage:  check_capture.bat C:\idong_capture\boot.pcapng
REM  Exit codes: 0 = PASS, 2 = FAIL (same codes as the old .ps1)
REM ============================================================
setlocal enabledelayedexpansion
if "%~1"=="" (
  echo [ERR] usage: check_capture.bat C:\idong_capture\boot.pcapng
  exit /b 1
)
if not exist "%~1" (
  echo [ERR] file not found: %~1
  exit /b 1
)
for %%F in ("%~1") do echo capture size: %%~zF bytes

set "VBS=%TEMP%\idong_cap_scan.vbs"
set "OUT=%TEMP%\idong_cap_result.txt"

> "%VBS%" echo Option Explicit
>>"%VBS%" echo Dim fso, shell, f, d, i, nPU, nOD, nRD
>>"%VBS%" echo Set fso = CreateObject("Scripting.FileSystemObject"^)
>>"%VBS%" echo Set shell = CreateObject("WScript.Shell"^)
>>"%VBS%" echo d = fso.GetAbsolutePathName(WScript.Arguments(0^)^)
>>"%VBS%" echo Set f = fso.OpenTextFile(d, 1, False, -1^) 'TristateTrue=binary-safe? no: use ADODB
>>"%VBS%" echo f.Close
>>"%VBS%" echo ' binary read via ADODB.Stream (no BOM mangling^)
>>"%VBS%" echo Dim bs
>>"%VBS%" echo Set bs = CreateObject("ADODB.Stream"^)
>>"%VBS%" echo bs.Type = 1 'adTypeBinary
>>"%VBS%" echo bs.Open
>>"%VBS%" echo bs.LoadFromFile d
>>"%VBS%" echo Dim b(): ReDim b(bs.Size - 1^)
>>"%VBS%" echo If bs.Size ^> 0 Then
>>"%VBS%" echo   b = bs.Read
>>"%VBS%" echo End If
>>"%VBS%" echo bs.Close
>>"%VBS%" echo nPU = 0: nOD = 0: nRD = 0
>>"%VBS%" echo ' PU magic 50 55 7f 7f
>>"%VBS%" echo For i = 0 To UBound(b^) - 3
>>"%VBS%" echo   If b(i^) = ^&H50 And b(i+1^) = ^&H55 And b(i+2^) = ^&H7F And b(i+3^) = ^&H7F Then nPU = nPU + 1
>>"%VBS%" echo Next
>>"%VBS%" echo ' OD magic 4f 44 7f 7f
>>"%VBS%" echo For i = 0 To UBound(b^) - 3
>>"%VBS%" echo   If b(i^) = ^&H4F And b(i+1^) = ^&H44 And b(i+2^) = ^&H7F And b(i+3^) = ^&H7F Then nOD = nOD + 1
>>"%VBS%" echo Next
>>"%VBS%" echo ' ASCII "Ready to load program!"
>>"%VBS%" echo Dim s: s = "Ready to load program!"
>>"%VBS%" echo For i = 0 To UBound(b^) - 24
>>"%VBS%" echo   Dim j, m: m = True
>>"%VBS%" echo   For j = 0 To 24
>>"%VBS%" echo     If Chr(b(i+j^)^) ^<^> Mid(s, j+1, 1^) Then m = False: Exit For
>>"%VBS%" echo   Next
>>"%VBS%" echo   If m Then nRD = nRD + 1: i = i + 24
>>"%VBS%" echo Next
>>"%VBS%" echo ' write results for the .bat to read
>>"%VBS%" echo Dim o: Set o = fso.CreateTextFile(WScript.Arguments(1^), True^)
>>"%VBS%" echo o.WriteLine nPU
>>"%VBS%" echo o.WriteLine nOD
>>"%VBS%" echo o.WriteLine nRD
>>"%VBS%" echo o.Close

cscript //nologo "%VBS%" "%~1" "%OUT%" >nul 2>&1
if not exist "%OUT%" (
  echo [ERR] vbs scanner failed - check cscript is allowed in the VM.
  echo        ^(it is a standard Windows component, not PowerShell^)
  del "%VBS%" >nul 2>&1
  exit /b 3
)

set /a PU=0, OD=0, READY=0
< "%OUT%" (
  set /a PU=
  set /a OD=
  set /a READY=
)
del "%VBS%" "%OUT%" >nul 2>&1

echo PU magic (50 55 7f 7f^)      : %PU% occurrences
echo OD magic (4f 44 7f 7f^)      : %OD% occurrences
echo ready-string occurrences    : %READY%

set OK=1
if %PU% LSS 10   ( echo [FAIL] too few PU frames - boot upload probably not captured & set OK=0 )
if %OD% LSS 3    ( echo [FAIL] too few OD frames - device replies missing & set OK=0 )
if %READY% LSS 1 ( echo [FAIL] no 'Ready to load program!' - device never answered DSP-start & set OK=0 )
if %PU% LSS 500 (
  echo [WARN] fewer than 500 PU frames - full packet train may be short;
  echo        a staging order that starts with a small blob can be normal.
)

echo.
if "%OK%"=="1" (
  echo PASS - the capture contains the vendor boot sequence.
  echo Copy it to the analysis host for the byte-for-byte gate.
  exit /b 0
) else (
  echo FAIL - redo the capture per README_CAPTURE.md section 5
  echo ^(power-cycle the sensor first so it is in the bootloader state^).
  exit /b 2
)
