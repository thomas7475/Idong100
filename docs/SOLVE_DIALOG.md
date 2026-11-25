# SOLVING "请您先安装爱动平台软键" (Please install the iDong platform first)

You hit the vendor's platform-dependency gate. Full byte-level analysis
of the installers (turn 17) found exactly what it checks and three ways
past it. **Solution A (registry mark) is the recommended one** — 2 minutes,
reversible, and then the sensor installer runs normally.

> **Turn-23 update — the "安装驱动程序失败！" box is NOT a bug: it means the
> sensor was not attached.** With the turn-22 kit the preinstall stub works
> and the install reaches the real driver stage. That stage scans for the
> actual hardware: dialog 201 (请将idong感应器连线...) pops for ~10 s and its
> WM_TIMER handler (xEyeSetupHelper.dll RVA 0x75f0) retries the device scan
> 10x1 s looking for USB\VID_0471&PID_2500 / 2500&MI_04 / 2600 / 0666 /
> 0668 / 2100. If none is found it logs "User terminate!" (RVA 0xde38) and
> shows dialog 202 = 安装驱动程序失败！+ the service phone — exactly the
> reported box (progress ~85 %, "Start install driver!" last status).
> **Fix: run the installer with the idong sensor plugged into a rear USB
> port.** The stage then enumerates the live devnode and installs the
> matching INF (xeyez.inf matches those five Philips IDs). New kit script
> `check_sensor_attached.bat` verifies enumeration before you start. If the
> sensor IS attached but not one of those IDs (shows as unknown device), run
> it through the bootloader path: the installer also accepts PID_2600
> (bootloader) and PID_2500 composite states.

> **Turn-22 update — the "Start install driver!" failure box:** with the
> turn-21 kit the installer extracted everything but still stopped at
> "Start install driver!" with the GBK box 预安装xeyez失败！ ("pre-install
> xeyez FAILED", title idong感应器 安装). Root cause: the NSIS script calls
> `xEyeSetupHelper.dll!InstallDriver`, whose first step calls
> `xEyeDriverCheck.dll!PreinstallSpecialDriver` — that helper copies
> Win7\xeyezDriver\xeyez.inf + xeyezCoIns.dll into the system dir, writes
> registry values under SYSTEM\CurrentControlSet\Control
> \CoDeviceInstallers and the legacy 'Driver Signing' policy keys, and runs
> `SetupOpenInfFileA` + `SetupCopyOEMInfA` on the 2008-era inf. On
> Win10/11 x64 that step fails (elevation / SetupCopyOEMInfA rejects the
> ancient inf) and the installer aborts with the box, even though none of
> it is a prerequisite for the rest of the flow. Fix (turn-22 kit): the
> `PreinstallSpecialDriver` thunk at RVA 0xad32 inside the embedded
> xEyeSetupHelper.dll is stubbed to `xor eax,eax; ret` (return success),
> on top of all earlier patches; the NSIS integrity CRC is recomputed and
> the stub-code integrity check passes. New files:
> `xeyez_setup_patched.exe` (82,838 B, SHA-256 e82c2f44...0c7ee8 —
> unchanged from turn 21) and `xeyez_alone_patched.dat` (23,637,085 B,
> SHA-256 845a9170...78fa16368). Replace your copies and re-run
> `install_patched.bat`. If the driver install step still balks, use the
> new `install_drivers_manual.bat` (pnputil /add-driver route) included
> in the kit root, and the one-boot 'Disable driver signature
> enforcement' route if signature is refused.

> **Turn-21 update — driver-install gate inside the installer:** after the
> turn-20 files, the installer extracted everything but failed at "Start
> install driver!" with the GBK dialog 驱动目录没有找到！ ("driver directory
> not found!", title idong感应器 安装). Root cause: the NSIS script calls
> `xEyeSetupHelper.dll!InstallDriver`, which checks the OS with
> `GetVersionExA`: XP = 5.1, Vista = 6.0, Win7 = 6.1. On Windows 8/10/11 an
> unmanifested 32-bit process is told 6.2/6.3, so ALL three checks fail and
> the installer aborts before choosing a driver directory. Fix (turn-21 kit):
> two bytes in the embedded xEyeSetupHelper.dll inside xeyez_alone.dat -
> `IsWin7` now returns true for any OS (so the Win7/Win7-64 driver sets are
> chosen), and the 'pre-install' devnode helper (CM_Locate_DevNodeA /
> CM_Reenumerate_DevNode, which fails with no sensor plugged in) is stubbed
> to return success. Re-download `xeyez_setup_patched.exe` (82,838 B,
> SHA-256 `e82c2f44…0ef0c7ee8`) and `xeyez_alone_patched.dat`
> (23,635,851 B, SHA-256 `b16d4a06…4f207d302`), replace the turn-20 ones
> (82,842 / 23,638,139 B) and re-run `install_patched.bat`.
> Note: the drivers themselves are 2008-era; on Win10/11 x64 the driver may
> still be refused signature enforcement - if Windows asks, use the one-boot
> 'Disable driver signature enforcement' route (README_CAPTURE.md section 4).

> **Turn-20 update — the patched installers showed "NSIS Error / Installer
> integrity check has failed":** the turn-19 repack rebuilt the compressed
> stream but left the archive's stored CRC and `length_of_all_following_data`
> stale, so the stub's integrity check failed before the gate was even
> reached. Root cause was recovered by emulating the stub's `loadHeaders`
> with Unicorn: the check is `crc32(file[512 .. firstheader + x2 - 4)) ==
> u32 at firstheader + x2 - 4` (`x2` = the `length_of_all_following_data`
> dword of the 28-byte firstheader; the first 512 bytes of the file are
> excluded, everything else - PE stub, firstheader, props, solid stream - is
> included). The turn-20 rebuild recomputes `x2` and appends a fresh CRC
> field, and the fixed files pass the *actual stub code* in emulation.
> Re-download `xeyez_setup_patched.exe` (SHA-256 `b2436be7…c165ded480`,
> 82,842 B) and `xeyez_alone_patched.dat` (SHA-256 `af070bbf…3ed85985e`,
> 23,638,139 B) and run `install_patched.bat` again.

> **Turn-19 update — the platform gate now passes and a SECOND
> dialog appears: `对不起，idong感应器软件与您的操作系统不兼容！`
> ("Sorry, the idong sensor software is incompatible with your OS!
> It runs on Windows XP, Windows 7 and 32-bit Vista"):** that is the
> OS-version gate, a *different* check in the same `VersionCheck.dll`.
> It whitelists ONLY XP (5.1), Vista (6.0) and Win7 (6.1) — and on
> WOW64 only Win7. Windows 8/10/11 → dialog + immediate `exit(0)`.
> **Compatibility mode genuinely cannot fix this one:** the gate reads
> `_winmajor`/`_winminor` — export *variables* of MSVCR80.dll —
> not `GetVersionEx` directly, so the compat layer's version lie
> never reaches the comparison. The fix is the kit's **patched
> installers** (`xeyez_setup_patched.exe`, `xeyez_alone_patched.dat`)
> whose embedded gate had 4 conditional jumps rewritten so every path
> returns 0 — the same value the gate returns on Windows XP. Run
> `install_patched.bat` (as admin) to swap them in and launch; the
> originals are backed up first. Full analysis in the section
> "The OS-version gate — turn 19" below.

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

## The OS-version gate — turn 19 (root cause of the incompatibility dialog)

Once the platform mark is in place, the launcher gets further and a
second MessageBox appears:

> `对不起，idong感应器软件与您的操作系统不兼容！（idong感应器软件可以运行在Windows XP，Windows7和32位Vista系统上。）`

Recovered from `VerifyInstallSystem` (RVA 0x20e4) in `VersionCheck.dll`
(the copy embedded in BOTH installers — they are byte-identical, PE32,
machine 0x014c):

1. `GetVersionExA` fills an `OSVERSIONINFO`; on failure the CRT's
   `_winmajor` / `_winminor` (MSVCR80.dll export *variables*) fill in.
2. Whitelist comparisons:
   - `major == 5 && minor == 1` → pass (XP);
   - `major == 6 && (minor == 0 || minor == 1)` → pass, and sets the
     "Win7" flag (this flag is only set for 6.1);
   - anything else → the MessageBox + `exit(0)`.
3. A WOW64 re-check: `IsWow64Process` + the "Win7" flag. On 64-bit
   Windows (the 32-bit DLL is always WOW64 there) every OS *except*
   Win7 fails this re-check and shows the same dialog. That is why the
   dialog appears on Windows 10/11 x64 regardless of the registry mark.

Why compatibility mode does nothing here: the comparison reads the
CRT snapshot variables `_winmajor`/`_winminor`, which MSVCR80
initialises from its own version query inside the plugin DLL's
startup — the kernel compat layer rewrites `GetVersionEx` *results*
for the process, but the whitelist numbers still describe your real
OS (10.0), which is not on the list. Only the numbers matter, so
only changing the numbers (or the comparison itself) helps.

### The fix: patched installers

`xeyez_setup_patched.exe` and `xeyez_alone_patched.dat` (in
`software/`, or generated by `scripts/rebuild_patched_installers19.py`)
each carry the same 5-byte patch inside their embedded copy of
`VersionCheck.dll`:

| file offset in DLL | original | patched | meaning |
|---|---|---|---|
| 0x1571 | `75 15` (jne) | `EB 15` (jmp) | XP-branch fail → re-check |
| 0x1579 | `75 0d` (jne) | `EB 0d` (jmp) | major≠6 fail → re-check |
| 0x1581 | `75 05` (jne) | `EB 05` (jmp) | minor≠1 fail → re-check |
| 0x15a4 | `74 06` (je)  | `EB 00` (jmp) | WOW64-dialog → benign |

Every control-flow path now returns 0 — the same return value as on
native Windows XP — and never reaches the MFC dialog + `exit(0)`.
The solid NSIS archives were decompressed, patched, recompressed
with identical LZMA settings, and round-trip-verified. The stale
Authenticode directory entry was zeroed (the vendor's signature
cannot survive the modification — Windows may show "unknown
publisher"; choose *Run anyway*). NSIS never verifies the signature,
and neither check the gate performs involves it.

Run `install_patched.bat` as administrator: it backs up
`xeyez_setup.exe` / `xeyez_alone.dat` (`.original.*`), copies the
patched ones in their place, and launches the installer. Then
continue with README_CAPTURE.md §3.


## Files in this fix kit (all PowerShell-free, turn 18)

| File | Purpose |
|---|---|
| `platform_mark.reg` | The registry values the platform gate reads, **both views** (Solution A) |
| `check_gate.bat` | Gate-probe status in your VM (run as admin; both registry views, the `i-dong` folder, the `xeyez` service, the patched-installer presence; stays open at the end) |
| `install_patched.bat` | Swaps in the OS-gate-patched installers and launches them (turn 19; backs up the originals first) |
| `SOLVE_DIALOG.md` | This document |

Verify with `check_gate.bat` after importing the `.reg`. Its verdict
line follows the view that matters for your OS bitness: on 64-bit
Windows it requires the `Wow6432Node` mark (what the 32-bit gate
actually reads); on 32-bit Windows the plain view. `\\.\vKeyboard`/
`\\.\vMouse` only exist after the *virtual HID drivers* are installed —
those are NOT required for the launcher to pass; the registry mark
alone satisfies the launcher's `VerifyInstallSystem` "Lobby" path.


## Turn 24 (2026-11-25): Windows 7 VM, the box after the driver install, check_capture closed at once

Setup of this turn: Windows 7 VM, vendor software installed, camera detected,
the driver works. All scripts of the kit were rewritten for this state.

### 1. The box after the driver install is not an error

The screenshot `win7-afterdriver.install.png` shows a message box of the vendor
program: "您购买的是爱动体感运动机(大众版)，若要升级到爱动体感运动机(运动王PC版)，请拨打咨询热线400-1818-520。"
In English: "You bought the i-dong motion machine (standard edition). To upgrade
to the i-dong motion machine (Sports King PC edition) please call the hotline
400-1818-520." It is an advertisement that the program shows when it recognises
the sensor as the standard edition. The installation is complete, the sensor
was found. Click OK and continue. Nothing has to be fixed.

This is a different box from the "安装驱动程序失败！" one of turn 23. That one
means that no sensor with VID 0471 was present while the installer looked for
it; run `check_sensor_attached.bat` to see the state before installing.

### 2. Why check_capture.bat "does nothing, it closes immediately"

Three defects of the turn-23 kit came together:

| Defect | Effect | Fix in turn 24 |
| --- | --- | --- |
| `check_capture.bat` needed a path as argument | It printed one line and exited, no pause | No argument needed: it reads `C:\idong_capture`, newest `runN`, a file or a folder can be dropped onto it, `pause` on every exit |
| `run_capture.bat` called `USBPcapCMD.exe --snap 0` without `-A` | USBPcap rejects snaplen 0, and without `-A` it records nothing | `-A` on every start, default snaplen |
| `run_capture.bat` started `C:\Program Files\USBPcap1\USBPcapCMD.exe` | The installer puts the program into `C:\Program Files\USBPcap\` | The scripts search the install folders and the uninstall registry entry |

So the checker looked for `C:\idong_capture\boot.pcapng`, a file that never
existed.

### 3. Further defects of the kit found and fixed in turn 24

| File | Defect | Fix |
| --- | --- | --- |
| all `.bat` | Line-feed line endings from the Linux sandbox | All converted to CRLF, `.gitattributes` keeps them CRLF |
| kit | Left-over PowerShell files `check_gate.ps1`, `scripts\check_capture.ps1` | Removed, the kit is pure cmd.exe |
| `run_capture.bat` | Search for a free run folder: `if exist ... set /a N+=1 & goto` - the `goto` ran even when the condition was false, an endless loop | Two separate statements with parentheses-free `if` |
| `run_capture.bat` | USBPcap was started in the window of the script | USBPcapCMD attaches to the console of its parent and reads its keys, so it swallowed the "press a key to stop" key press. It now runs in its own minimized window per root hub |
| `test_usbpcap.bat` | Label `:eval_missing` was missing | Added |
| both | Argument check through a pipe with delayed expansion | Replaced by a `for /f "delims=0123456789"` test, no pipe |
| `install_drivers_manual.bat` | `pnputil /add-driver` does not exist on Windows 7, and a fallback message without fallback code | Windows 7 syntax `pnputil -i -a`, real fallback to `Win7\xeyezDriver` |
| kit | `Wireshark-win64-4.6.10.exe` does not install on Windows 7 | Replaced by 3.2.18, the last 3.2 version |

### 4. Procedure on the Windows 7 VM

1. `software\USBPcapSetup-1.5.4.0.exe` inside the VM, then restart the VM.
2. `test_usbpcap.bat`: is USBPcap installed and does it record?
3. `run_capture.bat`: unplug the sensor while it records, plug it in again.
4. `check_capture.bat`: reads the files and prints the verdict.
5. Send `C:\idong_capture\run<N>` to the analysis host.

The files are in `C:\idong_capture\run<N>\USBPcap<K>.pcap`. Details are in
`CAPTURE_KIT.md`.

## Turn 25 (2026-11-25): the write-protected C:\idong_capture

User report: "for the new capture.bat i get cannot create he folder
C:\idong_capture the folder is there but it is write protected, it is created
when i deleted it." (The folder blocks writing; when it is deleted, it comes
back with the same problem - because the next elevated script creates it
again.)

### Cause

An elevated window (Run as administrator) created `C:\idong_capture` in an
earlier run. On Windows 7 the default ACL of a folder created by an elevated
process contains write entries for `BUILTIN\Administrators` and
`NT AUTHORITY\SYSTEM`, but not the normal write entry for the interactive
user. Every later non-elevated window could read the folder but not write into
it, so `md C:\idong_capture\run1` failed with "Access is denied" and the
turn-24 scripts reported "cannot create the folder".

Deleting and letting an elevated script re-create it reproduces the same ACL,
which matches the user's observation "it is created when i deleted it".

### Fix

New helper `tools/capture/predir.bat`, called by `run_capture.bat`,
`test_usbpcap.bat` and `check_capture.bat` before anything is written:

1. create `C:\idong_capture` when missing,
2. write and delete a probe file,
3. on failure, elevated: `icacls C:\idong_capture /grant Users:(OI)(CI)M`
   (the SID form `*S-1-5-32-545` is tried first, for accounts whose Users
   group is not named "Users"), then probe again,
4. on failure: fall back to `%USERPROFILE%\idong_capture`,
5. print which folder is in use and set `CAPROOT` / `PREDIRRC` for the caller.

Result: the scripts no longer stop on this folder. They either repair it
(elevated), or they use the fallback folder and say so, and the output always
names the folder the captures went to.

Also in this turn: turn 24 was pushed to GitHub (`main`, `project/state`,
`chat/context`), including the rebuilt kit zip in `download/`.

### Manual repair without the kit

Right-click `cmd.exe`, Run as administrator, then:

    icacls C:\idong_capture /grant Users:(OI)(CI)M

## Turn 26 (2026-11-25): the scripts still could not write, and the copy problem

User report: "the new scrips are not working the predir.bat does nothing and
does not stay open the other capture .bat complain that it could not write to
the folder even if it is there they generate a work folder but for some
reason the scripts no not work rewrite them, and why cant i mark and copy any
of the content of the script cmd window". Screenshot: run_capture.bat ended
with "[ERR] no writable capture folder: neither C:\idong_capture nor the
USERPROFILE fallback could be written" - predir.bat had returned 2.

### Why the turn-25 scripts stopped

The turn-25 predir.bat had only two candidates, C:\idong_capture and
%USERPROFILE%\idong_capture. On this machine both rejected the probe file,
so PREDIRRC=2 and every script aborted before USBPcap was even started. Why
both failed was invisible: the probe errors were discarded (>nul 2>&1) and
predir.bat itself closed instantly when run by hand (no pause, "not meant to
be run by hand"), which the user read as "does nothing and does not stay
open". The "work folder" the user saw being generated was the leftover
C:\idong_capture\work of the earlier turn-24 attempt.

### The turn-26 rewrite

1. predir.bat now tries four folders in this order:
   C:\idong_capture, %USERPROFILE%\idong_capture,
   %LOCALAPPDATA%\idong_capture, %TEMP%\idong_capture. The last two are
   per-user folders that are writable by design on every Windows 7 profile,
   so one of the four always accepts a file, even when C:\ or the profile
   root is locked down.
2. When the first candidate fails and the window is elevated, the icacls
   repair of C:\idong_capture runs (SID form first). When the repair fails
   as well, the current rights of the folder are printed, as icacls
   sees them, instead of being swallowed.
3. Every probe error is collected and shown: hand-run predir.bat prints one
   line per candidate with the exact reason ("cannot create the folder" /
   "cannot write a file into it").
4. predir.bat run by hand is now a supported self-test: header, verdicts,
   the error list, the result line, and a pause, so the window stays open.
   The calling scripts use `predir.bat /called` for the quiet variant.
5. run_capture.bat takes the capture folder as a second argument
   (run_capture.bat 300 D:\caps) for disks whose C: cannot be written at
   all.
6. The three capture scripts tell the user, at every exit, how to copy the
   window text out: title bar - Bearbeiten - Markieren - Enter, then paste
   with Strg+V (English: Edit - Select All). This is also section 7b of
   CAPTURE_KIT.md now. cmd windows are not normal text windows, so dragging
   over them selects nothing - that is the answer to the user's copy
   question, not a bug in the scripts.

### Still not verified

There is no Windows in the sandbox where these scripts are written: the
folder candidates, the icacls repair and the fallback chain are statically
checked only (0 lint errors, the JScript scanner harness passes 66/66). If
prefdir.bat by hand reports a case not covered by the four candidates, its
window text decides the next step.

## Turn 27 (2026-11-25): the folders WERE created, the script denied it

User report: "the script is still not working correct it says the folder
where not created but from the picture you can clearly see that they where
created." Attached: foldercreated.png (Explorer at C:\Users\t - idong_capture
sits there, created 25.11. 14:39) and foldercreated2.png (Explorer at C:\ -
idong_capture sits there, created 25.11. 14:39). The pasted predir.bat
self-test output says all four candidates failed with "cannot create the
folder" - but the timestamps of both folders match the run, so md had
created them.

### Root cause: the \nul existence trick

The turn-26 predir.bat judged a folder with:

    if not exist "%W%\nul" md "%W%" >nul 2>&1
    if not exist "%W%\nul" ( call :note "cannot create the folder" ... )

"if exist folder\nul" is a DOS-era device-name trick. On NTFS it answers
true for any existing folder, but on FAT32/exFAT volumes - common for VM
disks - it answers false even for a folder that md created a millisecond
earlier. The user's Windows ran from such a volume: every md succeeded
(both folders exist, timestamps prove it), every check lied, so all four
candidates were rejected with "cannot create the folder" and PREDIRRC=2
again. The same \nul check sat in run_capture.bat (ARG2, work, runN),
test_usbpcap.bat (work) and check_capture.bat (TARGET and runN scan).

A second, smaller bug compounded the confusion: :note appended the first
candidate's error twice (the empty-string test and the append both ran
under enabledelayedexpansion), which is why the pasted output lists
"[C:\idong_capture: cannot create the folder]" two times.

### The turn-27 fix

1. :try in predir.bat: md unconditionally, then copy /y nul into the
   folder - the write alone decides. No existence check anywhere.
2. :note rewritten so each error lands exactly once (PROBES_Cx guards).
3. The same treatment in run_capture.bat (ARG2 probe, work probe, mkrun
   loop: create runN, write dir_probe.tmp, only a failed write advances
   N), test_usbpcap.bat (work probe).
4. check_capture.bat: folder tests use the trailing-backslash form
   folder\ only (works on every file system); the \nul variants removed.
5. predir.bat header comment documents the turn-27 change; CAPTURE_KIT.md
   section 7a rewritten around "a folder counts only when a file can be
   written into it", plus a troubleshooting row for the \nul liar.

### Verification

Lint: 0 errors on all 9 .bat. Node scanner harness: 66/66 PASSED. Kit
text checks: ASCII, CRLF, no PowerShell. Still no Windows in the sandbox:
the FAT32 explanation for the lying \nul check is the consistent hypothesis
(both folders created at 14:39 while the self-test denied them), but the
volume format itself is not verified - the fix removes the check on every
file system, so the explanation no longer matters for the scripts to work.

### Next

User replaces the kit with the turn-27 one. predir.bat by hand should now
say "OK: C:\idong_capture is writable." Then test_usbpcap.bat ->
run_capture.bat -> check_capture.bat, and the runN folder it names gets
sent.

## Turn 28 (2026-11-25): the first real capture - and the download is the vendor hex file

User delivered idong_capture.zip: run1/USBPcap1.pcap (230,939 B, 3,260 USBPcap
records, 108 s) with two complete plug cycles, plus a screenshot of the
post-driver-install state. Quote: "i was able to get a capture of the USB camera
initializing the capture has 2 cycles of plugging un plugging, analyze the
findings i was able to open the capture with wireshark 4.0.17 is there more to
capture, if so let me know, if not lets continue with the driver implementation."

### What the capture contains (verified, not assumed)

Bus 4 (USBPcap1), device 2: two byte-identical boot cycles.

1. Enumerate as 0471:2600 (boot loader, maxpkt 64).
2. The 7f55 handshake: 0x0800 GetMcuVersion -> "2600"; 0x0101 with body
   0x53A3 = 21,411 = the exact download size; the image is sent as 334 x 64 B
   + 1 x 35 B raw blocks on ep 0x02 (no per-block framing); the sensor echoes
   every block back on ep 0x82 byte-identically (334/334 verified); 0x0102
   (0x8000 + size), 0x0103 (fe2ac9bb ae11d5c2), 0x0104 run.
3. Re-enumeration as 0471:2500 (composite ef/02/01, bcdDevice 3.001, 5
   interfaces: bulk 0x02/0x82/0x03/0x83 (512 B), iso 0x85 IN (812/4900 B
   alt 1/2), int 0x84 (32 B)).
4. Runtime on 0x02/0x82: 0x0701 image requests (replies are raw, not framed:
   "2619"+cc, 64-B image-like payloads), 0x0400-0x0404 exposure/gain pairs
   (00 38 / 00 18), 0x0300-0x0303 config write + verified read-back, 0x0500,
   0x0800 (-> "2619" = MCU app version), 0x0d00 and 0x0f04 polls. Control
   requests bm 0xA1 (req 82/84/83/86/87) return descriptors and TLV lists.
5. The session ends in 0x0701/0x0d00/0x0f04 polling - the vendor program
   never got further.

**Verified**: the 21,411 downloaded bytes are byte-identical to
app_m8051_2611.hex from the vendor installer loaded at 0x8000.
app_m8051_2500.hex does not match. The MCU download is fully reproducible
offline.

### Not in this capture (the honest gap list)

- No PU/OD frames at all: this firmware version does not use the 50 55 7f 7f /
  4f 44 7f 7f DSP framing the DLL analysis predicted for the DSP upload.
- No DSP upload (ens_code.ldr 54,420 B) and no FPGA image traffic.
- No traffic on ep 0x85 (isochronous) or 0x84 (interrupt) - the camera data
  stream itself was never opened by the vendor program in this window.

### Kit fixes of turn 28 (check_capture.bat)

1. The kit's own report called this valid pcap "not a USBPcap .pcap file
   (wrong magic number)". The magic constant is right; the failing path is
   bin() returning an empty string (VBArray/ADODB hiccup) - previously
   indistinguishable from a wrong magic. Now a short read is reported as its
   own error with a note that the file itself is fine.
2. The old PASS gate demanded PU+OD frames - which this firmware never sends.
   New PASS gate: both 2600 and 2500 descriptors present AND >= 10,000 B on
   ep 0x02 (the download volume). The real capture now scores PASS.
   Synthetic harness: 66/66 PASSED. The scanner counts ep02/ep82 byte
   volumes now.

### Verdict for the user

One more capture scene is still needed - the full vendor session until the
camera image actually shows in the vendor UI (that is when the DSP/FPGA
upload and the isochronous stream would appear). But the boot-loader half of
the driver is implementable now with zero further captures, because every
byte of that phase is verified against the vendor's own files.

### Files of turn 28

- scripts/analyze_capture28.py - USBPcap pcap parser (descriptors, tokens,
  endpoint stats, --hex dump)
- scripts/timeline_capture28.py - chronological event timeline + 7f55 census
- scripts/run_checker_once.js - runs the kit's JScript scanner on one pcap
  (Node shim) for local verification
- work/cap28/mcu_download.bin - the extracted 21,411-B download (kept out of
  download/ deliberately: it is a firmware image, not a deliverable)
- download/idong100_capture_analysis_turn28.md - the full analysis report
- Kit rebuilt as turn 28 (check_capture.bat with the fixes above).
