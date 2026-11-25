# Chat context backup - turn 24 - 2026-11-25

Project: idong100 (Toltech / i-dong "idong Sensor V2.6", USB 0471:2600 boot loader,
0471:2500 running). Phase 2 item P2.3: USBPcap capture kit for Windows.
Repository: https://github.com/thomas7475/Idong100.git (branches `main`,
`project/state`, `chat/context`). Working clone: `research/idong100_restore`.

## User message of turn 24

"https://github.com/thomas7475/Idong100.git <PAT> i use a win7 vm the software is
installed and the camera gets detected and the dirver seams to work, the capture
is there a way to reset the camer to usb id 2600 to capture the upload,
check_capture does nothing it closes immediately, is there a way to test if
USBPcap is working as intended, where is the captured file stored."
Uploads: `win7-afterdriver.install.png`, chat context backups of turns 18, 19 and
23, phase 2 zips of turns 17 and 23.

Standing rules: English only; zip backup of generated files in `download/`; chat
context backup every turn; never run full cargo builds; PAT is re-supplied by the
user each turn and is never printed or stored; no PowerShell in deliverables,
pure cmd `.bat`; scripts stay open after they run.

## Answers

1. Reset to 2600: no command needed. 2600 is the state after every power-up
   (boot loader of the 8051). The vendor program downloads the MCU program
   `app_m8051_2611.hex` over 2600, the sensor comes back as 2500 and gets the
   DSP and FPGA upload. Unplug and plug in again while USBPcap records and the
   vendor program runs. In the VM: disconnect / connect in the USB menu. The
   vendor DLL has an `McuReset` command, the kit never sends it.
2. `check_capture.bat`: rewritten. No argument needed (reads
   `C:\idong_capture`, newest `runN`), parses USBPcap packets, prints Sensor /
   MCU / DSP / Banner / Verdict, always pauses. The turn-23 version needed a
   path, had no pause, and looked for a `boot.pcapng` that was never written.
3. Test USBPcap: new `test_usbpcap.bat` (finds USBPcapCMD, version, driver
   service, hub class filter, root hubs, sensor state, 20 s recording on every
   root hub, packet count, verdict and hints).
4. Files: `C:\idong_capture\run<N>\USBPcap<K>.pcap` plus `session.txt`,
   `sensor_states.txt`, `check_report.txt`. Test files:
   `C:\idong_capture\usbpcap_test\test<K>.pcap`.
5. Screenshot dialog: "您购买的是爱动体感运动机(大众版)..." is the vendor's
   upgrade advertisement (standard edition, hotline 400-1818-520), not an error.

## Files of this turn

Repo `research/idong100_restore`:
- `tools/capture/check_capture.bat` (hybrid cmd + ES3 JScript, batch part fixed)
- `tools/capture/test_usbpcap.bat` (new)
- `tools/capture/run_capture.bat` (rewritten)
- `tools/capture/check_sensor_attached.bat` (rewritten)
- `tools/capture/install_drivers_manual.bat` (Windows 7 pnputil syntax)
- `tools/capture/check_gate.bat` (two text fixes)
- `tools/capture/MANIFEST.txt` (regenerated), `.gitattributes` (new, CRLF)
- `docs/CAPTURE_KIT.md` (rewritten for Windows 7), `docs/SOLVE_DIALOG.md` and
  `docs/PROTOCOL.md` (turn 24 sections)
- all `.bat` and `.reg` are ASCII with CRLF line endings

Scripts `scripts/`: `lint_bat.py`, `crlf.py`, `make_test_pcap.py`,
`test_check_capture.js`, `build_turn24.py`, `pe_imports.py`, `pe_strings.py`,
`ws_find.py`, `turn24_*.md` (doc sources).

Deliverables in `download/`:
- `idong100_p2_usbpcap_kit.zip` and folder `idong100_p2_usbpcap_kit/`
  (PowerShell files removed, `Wireshark-win64-4.6.10.exe` replaced by
  `Wireshark-win64-3.2.18.exe`, SHA-256
  cbee6bb8d250287d8bcc41dc0353c425d87dceffa5005b3d9ab755090709e7b9)
- `idong100_phase2_turn24_2026-11-25.zip`
- `chat_context_backup_2026-11-25_turn24.md` (this file)

## Verified / not verified

- USBPcap 1.5.4.0 behaviour read in the source (`USBPcapCMD/cmd.c`).
- Scanner of `check_capture.bat`: Node harness with synthetic captures,
  ALL CHECKS PASSED, also after the CRLF conversion.
- Batch files: static lint only. There is no Windows in the sandbox, the
  scripts were never executed and the capture recipe was not tried on hardware.
- Wireshark 3.2.18: last 3.2 version in the official version index (2021-11-17);
  downloaded over HTTPS, checksum not compared with a second source.

## Open

- Git push of `main`, `project/state`, `chat/context`: needs the PAT, which was
  not available in the context after the context reset. Local commits exist.
- A real capture on the Windows 7 VM: run `test_usbpcap.bat`, then
  `run_capture.bat`, send `C:\idong_capture\run<N>`.
