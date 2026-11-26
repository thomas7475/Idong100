# ZCode Chat Context Backup — 2026-11-24 (turn 19)

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
6. **(turn 18) NO PowerShell in deliverables** — pure cmd `.bat` only.

## Turn 19 request and outcome

User: the German `")" kann syntaktisch an dieser Stelle nicht verarbeitet
werden.` error from check_gate.bat, "i still get the windows version
incompatibility message even if run with compatibility mode, change the
scripts to stay open after they are run", + PAT + repo URL + new
screenshot `idong-xeyez-setup2.png`.

**Reading of the screenshot (VLM):** `对不起，idong感应器软件与您的
操作系统不兼容！（idong感应器软件可以运行在Windows XP，Windows7和
32位Vista系统上。）` — "Sorry, the idong sensor software is incompatible
with your OS! (It can run on Windows XP, Windows 7 and 32-bit Vista.)"
→ the turn-18 platform gate PASSED; this is a SECOND, OS-version gate.

**Root cause (decoded from disasm of `VerifyInstallSystem` RVA 0x20e4 in
`VersionCheck.dll`, PE32, byte-identical copies in BOTH installers):**
- `GetVersionExA` fills the version struct; on CRT path it reads
  `_winmajor`/`_winminor` — **export variables of MSVCR80.dll**.
- Whitelist: `(5,1)` XP pass; `(6,0)|(6,1)` Vista/Win7 pass and 6.1 sets
  a "Win7" flag; everything else → MFC MessageBox (the dialog) +
  `exit(0)` at 0x21ac..0x21d6.
- WOW64 re-check: `IsWow64Process` && !Win7 → same dialog. On 64-bit
  Windows the 32-bit DLL is always WOW64 → every non-Win7 x64 OS fails.
- **Why compatibility mode does nothing:** the gate compares the CRT
  snapshot variables, not `GetVersionEx`'s direct output; the compat
  layer's version lie never reaches the comparison. Only changing the
  numbers or the comparison itself helps.

**Fix delivered (turn 19):**

1. **Patched installers** (built by
   `scripts/rebuild_patched_installers19.py`):
   - `software/xeyez_setup_patched.exe` (76,316 B; DLL @ stream 0x1684)
   - `software/xeyez_alone_patched.dat` (23,631,617 B; DLL @ 0x91431)
   - Solid NSIS 2.44 archives: firstheader @0xfa04/0xf804, LZMA props
     `5d 00 00 80 00`, whole stream decompressed → 5 bytes patched in
     the embedded `VersionCheck.dll` → recompressed (identical filters)
     → round-trip verified byte-exact. Stale Authenticode security-dir
     entry zeroed (the vendor signature cannot match the modification;
     Windows may warn "unknown publisher" → Run anyway; NSIS never
     verifies it).
   - Patch bytes (offsets within the DLL file):
     `0x1571: 75→EB, 0x1579: 75→EB, 0x1581: 75→EB, 0x15a4-5: 74 06→EB 00`
     → every control-flow path returns 0 (same as native XP), never
     reaches the dialog + exit(0).
2. `install_patched.bat` (new, pure cmd): backs up originals
   (`xeyez_setup.original.exe`, `xeyez_alone.original.dat`), copies the
   patched files over `xeyez_setup.exe` / `xeyez_alone.dat`, launches
   the installer. Ends with pause.
3. `check_gate.bat` rewritten: the German `")"` crash was probe labels
   containing parentheses inside if-blocks → labels now use `-`, value
   display fixed (`tokens=1,2,*` + `%%C`), added OS-gate note +
   patched-installer presence checks, ends with pause.
4. `check_capture.bat` / `run_capture.bat`: pause added at every exit;
   a parenthesised echo inside an if-block fixed (`run_capture.bat`13).
5. Docs: `SOLVE_DIALOG.md` turn-19 section (full disasm + patch table),
   `CAPTURE_KIT.md` turn-19 note, `MANIFEST.txt` turn-19 header +
   SHA-256 of both patched installers
   (setup_patched `5957fc7b...81af565`, alone_patched `d11598c6...3f72de`).

**Deliverables (download/):** `idong100_p2_usbpcap_kit/` (10 script/doc
files + software/ with 2 NEW patched installers), `idong100_p2_usbpcap_
kit.zip` (22 files, 166 MB), `idong100_phase2_turn19_2026-11-24.zip`,
`chat_context_backup_2026-11-24_turn19.md`.

**What the user should do now (in the Windows VM):**

1. Copy the NEW kit (or at least `xeyez_setup_patched.exe`,
   `xeyez_alone_patched.dat`, `install_patched.bat`) into
   `C:\Users\t\Desktop\idong100_p2_usbpcap_kit`.
2. Right-click `install_patched.bat` → Run as administrator. It backs
   up the originals, swaps in the patched ones, launches the installer.
3. Expect NO OS-incompatibility dialog now. Windows may warn the exe
   is unsigned ("unknown publisher") → More info → Run anyway.
4. If the 2008-era driver is refused on Win10/11 x64: one-boot
   "Disable driver signature enforcement" (README_CAPTURE.md §4).
5. Then continue README_CAPTURE.md §3 (attach sensor, install) → §5
   (capture session) → `boot.pcapng` → byte-for-byte gate.

## Operational rules (cumulative)

- ALWAYS verify `git rev-parse --show-toplevel` inside
  research/idong100 before ANY git command (turn-18 incident).
- Turn-19 lesson: **UTF-16LE strings in PE resources** — Chinese dialog
  text was UTF-16LE in the DLL's dialog resource, invisible to GBK
  searches. Search `f.encode('utf-16-le')` AND `f.encode('gbk')`.
- NSIS solid repack recipe: find `NullsoftInst` → firstheader at
  find-4; props at firstheader+24; stream = props+`\xff`*8+rest;
  FORMAT_ALONE chunked decompress; patch; recompress with identical
  filters; payload = blob[13:] (strip alone header); rebuild
  dat[:firstheader+24]+props+payload; zero PE security dir
  (e_lfanew+24+96+32) if Authenticode present; round-trip verify.

## Repo state (turn 19)

- main: 27b5d55 (turn-19 gate fix) on top of 91b76fd.
- project/state: 9df1e88 merge-sync.
- chat/context: this file.

## Where the project stands

- Phase 1 (pure core) COMPLETE on main (2a1e713).
- Phase 2: P2.3 capture kit (turn 16) + platform-gate v2 (turn 18) +
  **OS-gate patched installers (turn 19)** delivered. P2.1 read-only
  probe done; P2.2 (udev rule) still open.
- NEXT: user runs `install_patched.bat` in the VM → install completes →
  README_CAPTURE.md §3/§5 capture session → `boot.pcapng` →
  host-side byte-for-byte gate comparison.

## Environment notes (persistent facts)

- Sandbox recycled between turns: /tmp trees lost; research/idong100
  re-cloned anonymously (public repo) — pushes need the PAT.
- pefile + capstone need `pip install` after each recycle (done turn 19).
- Uploads in /home/z/my-project/upload/ (read-only intent).
- Rust stable 1.100.0 in ~/.cargo (export PATH="$HOME/.cargo/bin:$PATH").
- analysis/ trees (nsis extracted payloads, reports) SURVIVED turn 19
  and were reused — do not re-extract if present.
