# ZCode Chat Context Backup — 2026-11-24 (turn 16)

**How to use this file:** attach/upload it in a future session so the
assistant can restore full context of this conversation and continue
where it left off. Written by the assistant itself as a session snapshot.

---

## User's standing rules (apply to EVERY reply)

1. **English only.**
2. **Zip backup of generated files** in `/home/z/my-project/download/`.
3. **Chat-context backup** (this kind of file) every time, in
   `/home/z/my-project/download/`.
4. **NEVER run full `cargo` builds** — RAM limit. `cargo check`, single
   bin/example builds, or lowered parallelism only.
5. **Git state tracking:** https://github.com/thomas7475/Idong100.git,
   branches `project/state` and `chat/context` — updated at end of
   every turn. PAT re-supplied by user.

## Turn 16 request and outcome

Q: "continue with Phase 2 for the capture one full vendor boot with
USBPcap on Windows what software do i need i run windows in a vm with
qemu and virtmanager, if possible provide the software to run in a .zip"

**Done — the complete capture kit was delivered as a zip:**

- `download/idong100_p2_usbpcap_kit.zip` (142 MB, 12 files) — contains:
  - `software/USBPcapSetup-1.5.4.0.exe` (desowin, signed;
    sha256 87a7edf9...ccb622, from github.com/desowin/usbpcap release
    1.5.4.0)
  - `software/Wireshark-win64-4.6.10.exe` (current stable x64;
    sha256 c3bd9153...c1e94, from 2.na.dl.wireshark.org)
  - `software/xeyez_setup.exe` + `xeyez_alone.dat` + `xeyez_Lobby.dat`
    — the vendor's own v2.6.0.6 installer set, copied from
    `analysis/remote/dependent/xeyez_pc_v2.6.0.6/` (user's own copy
    from the turn-12 download.i-dong.com extraction)
  - `README_CAPTURE.md` — the full procedure
  - `scripts/run_capture.bat` + `scripts/check_capture.ps1`
  - `MANIFEST.txt` (sha256 of everything)
- Repo on `main`: commit `ff41f8b` added `docs/CAPTURE_KIT.md` (the
  procedure doc) + `tools/capture/{run_capture.bat,check_capture.ps1,
  MANIFEST.txt}`. `project/state` synced (`9702cc0`).

### Key technical facts established this turn

- **USBPcap current version is 1.5.4.0.** Real download link is a
  GitHub release (desowin.org's thankyou.html redirects there):
  https://github.com/desowin/usbpcap/releases/download/1.5.4.0/
  USBPcapSetup-1.5.4.0.exe
- **Wireshark 4.6.10 x64** is current stable; URL pattern
  https://2.na.dl.wireshark.org/win64/Wireshark-4.6.10-x64.exe
- **THE critical VM detail:** the sensor re-enumerates 0471:2600 ->
  0471:2500 during boot, and virt-manager's single hostdev rule (bound
  to one device address) silently DETACHES mid-boot. The fix is TWO
  hostdev rules in the domain XML — one per PID (vendor 0x0471/product
  0x2600 and one with product 0x2500) — QEMU then re-attaches
  automatically at the new identity. Alternative: attach by physical
  bus/port. Spice USB redirection does NOT survive re-enumeration.
- **Install order inside the VM:** Wireshark first, USBPcap second
  (reboot required — the filter driver loads at boot), vendor stack
  third (xeyez_setup.exe + the two .dat files must sit in one folder).
- **Driver signature:** vendor xeyez.sys (2008) may be refused on Win10
  x64 — disable driver signature enforcement once via advanced startup
  (option 7) or `bcdedit /set testsigning on`.
- **Capture command (the one-liner):**
  `C:\Program Files\USBPcap1\USBPcapCMD.exe -d \\.\USBPcap1 -o
  C:\idong_capture\boot.pcapng --snap 0` — root-hub filter so BOTH PIDs
  land in one file across the re-enumeration.
- **Power-cycle note:** the sensor returns to the bootloader state only
  after being unplugged from power; a warm USB replug may keep it in
  runtime state (vendor design).
- **Acceptance criteria for THE GATE (capture must contain):** 0471:2600
  attach + descriptors; PU-magic (50 55 7f 7f) OUT frames (DSP-start,
  count+CRC, the type-3 64-B packet train — count reveals the staging
  order, open item #1); OD-magic (4f 44 7f 7f) IN frames ("Ready to
  load program!" type 2, CRC verdict type 3, device-ID); the
  disconnect/re-connect as 0471:2500 with composite descriptors; runtime
  keep-alives 0xFFF->0xFFE + ~50 fps marker records.

## Where the project stands

- Phase 1 (pure core) COMPLETE on `main` (commit 2a1e713, 35 tests
  green; see turn-15 backup).
- Phase 2 IN PROGRESS: the capture kit is delivered. NEXT STEPS:
  1. User runs the kit in the VM (procedure in README_CAPTURE.md /
     docs/CAPTURE_KIT.md), obtains `boot.pcapng`.
  2. Host side: `tshark -r boot.pcapng -Y usb.capdata` extraction ->
     golden vectors -> byte-for-byte comparison against the Phase-1
     decoder (`FrameDecoder` / `BootUploader` replay) -> resolves open
     items #1 (staging order), #2 (CRC variant), #3 (count/CRC framing).
  3. Flip `BOOT_CONFIRMED = true` in `idong/src/protocol/boot.rs` only
     after the transcript matches to the byte. Then Phase 3 (hardware
     boot via our uploader) is unblocked.
- P2.1 (read-only probe) is already implemented in
  `idong-transport/src/usb.rs` from turn 15; P2.2 (udev rule) still open.

## Environment notes (persistent facts)

- Rust toolchain: rustup **stable 1.100.0** in `~/.cargo` (PATH export
  needed: `export PATH="$HOME/.cargo/bin:$PATH"`).
- 7zz at `/tmp/7zip/7zz` (re-download 7z2408-linux-x64.tar.xz from
  7-zip.org if the sandbox recycled).
- Vendor NSIS tree: `analysis/nsis/xeyez_pc_v2.6.0.6/` (xeyez_Lobby,
  xeyez_alone, full 561-file app payload incl. xeyez.exe, driver
  installers, firmware). Original installer set:
  `analysis/remote/dependent/xeyez_pc_v2.6.0.6/`.
- pefile/capstone need pip install after a sandbox recycle.
- The kit zip was verified: 12 files, 142,457,832 bytes, listing checked.
