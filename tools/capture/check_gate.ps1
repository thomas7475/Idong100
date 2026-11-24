# check_gate.ps1 - verify the platform-gate probes in your Windows VM (admin)
# Reads the same things VersionCheck.dll's gate reads and reports status.
# Usage: powershell -ExecutionPolicy Bypass -File check_gate.ps1

$ErrorActionPreference = 'Continue'
Write-Host "=== idong platform-gate probe status ==="

# 1. The registry mark the launcher reads
$loc = Get-ItemProperty -Path 'HKLM:\SOFTWARE\TOLTech' -ErrorAction SilentlyContinue
if ($loc -and $loc.Location) {
    Write-Host "[OK]   HKLM\SOFTWARE\TOLTech\Location = $($loc.Location)"
} else {
    Write-Host "[MISS] HKLM\SOFTWARE\TOLTech\Location - import platform_mark.reg"
}

$root = Get-ItemProperty -Path 'HKLM:\SOFTWARE\TOLTech\xeyez_engine' -ErrorAction SilentlyContinue
if ($root -and $root.RootPath) {
    Write-Host "[OK]   HKLM\SOFTWARE\TOLTech\xeyez_engine\RootPath = $($root.RootPath)"
} else {
    Write-Host "[MISS] HKLM\SOFTWARE\TOLTech\xeyez_engine\RootPath (optional for gate)"
}

# 2. Virtual HID device nodes (exist only after platform's vMouse/vKeyboard
#    drivers are installed - NOT required for the launcher gate to pass;
#    informational)
foreach ($dev in @('vMouse', 'vKeyboard')) {
    $h = [System.IO.File]::Open("\\.\$dev", 'Open', 'Read')
    $null = $h.Close()
    if ($?) { Write-Host "[OK]   \\.\$dev device node present" }
    else   { Write-Host "[----] \\.\$dev absent (fine for standalone sensor install)" }
}

# 3. %CommonAppData%\i-dong folder (platform data dir; informational)
$appdata = [Environment]::GetFolderPath('CommonApplicationData')
if (Test-Path "$appdata\i-dong") {
    Write-Host "[OK]   $appdata\i-dong exists"
} else {
    Write-Host "[----] $appdata\i-dong absent (fine)"
}

# 4. Sensor driver presence (after install): the xeyez service
$svc = Get-Service -Name xeyez -ErrorAction SilentlyContinue
if ($svc) {
    Write-Host "[OK]   xeyez driver service exists (status: $($svc.Status))"
} else {
    Write-Host "[----] xeyez service not installed yet (expected before install)"
}

Write-Host ""
if ($loc -and $loc.Location) {
    Write-Host "RESULT: launcher gate SATISFIED - xeyez_setup.exe should proceed."
} else {
    Write-Host "RESULT: gate NOT satisfied - import platform_mark.reg first."
}
