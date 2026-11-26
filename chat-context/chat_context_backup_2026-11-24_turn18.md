# ZCode Chat Context Backup — 2026-11-24 (turn 18)

**How to use this file:** attach/upload it in a future session so the
assistant can restore full context of this conversation and continue
where it left off. Written by the assistant itself as a session snapshot.

---

## User's standing rules (apply to EVERY reply)

1. **English only.**
2. **Zip backup of generated files** in `/home/z/my-project/download/`.
3. **Chat-context backup** (this kind of file) every time, in
   `/home/z/my-project/download/`.
4. **NEVER run full `cargo` builds** — RAM limit. `cargo check`,
   single bin/example builds, or lowered parallelism only.
5. **Git state tracking:** https://github.com/thomas7475/Idong100.git,
   branches `project/state` and `chat/context` — updated at end of
   every turn. PAT re-supplied by user each turn (embedded in this
   turn's first message; do not print it).
6. **NEW (turn 18): NO PowerShell in deliverables** — the user's VM
   environment hit a compatibility problem with the .ps1 scripts
   (ran the installer with Vista + Win7 compatibility mode, nothing
   changed). Deliverables must be pure cmd `.bat` from now on.

## Turn 18 request and outcome

User: "the software hits a compatibility problem i did run it with
compatibility for vista and win7 nothing changed, rewrite the
powershell script to a cmd .bat". Uploads: backups + PAT + repo URL
+ `idong-xeyez-setup.png` (screenshot of the same gate dialog).

**Root cause of "nothing changed" found (turn 18) — the turn-17 fix
was incomplete, NOT a compatibility problem:**

- The gate DLL (`VersionCheck.dll`) is **PE32 / 32-bit** (machine
  0x014c — re-verified by pefile this turn). A 32-bit process
  opening `HKLM\SOFTWARE\...` on 64-bit Windows is silently
  WOW64-redirected to `HKLM\SOFTWARE\Wow6432Node\...`.
- Turn-17 `platform_mark.reg` wrote only the 64-bit view → the
  32-bit gate never saw the mark → identical dialog again.
- Compatibility mode (Vista/Win7) can NEVER fix this: it does not
  change which registry view a 32-bit process sees.
- Gate re-verified end-to-end: VersionCheck.dll imports
  RegOpenKeyExA/RegQueryValueExA/RegOpenKeyA (advapi32), reads
  `SOFTWARE\Toltech\xeyez_engine` + `RootPath`, `%CommonAppData%\i-dong`
  (`%s\i-dong`, `Common AppData` via Shell Folders), probes
  `\\.\vMouse`/`\\.\vKeyboard` (CreateFileA). Launcher NSIS script
  (re-extracted with nsis_firstblock_universal.py): `Software\TOLTech`
  `Location` + GBK gate string @0x12d0.
- Inner installer (`xeyez_alone.dat` script): its `Software\TOLTech\
  xeyez_engine` refs are version/uninstall bookkeeping (`2.6.0.6`,
  `TrayMenu`, `xeyez_engine_version`), NOT a platform gate — the
  gate message string is ABSENT from the inner script. Only the
  launcher gates.

**Fix delivered (turn 18):**

1. `platform_mark.reg` **v2**: writes BOTH registry views (64-bit +
   `Wow6432Node`), all three key spellings (TOLTech, TOLTech\
   xeyez_engine, Toltech\xeyez_engine). On 32-bit Windows the
   Wow6432Node lines are inert dead keys. Cleanup commands included
   in comments.
2. `check_gate.bat`: pure cmd rewrite of check_gate.ps1. reg query
   + sc query + exist checks; reports BOTH views (so the user can
   SEE which one is missing), creates `%ProgramData%\i-dong` (the
   folder probe), checks the `xeyez` service, verdict line follows
   the OS-bitness view that the 32-bit gate actually reads.
3. `check_capture.bat`: pure cmd rewrite of check_capture.ps1.
   Byte-accurate counting via an embedded VBScript run with cscript
   (standard Windows since 98 — NOT PowerShell) written to %TEMP% at
   runtime; same PU/OD/Ready magic checks, same exit codes (0 pass /
   2 fail).
4. `run_capture.bat`: step [4/4] now calls `check_capture.bat`.
5. Old `check_gate.ps1` / `check_capture.ps1` RETIRED (deleted from
   repo + kit).
6. `SOLVE_DIALOG.md` updated: turn-18 root-cause section ("The
   64-bit registry-view trap"), the both-views table, updated
   Solution A (reg import + md i-dong), .bat kit table.
7. `CAPTURE_KIT.md` / kit `README_CAPTURE.md`: turn-18 header note
   (32-bit DLL → Wow6432Node redirect; why compatibility mode is a
   dead end), .bat file references.

**Deliverables (download/):** `idong100_p2_usbpcap_kit/` updated
(9 doc/script files + software/), `idong100_p2_usbpcap_kit.zip`
(142 MB, 16 files), `idong100_phase2_turn18_2026-11-24.zip` (the 9
doc/script files), `chat_context_backup_2026-11-24_turn18.md`.

**What the user should do now (in the Windows VM):**

1. As admin: `reg import platform_mark.reg` (the NEW one — both
   views). If the turn-17 v1 was imported before, importing v2 is
   enough (it overwrites the same values and adds the 32-bit view).
2. Run `check_gate.bat` (as admin) — the verdict line must say
   "launcher gate SATISFIED" (on 64-bit it checks the Wow6432Node
   view specifically).
3. Re-run `xeyez_setup.exe` — gate passes → inner installer runs.
   If it ever still refuses: `xeyez_alone.dat` renamed to `.exe`
   and run directly (Solution B).
4. Then continue README_CAPTURE.md §3 onward (capture session).

## Operational incident this turn (IMPORTANT for future turns)

**Sandbox recycle had eaten the nested repo clone:** `research/
idong100` had NO `.git` dir this turn. A `git reset --hard origin/
main` executed inside it therefore hit the OUTER workspace repo
(/home/z/my-project/.git), checked Idong100 files into the workspace
root and DELETED the outer repo's tracked files (131 of 132,
incl. .env, analysis/ tracked trees). RECOVERED by `git reset --hard
abdf07e` (the outer Initial commit) — zero data loss, all files
restored (verified by ls-tree walk). The repo was then RE-CLONED
fresh from GitHub into research/idong100 with a proper nested .git.
**RULE going forward: ALWAYS verify `git rev-parse --git-dir` /
`--show-toplevel` before ANY git command inside research/idong100;
if the clone is missing, re-clone FIRST, never reset the outer repo.**

## Repo state (turn 18)

- main: turn-18 commit (platform_mark.reg v2 + check_gate.bat +
  check_capture.bat + run_capture.bat patch + docs) on top of 4a4862f.
- project/state: `state(turn 18): ...` merge-sync commit on top of d1fae9b.
- chat/context: `context(turn 18): ...` commit on top of a740294,
  carrying chat-context/chat_context_backup_2026-11-24_turn18.md.
- (Exact hashes in the push output of this turn's session.)

## Technical recipes (cumulative)

1. **NSIS LZMA first-block extraction** (any NSIS-2 installer): find
   clean ASCII `NullsoftInst`; LZMA props at +20; stream =
   props(5B) + `\xff`*8 + payload; `lzma.FORMAT_ALONE`. Script:
   scripts/nsis_firstblock_universal.py.
2. **WOW64 registry-view rule (turn 18):** a PE32 (machine 0x014c)
   EXE/DLL reading HKLM\SOFTWARE\X on 64-bit Windows actually reads
   HKLM\SOFTWARE\Wow6432Node\X. Any registry mark for a legacy
   32-bit installer must be written to BOTH views. pefile
   FILE_HEADER.Machine tells you 0x014c (32-bit) vs 0x8664 (64-bit).
3. **Pure-cmd byte scanner pattern:** certutil -encodehex is lossy
   for straddling patterns; instead embed a VBScript into %TEMP% via
   `>` echo lines with escaped special chars (^& ^< ^> ^( ^)) and run
   with `cscript //nologo`; ADODB.Stream binary read gives byte-
   exact counting. cscript/ADODB exist on every Windows since 98.
4. inno_bin_extract.py (user's tool, upload/) — Inno Setup 5.3.6
   header parser.
5. VersionCheck.dll probe set (disasm): CreateFileA on `\\.\vMouse`/
   `\\.\vKeyboard`, RegOpenKeyExA `Software\TOLTech`+`Toltech`,
   `Shell Folders` (Common AppData) + `%s\i-dong`,
   SetupQueryInfVersionInformationA; exports incl. VerifyInstallSystem,
   CheckVirtualVerSion, IsVirtualMouseInstalled/IsVirtualKeyBoardInstalled.

## Facts verified (cumulative)

- x64 drivers exist in payload: `DriverInstall\Win7\xeyezDriver64\
  xeyez.sys` + `virtualDriver64\vMouse.sys/vKeyboard.sys` (PE32+).
- INF hardware IDs: `USB\VID_0471&PID_2600`, `PID_2500&MI_04`,
  `PID_2100`, `PID_0668`, `PID_0666` (sensor generations V1.0–V2.6).
- Installer writes `SOFTWARE\Toltech\xeyez_engine` (RootPath), runs
  `xeyezMonitor.exe` tray via `xEyezSvStartXeyez.exe`, service
  `xEyeService.exe`; uninst via `idong感应器` key + shortcuts under
  `泰山在线\idong感应器.lnk`.
- Sensor re-enumerates 0471:2600 (bootloader) → 0471:2500 (runtime
  composite, 4 interfaces incl. UVC).

## Where the project stands

- Phase 1 (pure core) COMPLETE on main (2a1e713).
- Phase 2: P2.3 capture kit (turn 16) + gate fix v2 (turn 18)
  delivered. P2.1 read-only probe done (usb.rs, turn 15); P2.2
  (udev rule) still open.
- NEXT: user imports the v2 platform_mark.reg in the VM, runs
  check_gate.bat (expect "launcher gate SATISFIED"), re-runs
  xeyez_setup.exe → completes README_CAPTURE.md §3 → capture session
  (§5) → boot.pcapng → host-side byte-for-byte gate comparison.

## Environment notes (persistent facts)

- **Sandbox recycled between turns:** the /tmp trees were lost again
  (scripts regenerated this turn); research/idong100 was re-cloned
  (see incident above). The GitHub repo is the durable home.
- pefile + capstone need `pip install` after each recycle (done
  this turn).
- 7zz (if needed): re-download 7z2501-linux-x64.tar.xz from
  7-zip.org into /tmp/7zip.
- Rust stable 1.100.0 in ~/.cargo (export PATH="$HOME/.cargo/bin:$PATH").
- Uploads live in /home/z/my-project/upload/ (read-only intent).
  Working dirs: /home/z/my-project/{analysis,scripts,download,
  research/idong100}.
