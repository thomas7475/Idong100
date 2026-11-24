# SOLVING "请您先安装爱动平台软键" (Please install the iDong platform first)

You hit the vendor's platform-dependency gate. Full byte-level analysis
of the installers (turn 17) found exactly what it checks and three ways
past it. **Solution A (registry mark) is the recommended one** — 2 minutes,
reversible, and then the sensor installer runs normally.

> **Turn-18 update — if you imported the turn-17 `platform_mark.reg`
> on 64-bit Windows and the dialog STILL appeared:** that was a bug in
> the v1 `.reg`, now fixed. `VersionCheck.dll` (the gate) is **32-bit
> code**, so on 64-bit Windows it reads `HKLM\SOFTWARE\Wow6432Node\...`,
> while v1 wrote only the 64-bit view. Compatibility mode (Vista/Win7)
> cannot fix this — it never changes which registry view a 32-bit
> process sees. The current `platform_mark.reg` writes BOTH views;
> re-import it (as admin) and the gate passes. Verify with
> `check_gate.bat` (pure cmd, no PowerShell).

## What the dialog actually is

The dialog does NOT come from the sensor software itself. `xeyez_setup.exe`
is only a **launcher** (82 KB, contains just one plugin). Its decompressed
NSIS script carries this exact string:

> `请您先安装爱动平台软键` — "Please install the iDong (爱动) platform software first!"

Its gate logic (recovered from the script + `VersionCheck.dll` disasm):

1. Calls `VerifyInstallSystem` in `VersionCheck.dll`.
2. That checks whether the **Sports King platform** (爱动平台 — the big
   game portal you are NOT installing) left its mark:
   `HKLM\Software\TOLTech` value `Location` (the platform's install dir).
3. `VerifyInstallSystem` also probes `\\.\vMouse` / `\\.\vKeyboard`
   (virtual HID device nodes that the *platform's* drivers create) and
   the `%CommonAppData%\i-dong` folder.
4. Platform absent → the MessageBox you saw → installer aborts.

The sensor stack itself (driver + app + firmware, in `xeyez_alone.dat`)
has **no technical need** for the platform — the gate is a marketing
bundle-order check from 2010. Your dialog title "idong感应器 安装" is
just the sensor installer's branding.

## The 64-bit registry-view trap (turn 18, root cause of the retry)

`VersionCheck.dll` is PE32 (`machine 0x014c` — 32-bit x86) and opens
its keys with `RegOpenKeyExA`. Windows-on-WOW64 redirects every
`HKLM\SOFTWARE\...` access from a 32-bit process into
`HKLM\SOFTWARE\Wow6432Node\...` **silently**:

| You write | The 32-bit gate reads |
|---|---|
| `HKLM\SOFTWARE\TOLTech` (64-bit view) | `HKLM\SOFTWARE\Wow6432Node\TOLTech` — **miss!** |
| `HKLM\SOFTWARE\Wow6432Node\TOLTech` (32-bit view) | `HKLM\SOFTWARE\Wow6432Node\TOLTech` — hit |

So on a 64-bit VM (Win10 x64 as the kit recommends), the turn-17 v1
mark landed in the view the gate never reads. The fixed
`platform_mark.reg` writes both views, plus the `Toltech`-spelled
engine keys in both views. On real 32-bit Windows the extra
`Wow6432Node` lines are inert dead keys — no harm, no effect.

## Solution A (cleanest, recommended): fake the platform registry mark

The gate reads one registry value. Give it what it wants:

1. Import the kit's `platform_mark.reg` in the Windows VM — as admin:

   ```bat
   reg import platform_mark.reg
   ```

   (or double-click → Merge, confirm the UAC prompt). It writes both
   registry views:

   ```reg
   [HKEY_LOCAL_MACHINE\SOFTWARE\TOLTech]
   "Location"="C:\\Program Files (x86)\\TOLTech"

   [HKEY_LOCAL_MACHINE\SOFTWARE\TOLTech\xeyez_engine]
   "RootPath"="C:\\Program Files (x86)\\TOLTech\\xeyez_engine"

   ...same under Wow6432Node for the 32-bit gate on 64-bit Windows...
   ```

   (On real 32-bit Windows drop the ` (x86)` part — noted in the file.)
   The `TOLTech\xeyez_engine RootPath` value is what the *sensor
   installer itself* writes later (`SOFTWARE\Toltech\xeyez_engine` —
   case-insensitive), so setting it up front is also what the install
   flow expects.

2. Create the folder probe (optional but free — `check_gate.bat`
   does it for you): `md "%ProgramData%\i-dong"` as admin.

3. Re-run `xeyez_setup.exe`. The gate passes, it launches the inner
   installer (`xeyez_alone.dat` — the "standalone" flavor; the launcher
   offers `xeyez_Lobby.dat` for the full portal), which installs the
   driver + app + firmware.

4. If the inner installer still refuses (it carries its own copy of
   `VersionCheck.dll`): run `xeyez_alone.dat` **directly** — rename to
   `xeyez_alone.exe` if Windows refuses a `.dat` extension — the inner
   installer's own platform probe is the same registry check, and it
   also passes once the mark is present.

## Solution B: skip the launcher entirely

The two `.dat` files are full NSIS installers (PE32 executables). The
launcher adds nothing but the gate. Copy `xeyez_alone.dat` to
`xeyez_alone.exe` and run it directly. If it self-checks the same
registry mark, combine with Solution A's `.reg` import first.
(Note: turn-17 script analysis of `xeyez_alone.dat` shows its own
`Software\TOLTech\xeyez_engine` references are version/uninstall
bookkeeping — `2.6.0.6`, `TrayMenu`, `xeyez_engine_version` — so the
inner installer is primarily gated through its `VersionCheck.dll`
calls, same mark.)

## Solution C: install the real platform

If you want the full 2010 Sports King portal for completeness: it is the
big `idong_1.9.04_windows_yundongwang_A_setup.zip` (1.5 GB, still live
on `download.i-dong.com`, URL in the research notes) whose Inno header
we parsed — `app='爱动体感运动机(运动王PC版)' v1.9.04`, default dir
`{pf}\TOLTech`, 277 registry entries incl. `Software\TOLTech`. That is
the "platform software" the dialog means. Not needed for our capture.

## After the gate passes — for the USBPcap capture

Nothing else changes. Continue with `README_CAPTURE.md` as before:
attach the sensor, run `scripts/run_capture.bat`, boot the vendor app,
stop after ~60 s of runtime traffic, `scripts/check_capture.bat` for the
magic-byte sanity check (pure cmd + cscript; the old PowerShell
versions are retired — turn 18 user rule: no PowerShell).

One extra note now verified from the payload: the installer writes
`SOFTWARE\Toltech\xeyez_engine` `RootPath`, launches `xeyezMonitor.exe`
(tray) via `xEyezSvStartXeyez.exe`, and installs the `xeyez` kernel
driver (`DriverInstall\Win7\xeyezDriver64\xeyez.sys` on x64 —
hardware IDs `USB\VID_0471&PID_2600`, `...PID_2500&MI_04`,
`...PID_2100`, `...PID_0668`, `...PID_0666` — i.e. every sensor
generation from V1.0 to V2.6, bootloader and runtime). If Windows
10 x64 refuses the 2008-era signature, use the one-boot
"Disable driver signature enforcement" route (README_CAPTURE.md §4).

## Why the dialog showed garbled text

The installer is GBK-encoded (Chinese, codepage 936) but you read it
as CP437/CP1252, so 请您先安装爱动平台软键! displayed as
ÇëÄúÏÈ°²×°◎π¯Æ½Ì¨Èí¼þ! — the same bytes. If you want readable dialogs,
run the VM in a Chinese locale or with `locale emulator`-style
AppLocale settings; not required for the capture though.

## Files in this fix kit (all PowerShell-free, turn 18)

| File | Purpose |
|---|---|
| `platform_mark.reg` | The registry values the gate reads, **both views** (Solution A) |
| `check_gate.bat` | Verifies the gate's probes in your VM (run as admin; reports both registry views, creates the `i-dong` folder, checks the `xeyez` service) |
| `SOLVE_DIALOG.md` | This document |

Verify with `check_gate.bat` after importing the `.reg`. Its verdict
line follows the view that matters for your OS bitness: on 64-bit
Windows it requires the `Wow6432Node` mark (what the 32-bit gate
actually reads); on 32-bit Windows the plain view. `\\.\vKeyboard`/
`\\.\vMouse` only exist after the *virtual HID drivers* are installed —
those are NOT required for the launcher to pass; the registry mark
alone satisfies the launcher's `VerifyInstallSystem` "Lobby" path.
