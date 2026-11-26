# Zcode Chat Context Backup — 2026-11-24 (turn 23)

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

## Turn 23 request and outcome

User: "the installer crashes now" + screenshot
`idong-xeyez-setup-crach.png`: box 安装驱动程序失败！
("Driver installation failed!") + support phone 400-1818-520,
installer behind it at ~85-90 % progress, last status
"Start install driver!".

**Root cause (disassembly): NOT a bug — the sensor was not attached.**
- The turn-22 preinstall stub WORKED; install reached the real driver
  stage.
- That stage shows dialog 201 (请将idong感应器连线... = "please plug
  the sensor into USB; this dialog auto-closes when connected").
- Its WM_TIMER handler (xEyeSetupHelper.dll RVA 0x75f0) retries the
  device scan 10 x 1 s looking for USB\VID_0471&PID_2500 /
  2500&MI_04 / 2600 / 0666 / 0668 / 2100 (six comparison strings at
  RVAs 0xdabc/0xdad4/0xda74/0xda8c/0xdaa4/0xdb08; check sub 0x4340;
  loop at 0x9795-0x981c with 5000 ms Sleep).
- If none found: logs "User terminate!" (string RVA 0xde38), calls
  0x8050 which shows **dialog 202** = 安装驱动程序失败！+ service
  phone (UTF-16 resource at .rsrc RVA 0x19eb0, shown via
  FindResourceA/LoadResource in sub 0x65d0) — the user's exact box.
- Vendor design: the driver stage needs the LIVE devnode (enumerates
  it, then installs the matching INF; xeyez.inf matches those five
  Philips VID_0471 IDs).

**Fix = user action, no binary change:** run install_patched.bat WITH
the idong sensor plugged into a rear USB port (no hubs). New
`check_sensor_attached.bat` (wmic VID_0471 pre-flight, cmd only) added
to kit root + scripts/. Docs updated (SOLVE_DIALOG, CAPTURE_KIT,
MANIFEST turn-23 notes).

## Sandbox recycle note (IMPORTANT for future turns)

The sandbox was recycled between turn 22 and turn 23:
- `research/idong100` working tree was WIPED (only CONTEXT.md,
  README.md, agents/ left). Restored via
  `git clone download/idong100_repo_bundle_2026-11-24.bundle` ->
  `research/idong100_restore`, then fetch from GitHub + checkout
  main/project/state/chat/context. **If the tree is gone again,
  re-clone from the bundle or GitHub and work there.**
- `download/`, `scripts/`, `analysis/` (incl. nsis extraction with
  $PLUGINSDIR VersionCheck.dll) all SURVIVED.
- `/tmp/stream_xeyez_alone.dat.bin` was STALE — regenerate by
  decompressing the kit's xeyez_alone.dat when needed.
- pip install pefile capstone (unicorn for emulation) after recycle.

## Repo state (turn 23)

- main: 9cd4b00 (turn-23 docs+script) on top of 335d920 (t22).
- project/state: 652c97f. chat/context: this file.
- Kit rebuilt: download/idong100_p2_usbpcap_kit.zip (28 files,
  166,129,470 B) incl. check_sensor_attached.bat.
- Binaries UNCHANGED from turn 22 (final):
  xeyez_setup_patched.exe 82,838 B
  sha256 e82c2f444d2d7b9619b07247323cbd1447ba2865d53d3af587bd5bc1ef0c7ee8
  xeyez_alone_patched.dat 23,637,085 B
  sha256 845a9170eee16dde87a59cf656c9c1594368975112f6218b200301f78fa16368

## Analysis facts recovered this turn (reuse, don't re-derive)

- Helper dialog resources (zh-CN lang 2052): 201 = connect-sensor wait
  (auto-closes on connect; 中止安装(&T) button); 202 = 安装驱动程序失败！
  (this turn's box); 203 = 复位idong感应器失败！; 204 = 安装虚拟输入驱动
  程序失败; 205 = 清除旧虚拟输入驱动程序失败; 206 = reboot-needed for
  virtual input update; 209 = 缺少USB HUB驱动.
- Dialog-show helper: sub 0x65d0 (FindResourceA id, LoadResource,
  CreateDialogIndirectParam-style via [0x1000d04c]/[0x1000d058]);
  failure-report wrapper 0x8050 shows dialog 202.
- InstallDriver tail: device-found path 0x995e maps device flavor
  (edi 1..6) to INF hardware IDs then calls CheckSpecialDevice(0xad2c)
  per flavor; failure tails 0x9bed/0x9bef just LOG (wsprintfA "%d" fmt
  at 0x10012ec0, log sub 0xad3e) and clean up.
- The installer ALSO expects possibly the firmware-download path
  (0x9d50 uses xeyezHal_DownLoadHex for bootloader PID_2600 devices).
- If install keeps failing WITH sensor attached: try
  install_drivers_manual.bat (pnputil route) + one-boot DSE
  (README_CAPTURE §4).

## Where the project stands

- Phase 1 (pure core) COMPLETE on main.
- Phase 2: P2.3 kit final (t18-t22 patches). P2.1 probe done; P2.2 udev
  rule open.
- NEXT: user re-runs install_patched.bat WITH the sensor attached
  (verify with check_sensor_attached.bat) -> full vendor install ->
  README_CAPTURE §3/5 capture session (Wireshark + USBPcap) ->
  boot.pcapng -> byte-for-byte gate comparison.
