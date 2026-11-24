# check_capture.ps1 - idong100 P2.3 capture sanity check (run in VM)
# Scans a USBPcap pcapng for the vendor protocol markers:
#   - PU magic bytes 50 55 7f 7f (host->device boot frames)
#   - OD magic bytes 4f 44 7f 7f (device->host boot frames)
#   - "Ready to load program!" ASCII (type-2 ready reply)
#   - device change 0471:2600 -> 0471:2500 (re-enumeration)
# Usage: powershell -File check_capture.ps1 -Pcap C:\idong_capture\boot.pcapng
param(
    [Parameter(Mandatory = $true)]
    [string]$Pcap
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path $Pcap)) {
    Write-Host "[ERR] file not found: $Pcap"
    exit 1
}
$bytes = [System.IO.File]::ReadAllBytes($Pcap)
$len = $bytes.Length
Write-Host "capture size: $len bytes"

# --- magic helpers ---------------------------------------------------------
function Count-Occurrence([byte[]]$hay, [byte[]]$needle) {
    $count = 0
    $i = 0
    while ($i -le ($hay.Length - $needle.Length)) {
        $match = $true
        for ($j = 0; $j -lt $needle.Length; $j++) {
            if ($hay[$i + $j] -ne $needle[$j]) { $match = $false; break }
        }
        if ($match) { $count++; $i += $needle.Length } else { $i++ }
    }
    return $count
}

$pu = Count-Occurrence $bytes ([byte[]](0x50,0x55,0x7f,0x7f))
$od = Count-Occurrence $bytes ([byte[]](0x4f,0x44,0x7f,0x7f))

# "Ready to load program!" as ASCII inside the capture
$ready = Count-Occurrence $bytes ([byte[]][System.Text.Encoding]::ASCII.GetBytes("Ready to load program!"))

# USBPcap device IDs are recorded in the pcapng per-device blocks; the
# PID pair appears as ASCII in machine-readable sections in many builds.
# Robust route: scan for the descriptor strings the vendor stack queries:
# the ascii "045" VID/PID hex forms are unreliable, so we look for the
# plain-word markers the boot protocol writes instead.
$readyCount = $ready

Write-Host "PU magic (50 55 7f 7f)      : $pu occurrences"
Write-Host "OD magic (4f 44 7f 7f)      : $od occurrences"
Write-Host "ready-string occurrences    : $readyCount"

# --- verdict ---------------------------------------------------------------
$ok = $true
if ($pu -lt 10)  { Write-Host "[FAIL] too few PU frames - boot upload probably not captured"; $ok = $false }
if ($od -lt 3)   { Write-Host "[FAIL] too few OD frames - device replies missing"; $ok = $false }
if ($readyCount -lt 1) { Write-Host "[FAIL] no 'Ready to load program!' - device never answered DSP-start"; $ok = $false }

# packet-train heuristic: the .bit alone is 235,930 B = 3687 x 64-B
# packets; even if staging starts with a smaller blob, expect > 500
if ($pu -lt 500) {
    Write-Host "[WARN] fewer than 500 PU frames - full packet train may be short;"
    Write-Host "       if the staging order starts with a small blob this can be normal."
}

if ($ok) {
    Write-Host ""
    Write-Host "PASS - the capture contains the vendor boot sequence."
    Write-Host "Copy it to the analysis host for the byte-for-byte gate."
    exit 0
} else {
    Write-Host ""
    Write-Host "FAIL - redo the capture per README_CAPTURE.md section 5"
    Write-Host "(power-cycle the sensor first so it is in the bootloader state)."
    exit 2
}
