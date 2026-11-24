# ZCode Chat Context Backup — 2026-11-24 (turn 17)

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
   every turn. PAT re-supplied by user each turn (it was embedded in
   this turn's first message; do not print it).

## Turn 17 request and outcome

User hit the vendor installer dialog (garbled GBK):
`idong感应器 安装 — 请您先安装爱动平台软键!` ("Please install the
iDong platform software first!"), asked how to solve it. Uploaded:
`idong.png` (screenshot), `idong_setup.exe` + `pingbisetup.exe` +
`AUTORUN.INF` (from the vendor CD), `inno_bin_extract.py` (user's own
Inno extractor).

**Solved — root cause found by byte-level analysis, fix delivered:**

- `idong_setup.exe` = Inno Setup 5.3.6 loader of the full Sports King
  platform (4.58 GB in external .bin slices, app
  `爱动体感运动机(运动王PC版)` v1.9.04, default dir `{pf}\TOLTech`).
  `pingbisetup.exe` = trivial Flash-camera-block util. NOT the issue.
- The DIALOG comes from `xeyez_setup.exe` (the sensor-stack launcher
  in the P2 kit): an 82 KB NSIS-2.44 launcher containing only
  `VersionCheck.dll`. Its NSIS script (decompressed from the LZMA
  first block — recipe below) carries the exact GBK string
  `请您先安装爱动平台软键` @0x12d0.
- Gate flow (script strings + VersionCheck.dll disasm, pefile+capstone):
  `VerifyInstallSystem` → reads `HKLM\SOFTWARE\TOLTech` value
  `Location` (platform install dir), probes `\\.\vMouse` /
  `\\.\vKeyboard` (virtual-HID device files created only by the
  platform's drivers), `%CommonAppData%\i-dong` folder → all absent →
  MessageBox the dialog → Abort. It also lists
  `xeyez_alone.dat xeyez` / `xeyez_Lobby.dat Lobby` (flavors it
  launches after the gate passes).
- The sensor stack itself (xeyez_alone.dat: driver+app+firmware,
  561 files) has NO technical need for the platform — pure 2010
  marketing bundle-order check.

**Fix delivered (Solution A, recommended):** import
`platform_mark.reg` (writes `HKLM\SOFTWARE\TOLTech\Location` +
`SOFTWARE\TOLTech\xeyez_engine\RootPath` — exactly what the gate
reads) → re-run `xeyez_setup.exe`. Alternatives in SOLVE_DIALOG.md:
B) run `xeyez_alone.dat` directly (rename to .exe); C) install the
real 1.5 GB platform (live on download.i-dong.com). In-VM verifier:
`check_gate.ps1`.

**Deliverables (download/):**
- `idong100_p2_usbpcap_kit/` rebuilt: + SOLVE_DIALOG.md,
  platform_mark.reg, check_gate.ps1; README_CAPTURE.md updated with a
  gate-fix pointer at top; MANIFEST.txt updated.
- `idong100_p2_usbpcap_kit.zip` — 142 MB, 15 files.
- `chat_context_backup_2026-11-24_turn17.md` (this file) +
  `idong100_phase2_turn17_2026-11-24.zip`.

**Repo:** main `4a4862f` (docs/SOLVE_DIALOG.md + tools/capture/
{platform_mark.reg, check_gate.ps1}); project/state `d1fae9b`;
chat/context `6fb6c92`.

### Technical recipes established this turn (reusable)

1. **NSIS LZMA first-block extraction** (works on any NSIS-2 installer):
   find clean ASCII `NullsoftInst` in the file (NOT the decoy
   `\xef\xbe\xad\xde` magic inside PE code!); LZMA props start 20
   bytes later; stream = props(5B) + `\xff`*8 (fake size) + payload,
   decompress with `lzma.FORMAT_ALONE`. Script: scripts/
   nsis_firstblock_universal.py. Saved decompressed scripts to
   /tmp/*.script.bin (not persisted — regenerate if needed).
2. 7-zip's `uninst.exe.nsis` output is itself an LZMA block with the
   same structure (magic at offset 4, props at +28).
3. `inno_bin_extract.py` (user's tool, upload/) — Inno Setup 5.3.6
   header parser; used to ID idong_setup.exe/pingbisetup.exe.
4. VersionCheck.dll probe set (disasm): CreateFileA on
   `\\.\vMouse`/`\\.\vKeyboard`, RegOpenKeyExA `Software\TOLTech`,
   `Software\Microsoft\Windows\CurrentVersion\Explorer\Shell Folders`
   (Common AppData) + `%s\i-dong`, SetupQueryInfVersionInformationA
   for INF version compares, `IsCalledByAnother` export.

### Facts verified this turn

- x64 drivers exist in payload: `DriverInstall\Win7\xeyezDriver64\
  xeyez.sys` + `virtualDriver64\vMouse.sys/vKeyboard.sys` (all PE32+).
- INF hardware IDs: `USB\VID_0471&PID_2600`, `PID_2500&MI_04`,
  `PID_2100`, `PID_0668`, `PID_0666` (sensor generations V1.0–V2.6).
  Virtual-HID INFs use Vid_0471&Pid_0666/0668.
- Installer writes `SOFTWARE\Toltech\xeyez_engine` (RootPath), runs
  `xeyezMonitor.exe` tray via `xEyezSvStartXeyez.exe`, service
  `xEyeService.exe`; uninst via `idong感应器` key + shortcuts under
  `泰山在线\idong感应器.lnk` ("Taishan Online" = vendor brand).
- Driver INF: Class=USB, kernel service `XEYEZ`, demand-start,
  `xeyez.sys` → `System32\Drivers`, coinstaller `xeyezCoIns.dll`
  (SetupCopyOEMInfA). Catalog `xeyez.cat` referenced but NOT shipped
  (unsigned → expect the signature-enforcement caveat, README §4).

## Where the project stands

- Phase 1 (pure core) COMPLETE on main (2a1e713).
- Phase 2: P2.3 capture kit delivered turn 16; gate fix delivered
  turn 17. NEXT: user imports platform_mark.reg in the VM, runs
  xeyez_setup.exe (or xeyez_alone.dat direct), completes §3 of
  README_CAPTURE.md, then the capture session (§5) → boot.pcapng →
  host-side byte-for-byte gate comparison (Phase-1 decoder replay).
- P2.1 read-only probe: implemented turn 15 (idong-transport/src/usb.rs).
  P2.2 (udev rule) still open.

## Environment notes (persistent facts)

- **Sandbox recycled between turns 16→17**: lost /tmp/7zip, analysis/
  nsis extracted trees were intact but the idong100-repo work tree was
  re-cloned from GitHub. The repo is the durable home for all state.
- 7zz: re-download `https://www.7-zip.org/a/7z2501-linux-x64.tar.xz`
  (7z2408 is 404 now) → tar -xf into /tmp/7zip.
- Rust stable 1.100.0 in ~/.cargo (export PATH="$HOME/.cargo/bin:$PATH").
- pefile + capstone need `pip install` after recycle (they were
  reinstalled this turn).
- Uploads live in /home/z/my-project/upload/ (user's files, read-only
  intent). Working dirs: /home/z/my-project/{analysis,scripts,download}.
