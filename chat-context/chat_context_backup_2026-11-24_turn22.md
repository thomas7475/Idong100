# Zcode Chat Context Backup — 2026-11-24 (turn 22)

**How to use this file:** attach/upload it in a future session so the
assistant can restore full context of this conversation and continue
where it left off. Written by the assistant itself as a session snapshot.

---

## User's standing rules (apply to EVERY reply)

1. **English only.**
2. **Zip backup of generated files** in `/home/z/my-project/download/`.
3. **Chat-context backup** (this kind of file) every turn, in
   `/home/z/my-project/download/`.
4. **NEVER run full `cargo` builds** — RAM limit. `cargo check`,
   single bin/example builds, or lowered parallelism only.
5. **Git state tracking:** https://github.com/thomas7475/Idong100.git,
   branches `project/state` and `chat/context` — updated at end of
   every turn. PAT re-supplied by user each turn; never print it.
6. **(turn 18) NO PowerShell in deliverables** — pure cmd `.bat` only.

## Turn 22 request and outcome

User: the installer still fails after "Start install driver!" — same
extraction log as turn 21, GBK error dialog (screenshot
`idong-xeyez-setup4.png`).

**Root cause (disassembly):** `xEyeSetupHelper.dll!InstallDriver`'s
FIRST sub-step calls `xEyeDriverCheck.dll!PreinstallSpecialDriver`
(through the IAT thunk at RVA 0xad32). That helper:
- copies `Win7\xeyezDriver\xeyez.inf` + `xeyezCoIns.dll` into the
  system dir (`CopyFileA`),
- writes registry values under `SYSTEM\CurrentControlSet\Control\
CoDeviceInstallers` + the legacy `Driver Signing` policy keys
(`RegOpenKeyExA`/`RegSetValueExA`),
- runs `SetupOpenInfFileA` + `SetupCopyOEMInfA` on the 2008-era inf.
On Win10/11 x64 it fails and InstallDriver aborts with the box
`预安装xeyez失败!` ("pre-install xeyez FAILED", title `idong感应器 安装`)
— the message in the screenshot (its text contains 'xeyez' in ASCII
inside the mojibake, matching this string and NOT the turn-21
`驱动目录没有找到` one).

**Fix (turn 22, scripts/rebuild_patched_installers22.py):** stub the
thunk at 0xad32 to `31 C0 C3` (`xor eax,eax; ret` = success) inside
the embedded xEyeSetupHelper.dll in xeyez_alone.dat's solid stream.
All prior patches retained (t19 VersionCheck.dll gate, t21 IsWin7
any-OS @0xa295 + devnode helper stub @0x2c60). NSIS integrity CRC
recomputed; real-stub emulation integrity check passes.

**Also fixed (meta-bug):** the turn-22 rebuild script initially had a
bytearray slice-assignment bug: `out[hh+off:hh+len(newb)] = newb`
INSERTS bytes (the slice end must be `hh+off+len(newb)`) — it silently
grew the stream by 10 bytes (95,248,820 -> 95,248,830) and moved every
later byte, breaking earlier patches. Fixed; the correct output has
stream length 95,248,820 EXACTLY like the original.

**New deliverables (download/idong100_p2_usbpcap_kit/software/):**
- `xeyez_setup_patched.exe` — 82,838 B — unchanged from turn 21 —
  sha256 `e82c2f444d2d7b9619b07247323cbd1447ba2865d53d3af587bd5bc1ef0c7ee8`
- `xeyez_alone_patched.dat` — **23,637,085 B** —
  sha256 `845a9170eee16dde87a59cf656c9c1594368975112f6218b200301f78fa16368`
- New `install_drivers_manual.bat` (kit root + scripts/): pnputil
  `/add-driver ... /install /subdirs` fallback for manual driver
  install, with one-boot DSE instructions. Turn-21 .dat (23,635,851 B)
  is superseded — replace BOTH files.

**User action:** replace both patched files, re-run
`install_patched.bat` as admin. If the vendor flow still balks, run
`install_drivers_manual.bat`. If the driver is refused by signature
enforcement, use the one-boot 'Disable driver signature enforcement'
route (README_CAPTURE.md §4) and re-run.

## Analysis facts recovered this turn (reuse, don't re-derive)

- The REAL `DriverX64Setup.exe` is a 64-bit PE at stream offset
  **0x64a7e1** (PDB path `f:\xeyezv2.1\Source\Output\DriverX64Setup.pdb`;
  imports newdev.dll!UpdateDriverForPlugAndPlayDevicesW + GetVersionExA;
  its GetVersionExA call at RVA 0x19d6 is only CRT cookie-seeding — no OS
  gate found in it). It is launched by xEyeSetupHelper.dll!0x84b0 via
  `CreateProcessA("DriverX64Setup.exe", '-HardwareId="%s" -InfFile="%s"')`
  and its exit code 0/0xFFFFFFFF is checked (0 = install OK).
- xEyeDriverCheck.dll (stream 0x97a35, x86, exports 15 functions incl.
  PreinstallSpecialDriver 0x1d69, CheckSpecialDevice 0x1751,
  RescanHardwareChanges 0x239a) is ALSO duplicated at stream 0x23e5c3e.
- PreinstallSpecialDriver internals: index tables at RVA 0x7078
  (0x30-byte GUID entries incl. {3F3C9E60-C465-11CF-8056-444553540000},
  {6BDD1FC6-810F-11D0-BEC7-08002BE2092F} = HID class, {4D36E96C-E325-
  11CE-BFC1-08002BE10318} = USB class) and 0x7138 (0x104-byte inf-name
  entries 'xeyez.inf'); core at 0x1c86 -> per-inf worker 0x1970
  (GetSystemDirectoryA + CopyFileA + CoDeviceInstallers RegSetValueExA +
  SetupOpenInfFileA/SetupCopyOEMInfA).
- InstallDriver message boxes (all MessageBoxA, title 'idong感应器 安装'):
  RVA 0x9589 = 预安装xeyez失败! (Preinstall fail), 0x977e = 扫描失败！
  (scan fail when 0x2c60 devnode gate fails), 0x9c14 = 驱动目录没有找到!
  (no XP/Vista/Win7 match — patched in t21).
- NSIS installer script strings: vcredist_x86.exe -silent; xEyeService
  -service; InstallDriver called with 3 args stored to 0x10013388..90.

## Operational rules (cumulative; the important ones)

- `cd /home/z/my-project/research/idong100` before git ops.
- NSIS solid repack recipe (final, turn-22 script is the reference):
  decompress with FORMAT_ALONE (props + b'\xff'*8 + file[F+33:]) using
  dec.decompress(stream) + `while not dec.eof: decompress(b'')`; verify
  stream length EXACTLY equals the original's before patching; patch
  with SINGLE-BYTE stores or slice `out[off:off+len]` (NEVER
  `out[off:len]` — that inserts); recompress with dict 0x800000 lc3
  lp0 pb2 MODE_NORMAL nice_len 128 BT4; new_x2 = F+33+len(payload)-F+4;
  crc = crc32(file[512:F+24]+pack(new_x2)+props+payload); tail from
  F+old_x2-4+4; zero PE security dir.
- Embedded-PE location: scan `4d5a9000030000`, parse exports, match by
  export NAME. Beware MULTIPLE copies of the same DLL in one stream
  (xEyeDriverCheck at 0x97a35 AND 0x23e5c3e) — patch BOTH if both are
  extracted.
- pefile export iteration: `pe.DIRECTORY_ENTRY_EXPORT.symbols`.
- For x64 PEs: RIP-relative IAT calls need target = addr+size+disp, and
  compare as RVA (IAT slot VA - imagebase).
- pip install pefile capstone unicorn after every sandbox recycle.
- The kit's software/ dir holds BOTH original sources AND patched
  outputs; the original .dat sha256 6a877968deee70e9 (23,655,864 B),
  original .dat stream length 95,248,820 bytes.

## Repo state (turn 22)

- main: 335d920 (turn-22 preinstall stub) on top of faff296 (t21).
- project/state: 90ad425. chat/context: this file (being pushed).
- Kit rebuilt: download/idong100_p2_usbpcap_kit.zip (24 files,
  166,123,880 B).

## Where the project stands

- Phase 1 (pure core) COMPLETE on main.
- Phase 2: P2.3 kit + t18 platform gate v2 + t19 OS-gate + t20 NSIS CRC
  + t21 IsWin7/devnode + t22 PreinstallSpecialDriver stub. P2.1 probe
  done; P2.2 udev rule open.
- NEXT: user re-runs install_patched.bat with turn-22 files (+ one-boot
  DSE if signature refused) -> full vendor install -> README_CAPTURE
  §3/5 capture session -> boot.pcapng -> byte-for-byte gate comparison
  on the host.

## Environment notes

- analysis/ holds xEyeSetupHelper_orig.dll, xEyeDriverCheck_orig.dll,
  DriverX64Setup extraction facts (PE at stream 0x64a7e1, 64-bit).
- /tmp/stream_xeyez_alone.dat.bin (decompressed stream) may be STALE
  after sandbox recycles — regenerate before use.
