# SOLVING "请您先安装爱动平台软键" (Please install the iDong platform first)

You hit the vendor's platform-dependency gate. Full byte-level analysis
of the installers (turn 17) found exactly what it checks and three ways
past it. **Solution B (registry key) is the recommended one** — 2 minutes,
reversible, and then the sensor installer runs normally.

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

## Solution A (cleanest, recommended): fake the platform registry mark

The gate reads one registry value. Give it what it wants:

1. Save this as `platform_mark.reg` and double-click it (or run
   `reg import platform_mark.reg` as admin) in the Windows VM:

```reg
Windows Registry Editor Version 5.00

[HKEY_LOCAL_MACHINE\SOFTWARE\TOLTech]
"Location"="C:\\Program Files (x86)\\TOLTech"

[HKEY_LOCAL_MACHINE\SOFTWARE\TOLTech\xeyez_engine]
"RootPath"="C:\\Program Files (x86)\\TOLTech\\xeyez_engine"
```

   (On 32-bit Windows drop the ` (x86)` part.) The `TOLTech\xeyez_engine
   RootPath` value is what the *sensor installer itself* writes later
   (`SOFTWARE\Toltech\xeyez_engine` — case-insensitive), so setting it up
   front is also what the install flow expects.

2. Re-run `xeyez_setup.exe`. The gate passes, it launches the inner
   installer (`xeyez_alone.dat` — the "standalone" flavor; the launcher
   offers `xeyez_Lobby.dat` for the full portal), which installs the
   driver + app + firmware.
3. If the inner installer still refuses (it carries its own copy of
   `VersionCheck.dll`): run `xeyez_alone.dat` **directly** — rename to
   `xeyez_alone.exe` if Windows refuses a `.dat` extension — the inner
   installer's own platform probe is the same registry check, and it
   also passes once the mark is present.

## Solution B: skip the launcher entirely

The two `.dat` files are full NSIS installers (PE32 executables). The
launcher adds nothing but the gate. Copy `xeyez_alone.dat` to
`xeyez_alone.exe` and run it directly. If it self-checks the same
registry mark, combine with Solution A's `.reg` import first.

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
stop after ~60 s of runtime traffic, `check_capture.ps1` for the
magic-byte sanity check.

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

## Files in this fix kit

| File | Purpose |
|---|---|
| `platform_mark.reg` | The registry values the gate reads (Solution A) |
| `check_gate.ps1` | Verifies the gate's probes in your VM (run as admin) |
| `SOLVE_DIALOG.md` | This document |

Verify with `check_gate.ps1` after importing the `.reg`; it reports
which probes pass (registry Location, `\\.\vKeyboard`/`\\.\vMouse` only
exist after the *virtual HID drivers* are installed — those are NOT
required for the launcher to pass; the registry mark alone satisfies
the launcher's `VerifyInstallSystem` "Lobby" path).
