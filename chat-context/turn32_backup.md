# Chat context backup - turn 32 - 2026-11-26

Project: idong100 (Toltech / i-dong "idong Sensor V2.6.0.6", USB 0471:2600
boot -> 0471:2500 running). Repo:
https://github.com/thomas7475/Idong100.git (main 7a320b8 after turn 32,
project/state 58b85aa, chat/context b86c88f). Phase 2: USBPcap capture
kit + Rust driver.

## User message of turn 32

Link to the repo + a fresh PAT + "first read the latest project state
from github i made a view more usb captures analyze them then continue
with the next step of the driver". Uploads: idong_capture2.zip,
idong_capture3.zip, idong_capture4.zip (kit v31 sessions, 120/120/300 s).

Standing rules: English only; zips + chat backup in download/ every turn;
PAT never printed or stored; pure cmd .bat, no PowerShell.

## What turn 32 did

1. Read the latest repo state (project/state 08c8495 = turn 31).
2. Analyzed the three captures with a fresh USBPcap parser
   (scripts/usbpcap_scan32.py + stream32/analyze32/dump32).
3. Decoded the COMPLETE observed wire protocol (see
   research/idong100/agents/14-captures32/FINDINGS.md, docs/PROTOCOL.md
   "Turn 32"): frame `7F 55 <cmd u16 LE> <flags> <00> <body>`, raw 64-B
   records for download/echo/EEPROM; boot on 2600 = 0x0008 "2600" /
   0x0101 begin / 335 blocks / 0x0201 verify + full echo / 0x0301 key
   FE2AC9BBAE11D5C2 / 0x0401 launch -> re-enum 2500; session on 2500 =
   0x0008 "2619" / 0x0107 EEPROM reads (calibration 121 B, "000T",
   "2613") / 0x0104+0x0204 CMOS tables / 0x0103+0x0203 chunks / 0x0303 /
   polls. Zero PU/OD frames; NO DSP/FPGA upload ever on the wire.
   Turn-28 IDs were BE readings of the same commands.
4. Identified the downloaded image byte-exactly: app_m8051_2611.hex
   flattened [0x8000,0xD3A3), gap bytes 0x00 = 21,411 B = 0x53A3, SHA-256
   5da1565c6704d387081844e712b4be4d521083e21a619df7d6cb11551c2aba9c
   (NSIS solid LZMA extraction of xeyez_Lobby.dat reproduced here, hex
   matched the turn-12 manifest, flatten rule matched the capture).
5. Driver step: NEW idong/src/protocol/runtime.rs (RtFrame codec,
   RuntimeSession: get_version / flash_sequence (gated) / read_eeprom
   with checksum / init_cmos with the observed tables / polls),
   FirmwareSet::mcu_image() (hex flatten + hash pin), golden replay
   tests/runtime_replay.rs, example mcu_image.rs. 48 tests green;
   mcu_image() validated byte-exact against the real hex + capture.
   Rust toolchain installed in-sandbox (1.100.0) to make this real.
6. Fixed the capture kit: check_capture.bat had a broken ADODB/VBArray
   binary reader (every check_report.txt said "could not read the first
   24 bytes" - 12/12 files). Replaced with an iso-8859-1 text-stream
   reader (no VBArray); gate updated to the turn-32 reality (2500
   session with download+echo = PASS); counts 0x0008 version replies
   "2600"/"2619". Gate logic simulated in Python: all three captures
   PASS.

## The one open thing for the user

All three vendor sessions are complete and healthy (flash, calibration,
register init, polls, 27 cycles in cap4) - but the app NEVER entered a
camera/marker streaming state (nothing on ep 0x85/0x84, no marker
records). Next capture: get the vendor app to a VISIBLE live camera
image first (exercise/game flow or the tray menu), THEN run a short
USBPcap capture (~30 s) while the image is on screen. That session
should finally contain the frame/marker stream.

## Key files

- repo: idong/src/protocol/runtime.rs, idong/tests/runtime_replay.rs,
  idong/src/lib.rs (mcu_image), tools/capture/check_capture.bat,
  docs/PROTOCOL.md (Turn 32), research/idong100/agents/14-captures32/
- download/: idong100_turn32_analysis.zip (capture analysis outputs),
  idong100_capture_kit32.zip (kit with the fixed check_capture.bat),
  this backup.
- scripts/: usbpcap_scan32.py, usbpcap_stream32.py, usbpcap_analyze32.py,
  usbpcap_dump32.py, patch_check_capture32.py, check_gate32_sim.py.
