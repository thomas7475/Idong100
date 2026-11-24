@echo off
REM ============================================================
REM  idong100 P2.3 capture session (run INSIDE the Windows VM)
REM  Prereq: Wireshark + USBPcap installed (see README_CAPTURE.md)
REM  Output: C:\idong_capture\boot.pcapng (+ capture log)
REM ============================================================
setlocal enabledelayedexpansion
set CAPDIR=C:\idong_capture
set USBPCAP=C:\Program Files\USBPcap1\USBPcapCMD.exe

if not exist "%USBPCAP%" (
  echo [ERR] USBPcapCMD not found at "%USBPCAP%"
  echo        Install USBPcapSetup-1.5.4.0.exe first - README section 3.
  pause & exit /b 1
)

mkdir %CAPDIR% 2>nul
cd /d %CAPDIR%

echo.
echo ============================================================
echo  idong100 vendor-boot capture - guided session
echo ============================================================
echo.
echo  1. ATTACH the sensor to the VM now (virt-manager hostdev).
echo     Wait until Device Manager shows 0471:2600 (unknown device
echo     is fine - no driver bound yet).
echo  2. Do NOT start the vendor app yet.
echo.
pause

REM Start the root-hub capture BEFORE any enumeration:
REM -d \\.\USBPcap1 filters the ROOT hub so both PIDs (2600 and 2500)
REM    land in ONE file across the re-enumeration.
echo [1/4] starting USBPcap root-hub capture...
start "USBPcap" /min "%USBPCAP%" -d \\.\USBPcap1 -o %CAPDIR%\boot.pcapng --snap 0
timeout /t 3 /nobreak >nul

echo.
echo  3. NOW start the vendor app (xeyez.exe, or let the tray
echo     xeyezMonitor service detect the sensor). The firmware
echo     upload happens within ~10 seconds.
echo  4. After the app shows the sensor running, let it stream
echo     for at least 60 seconds (keep-alive + marker traffic).
echo.
pause

echo [2/4] capture window done - stopping USBPcap...
taskkill /im usbpcapfilter.sys 2>nul
taskkill /im USBPcapCMD.exe 2>nul

echo [3/4] sanity check of the capture file...
if exist boot.pcapng (
  for %%F in (boot.pcapng) do echo     boot.pcapng : %%~zF bytes
) else (
  echo [ERR] no boot.pcapng produced - capture failed.
  pause & exit /b 1
)

echo [4/4] magic-byte scan (pure cmd + cscript - no PowerShell)...
call "%~dp0check_capture.bat" "%CAPDIR%\boot.pcapng"

echo.
echo  DONE. Copy C:\idong_capture\ to the analysis host.
echo  The byte-for-byte gate comparison runs on the host (tshark
echo  + the Phase-1 Rust decoder).
echo.
pause
endlocal
