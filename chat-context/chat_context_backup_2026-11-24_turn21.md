# ZCode Chat Context Backup — 2026-11-24 (turn 21)

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

## Turn 21 request and outcome

User: the installer now RUNS (the turn-20 NSIS integrity fix worked —
full file extraction logged: DriverInstall tree, Chs tree,
vcredist_x86.exe -silent, "Registre server end!", "Start install
driver!") but then **"the installer starts and tries to install the
driver it errors out"** + screenshot `idong-xeyez-setup4.png` (GBK
mojibake dialog over the installer's progress window).

**Root cause:** the NSIS script calls `xEyeSetupHelper.dll!InstallDriver`.
That function:
- selects the driver directory by checking the OS via `GetVersionExA`:
  XP = major 5.1, Vista = 6.0, Win7 = 6.1 (helpers at RVA 0xa110 /
  0xa190 / 0xa230 in the DLL). On Win8/10/11 an **unmanifested 32-bit
  process is lied to: 6.2/6.3** — all three checks fail =>
  MessageBoxA '驱动目录没有找到!' ('driver directory not found!',
  title 'idong感应器 安装') and the install aborts. THAT is the dialog
  in the screenshot.
- a second gate: the 'pre-install xeyez' helper at RVA 0x2c60
  (`CM_Locate_DevNodeA(&dn, NULL, 0)` + `CM_Reenumerate_DevNode`) which
  fails when **no sensor is plugged in** => '预安装xeyez失败!'.

**Fix (turn 21, scripts/rebuild_patched_installers21.py):** inside the
embedded xEyeSetupHelper.dll in xeyez_alone.dat's solid stream (file
offset == RVA in this DLL; the DLL PE sits at stream offset 0x3f145,
found by scanning MZ headers for the `InstallDriver` export):
- `0xa295: 74 02 -> EB 02` — IsWin7's minor-version je becomes jmp =>
  IsWin7() returns true for ANY OS => the **Win7 / Win7-64 driver
  sets** (the newest in the package, incl. virtualDriver64,
  xeyezDriver64) are chosen on Win10/11.
- `0x2c60: 51 6A 00 6A 00 8D... -> B8 01 00 00 00 C3` — the pre-install
  devnode helper is stubbed to `mov eax,1; ret` (the PnP rescan it
  triggers is not required during install; devices get scanned by the
  standard PnP machinery anyway).
Both installers rebuilt with the turn-19 VersionCheck.dll gate patches
+ these two + the turn-20 NSIS integrity CRC recipe. Verification:
full LZMA round-trip equality, patch bytes re-checked after
re-decompression, CRC stub-check `crc32(file[512..fh+x2-4)) ==
u32@fh+x2-4` passes, and the REAL stub code (Unicorn emulation) passes
the integrity check on the patched exe.

**New deliverables (download/idong100_p2_usbpcap_kit/software/):**
- `xeyez_setup_patched.exe` — 82,838 B —
  sha256 `e82c2f444d2d7b9619b07247323cbd1447ba2865d53d3af587bd5bc1ef0c7ee8`
- `xeyez_alone_patched.dat` — 23,635,851 B —
  sha256 `b16d4a068ba569f2545e87932de94b4fcf69a693866e8d39dd09fb44f207d302`
- Turn-20 versions (82,842 / 23,638,139 B) are superseded — replace.

**User action:** replace both patched files, re-run
`install_patched.bat` (as admin). Expect: no NSIS error, no
'驱动目录没有找到' dialog; install should proceed through driver
install. If Windows refuses the 2008-era driver on Win10/11 x64 by
signature enforcement, use the one-boot 'Disable driver signature
enforcement' route (README_CAPTURE.md section 4) BEFORE running the
install. Then README_CAPTURE.md section 3 (capture session).

## Analysis facts recovered this turn (reuse, don't re-derive)

- The setup.exe stream (31,876 B decompressed) contains ONLY
  VersionCheck.dll (@stream 0x1684); xEyeSetupHelper.dll exists ONLY
  in xeyez_alone.dat's 95,248,820 B stream (PE @0x3f145; its 8 exports:
  InstallDriver 0x9510, UninstallDriver 0x6bc0, UninstallClear 0x6c20,
  UpdateVirtualInputDriver 0x9460, UninstallVirtualInputDriver 0x8830,
  Dll* 0x6f10/0x6fd0/0x7d40/0x7d50).
- InstallDriver's flow: GetVersionEx helpers (0xa110 XP / 0xa190 Vista
  / 0xa230 Win7); on x64 (IsWow64Process true) it prefers
  `Win7\xeyezDriver64`, else `Win7\xeyezDriver`; XP/Vista have own paths;
  then a device-id scan loop (USB\VID_0471&PID_2100 / 2600 / 2500 /
  0666 / 0668 with `SetupDi*-style` calls via helper 0xbfca), the
  pre-install devnode gate (0x2c60), then file copy + inf install;
  retries up to 3 (0x2710 sleeps), 'CheckSpecialDevice bootloader!'
  path for PID_2500&MI_04.
- Vendor USB IDs: USB\VID_0471&PID_2100, PID_2600, PID_2500 (and
  PID_2500&MI_04 composite), PID_0666, PID_0668 — useful later for
  USBPcap device selection / udev rules.
- GBK strings in the DLL decoded: 驱动目录没有找到! (driver dir not
  found), 预安装xeyez失败! (pre-install failed), 扫描失败！(scan
  failed), title idong感应器 安装 (idong sensor install).

## Operational rules (cumulative)

- `git rev-parse --show-toplevel` inside research/idong100 before ANY git op.
- UTF-16LE vs GBK: search PE strings in BOTH encodings.
- NSIS solid repack recipe (turn-20/21, final): find `NullsoftInst` =>
  F = find-8; x2 = u32@F+24; props = F+28..F+33; stream = props +
  `\xff`*8 + file[F+33:]; FORMAT_ALONE decompress; patch; recompress
  (dict 0x800000, lc3 lp0 pb2, MODE_NORMAL nice 128 BT4); payload =
  blob[13:]; new_x2 = F+33+len(payload)-F+4; region = file[512:F+24] +
  pack(new_x2) + props + payload; crc = crc32(region); out = file[:F+24]
  + pack(new_x2) + props + payload + pack(crc) + old_tail(from F+x2);
  zero PE security-dir (e_lfanew+24+96+4*8, 8 bytes).
- Locating embedded PEs in a solid stream: scan `4d5a9000030000`, parse
  each with pefile fast_load + parse export dir, match by export NAME
  (e.g. InstallDriver) — MZ hits are ~145, only one matches.
- Unicorn stub-emulation recipe (for integrity check): see turn-20 notes.
- pefile+capstone+unicorn need `pip install` after each sandbox recycle.
- pefile export iteration: `pe.DIRECTORY_ENTRY_EXPORT.symbols` may be a
  list of symbols directly (not always a list of dirs) — handle both.

## Repo state (turn 21)

- main: faff296 (turn-21 driver-gate fix) on top of 77b3872 (turn 20).
- project/state: 9ba82ea. chat/context: this file (to be pushed).
- Kit rebuilt: download/idong100_p2_usbpcap_kit.zip (22 files, 166 MB).

## Where the project stands

- Phase 1 (pure core) COMPLETE on main.
- Phase 2: P2.3 capture kit + platform gate v2 (t18) + OS-gate patched
  installers (t19) + NSIS CRC fix (t20) + driver-install OS gate fix
  (t21). P2.1 read-only probe done; P2.2 (udev rule) still open.
- NEXT: user re-runs install_patched.bat with the turn-21 files (with
  one-boot DSE if the driver gets refused) -> full vendor install ->
  README_CAPTURE.md section 3/5 capture session -> boot.pcapng ->
  byte-for-byte gate comparison on the host.

## Environment notes (persistent facts)

- analysis/ trees survived (nsis payloads, VersionCheck.dll,
  xEyeSetupHelper_orig.dll now at
  /home/z/my-project/analysis/xEyeSetupHelper_orig.dll) — reuse.
- Uploads in /home/z/my-project/upload/ (read-only intent).
- Rust stable 1.100.0 in ~/.cargo.
