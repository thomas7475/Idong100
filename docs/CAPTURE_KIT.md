# idong100 USBPcap capture kit - turn 27, 2026-11-25 - Windows 7

> **Turn 36 note:** the ENGINE UNLOCK moved to `tools/kit36` - the one
> consolidated kit that also ships the finished patched DLLs
> (patched/xEyeUSB_t33.dll + xEyeRight_t34.dll - the license gate fix
> behind the "hardware not authorized" box). Apply kit36 BEFORE
> capturing: `patch_all36.bat` (admin), then `verify_all36.bat`. The
> capture workflow itself stays the one below.

This kit records the complete boot of the idong sensor ("idong Sensor V2.6",
USB ID 0471:2600 while it waits for its program, 0471:2500 when it runs), so
that the byte stream can be compared with the Phase 1 decoder. Every script is
pure cmd.exe, none uses PowerShell, and every window stays open at the end.

## 1. The four questions

### 1.1 Is there a way to reset the camera to USB ID 2600?

No command is needed. 0471:2600 is the state the sensor is in after every
power-up: it is the boot loader of its 8051 microcontroller. The vendor program
finds it there, downloads the MCU program (`app_m8051_2611.hex`), and the sensor
disconnects and comes back as 0471:2500, where the DSP upload (`ens_code.ldr`)
and the FPGA upload run. This is read from the vendor DLLs
(`xeyezHal.dll`, `xEyeUSB.dll` 2.6.0.6), it was not tested on hardware here.

There is no shipped tool that resets a running sensor back to 2600. The vendor
DLL does contain an `McuReset` command, and the scripts in this kit never send
it, because it is not needed and a wrong reset could damage the sensor.

So the way back to 2600 is to **unplug the sensor and plug it in again**, while
USBPcap is already recording and the vendor program is running.

In the VM: use the USB menu of the VM software and disconnect the sensor from
the guest, then connect it again. The VM may show two devices, 0471:2600 and a
moment later 0471:2500. Connect both to the guest.

### 1.2 check_capture.bat does nothing and closes immediately

Fixed. The old checker needed a path as argument, printed one line and exited
without a pause. It also looked for `C:\idong_capture\boot.pcapng`, and that
file was never written: the old `run_capture.bat` called USBPcap with
`--snap 0` (USBPcap rejects 0), without `-A` (USBPcap then records nothing) and
from `C:\Program Files\USBPcap1\` (the installer puts the program into
`C:\Program Files\USBPcap\`). The new `check_capture.bat` needs no argument: it
reads `C:\idong_capture` by itself, parses the USB packets, prints a report and
waits for a key. You can also drag one `.pcap` file or a folder onto it.

### 1.3 Is there a way to test if USBPcap is working as intended?

Yes: `test_usbpcap.bat`. It finds `USBPcapCMD.exe`, prints the version, checks
the driver service and the USB hub class filter, lists the root hubs, shows
whether the sensor is seen as 2600 or 2500, records 20 seconds from every root
hub, and then counts the USB packets in the files. The last line is
`USBPcap WORKS in this Windows` or the reason why not.

### 1.4 Where is the captured file stored?

| File | Content |
| --- | --- |
| `C:\idong_capture\run<N>\USBPcap1.pcap`, `USBPcap2.pcap`, ... | One capture per USB root hub, `<N>` grows with every run |
| `C:\idong_capture\run<N>\session.txt` | Start, stop, USBPcap version, elevated or not |
| `C:\idong_capture\run<N>\sensor_states.txt` | When the sensor was seen as 2600, 2500 or not present |
| `C:\idong_capture\run<N>\check_report.txt` | The same report that `check_capture.bat` printed |
| `C:\idong_capture\usbpcap_test\test<K>.pcap` | Files of `test_usbpcap.bat`, can be deleted |
| `C:\idong_capture\work\` | Scratch files of the scripts, can be deleted |

`<N>` is the number of the run folder: `run1`, `run2`, ... The folder is also
printed at the end of `run_capture.bat`.

## 2. Step by step on the Windows 7 VM

The vendor software is installed, the camera is detected and its driver works.
That is the starting point. Everything below runs **inside the Windows 7 VM**,
not on the host.

1. **USBPcap.** Run `software\USBPcapSetup-1.5.4.0.exe` and install it. Then
   **restart the VM**. Without the restart the USBPcap driver is not loaded and
   `USBPcapCMD` answers "No filter control devices are available."
2. **Test USBPcap.** Right-click `test_usbpcap.bat`, Run as administrator. Move
   the mouse inside the VM while it records. The result must be
   `USBPcap WORKS in this Windows`. If it is not, read the last lines: they
   name the reason and the fix.
3. **Prepare.** Start the vendor program and plug the sensor in, so that the
   camera works as usual. Leave the vendor program open.
4. **Record.** Right-click `run_capture.bat`, Run as administrator. It starts
   USBPcap on every root hub first, then tells you to unplug the sensor and to
   plug it in again. In the VM: disconnect it in the USB menu and connect it
   again. When the camera works again, press a key. Without a key the capture
   stops after 120 seconds. An optional number is the time limit:
   `run_capture.bat 300`.
5. **Read.** `run_capture.bat` scans the files at the end. You can also
   double-click `check_capture.bat` at any time, it reads the newest run
   folder.
6. **Send.** Copy the whole folder `C:\idong_capture\run<N>` to the analysis
   host: all `USBPcap<K>.pcap` files and the three text files. If the folder is
   inside a VM, shared folders, a network share or a second USB stick that is
   attached to the guest all work.

`check_sensor_attached.bat` is a quick check without recording: it prints
whether Windows sees the sensor as 0471:2600, as 0471:2500 or not at all.

## 3. How to read the scan

`check_capture.bat` prints one block per file: File, Size, Time, Sensor, MCU,
DSP, Banner and Verdict. The verdict of the best file is the result.

| Verdict | Meaning | What to do |
| --- | --- | --- |
| PASS | At least 10 MCU frames to the sensor and 3 frames from the DSP upload | Send the folder |
| PARTIAL | Some sensor frames, but the boot is cut off | Run again, unplug the sensor after the recording has started, wait longer |
| ENUM-ONLY | Only the USB enumeration, no sensor frames | The vendor program was not running, or the sensor was not replugged |
| EMPTY | File is 0 or 24 bytes long | USBPcap opened the root hub but no packet arrived: run `test_usbpcap.bat` |
| ERROR | Not a USBPcap file, or a corrupt record | Send the file anyway, the message says what is wrong |

The Sensor line shows the USB address of 0471:2600 and of 0471:2500. A good
capture shows both. If it says "0471:2500 did not appear inside this capture",
the vendor program did not finish the download: start it before the sensor is
plugged in and run again.

## 4. Notes for a VM

- USBPcap records what the **guest** Windows sends and receives. The VM passes
  the USB transfers through to the real sensor, so the payloads in the file are
  the vendor driver's requests and the sensor's answers. The timing in the file
  is the timing of the guest, not of the physical bus.
- Both 0471:2600 and 0471:2500 have to be attached to the guest. If the VM
  software asks again when the sensor changes its ID, answer yes.
- If the VM uses a USB 3.0 controller and the test files stay empty, switch
  the VM's USB controller to USB 2.0. USBPcap needs an extra step for USB 3.0
  root hubs: `USBPcapCMD.exe -I` in an elevated command prompt, as its own
  help text says.
- Run the scripts with "Run as administrator" inside the guest. Otherwise
  Windows shows one UAC prompt per root hub for every run.

## 5. Wireshark, optional

The kit does not need Wireshark: `check_capture.bat` reads the files itself.
To look at a capture by hand, install `software\Wireshark-win64-3.2.18.exe`
inside the VM and open `USBPcap<K>.pcap`. Wireshark 4.x does not install on
Windows 7: the Wireshark documentation says "Wireshark 3.2 was the last release
branch to officially support Windows 7 and Windows Server 2008 R2", and 3.2.18
is the last 3.2 version in the official version index. 3.2.18 is from 2021 and
gets no security updates, so use it only to open your own capture files.

## 6. Without the scripts

In a command prompt that was started with "Run as administrator":

```
"C:\Program Files\USBPcap\USBPcapCMD.exe" --extcap-interfaces
"C:\Program Files\USBPcap\USBPcapCMD.exe" -d \\.\USBPcap1 -A -o C:\idong_capture\manual.pcap
```

The first line lists the root hubs, for example `\\.\USBPcap1`. The second one
records that root hub into `manual.pcap`. Stop with `q` in that window. Three
rules: `-A` is required, an existing file is never overwritten (USBPcap exits
at once, use a new name), and the path must not contain a space when the window
is not elevated.

## 7. Troubleshooting

| What you see | Reason | Fix |
| --- | --- | --- |
| `test_usbpcap.bat`: "not found. USBPcap is not installed" | USBPcapSetup was not run inside the VM | Run it inside the guest, restart the VM |
| "none. USBPcap finds no USB root hub" | No restart after the install, driver not loaded | Restart the VM |
| "USBPcapCMD created no file" | UAC prompt was not answered, or not elevated | Run as administrator |
| Files stay at 0 or 24 bytes | No USB traffic during the test, or USB 3.0 root hub | Move the mouse, replug the sensor, USB 2.0 controller |
| Sensor line: "0471:2600 did not appear" | Recording started too late | Unplug the sensor after the recording has started |
| Sensor stays at 2600, no 2500 | Vendor program not running | Start the vendor program, or restart it while the capture runs |
| Sensor: not present | VM does not pass the sensor to the guest | Connect it in the USB menu of the VM |
| "cannot create the folder C:\idong_capture" (turn 25) | `C:\idong_capture` was created by an elevated window; a normal window may read it but not write into it | Every script calls `predir.bat`: it repairs the rights when elevated, or picks a per-user fallback folder and says which one. To repair by hand: run any kit script once with Run as administrator |
| "[ERR] no writable capture folder ..." but the folders ARE there (turn 27) | The turn-26 scripts checked folders with `if exist folder\nul`, a DOS-era trick that reports "not there" on FAT32/exFAT drives (common in VMs) right after `md` created them - so every candidate was falsely rejected with "cannot create the folder" | Fixed in turn 27: no existence checks anywhere, a real file write is the only judge. Re-download the kit of turn 27 |
| "[ERR] no writable capture folder: none of the four candidates ..." (turn 26) | Even the per-user folders rejected the probe file | Double-click `predir.bat` alone: its self-test lists each folder with the exact error and the rights of `C:\idong_capture`, and it stays open. Send that text |

## 7a. The write-protected C:\idong_capture and the fallback folders (turns 25-26)

`C:\idong_capture` can end up writable only for administrators: an elevated
window created it (the first run of an elevated `run_capture.bat` or
`test_usbpcap.bat`). On Windows 7, a folder created by an elevated process is
by default not writable by a normal user window: `md C:\idong_capture\run1`
then fails with "Access is denied", which the scripts of turn 24 reported as
"cannot create the folder". Deleting the folder does not help, because the
next elevated script re-creates it with the same rights.

Every script of the kit calls the helper `predir.bat` before it writes
anything. It tries four folders in this order:

1. `C:\idong_capture` - probe file written and deleted,
2. `%USERPROFILE%\idong_capture` - `C:\Users\<name>\idong_capture`,
3. `%LOCALAPPDATA%\idong_capture` - `...\AppData\Local\idong_capture`,
4. `%TEMP%\idong_capture` - `...\AppData\Local\Temp\idong_capture`.

Folders 3 and 4 are per-user folders that are writable by design on every
Windows 7 profile, so one of the four always works. When candidate 1 fails
and the window is elevated, `predir.bat` first repairs the rights of
`C:\idong_capture` with `icacls ... /grant Users:(OI)(CI)M` (the SID form
`*S-1-5-32-545` is tried first, for non-English Windows where the group has
another name) and probes again.

**No script checks anymore whether a folder "exists"** (turn 27). The old
`if exist folder\nul` check reported freshly created folders as missing on
FAT32/exFAT drives - common in VMs - which is why the turn-26 kit claimed
"cannot create the folder" for folders that were demonstrably there. Now
`md` runs, then a probe file is written, and only the write decides. The
same rule applies to `run_capture.bat` (its `work` and `runN` folders) and
`test_usbpcap.bat`; `check_capture.bat` recognizes folders by the trailing
backslash form `folder\`, which works on every file system.

`predir.bat` sets `CAPROOT` (the folder to use) and `PREDIRRC` (0 = normal,
1 = fallback, 2 = nothing was writable). The scripts print the folder they
use at the start; the file list of section 1.4 applies to that folder. The
repair grants the *Users* group the right to create, change and delete files
in the capture folder - the same rights the folder would have if a normal
window had created it. It touches nothing else.

`run_capture.bat` also takes the capture folder as its second argument:
`run_capture.bat 300 D:\caps` records into `D:\caps\runN`, for machines whose
`C:` is full or broken.

### Running predir.bat by hand

Double-clicking `predir.bat` alone starts its self-test: it prints the
verdict for each of the four candidate folders, the exact error of every
failed probe, and - when it tried the repair - the current rights of
`C:\idong_capture` as `icacls` prints them. It ends with `pause`, so the
window stays open, and with the steps for copying the window text out (see
7b). When all four folders fail, run it by hand once and send its text.

A one-time alternative without the kit: right-click `cmd.exe`, Run as
administrator, then `icacls C:\idong_capture /grant Users:(OI)(CI)M`.

## 7b. Copying text out of a cmd window (turn 26)

A cmd window is not a normal text window, so dragging the mouse over it
selects nothing. To copy its text out:

1. right-click the **title bar** of the window (or its icon, top left),
2. in the menu that opens choose **Markieren** ("Mark", English: Edit - then
   Select All; on English Windows the menu item is "Edit"),
3. the whole window content is now selected - press **Enter** to copy it,
4. click into any text field (browser, editor, e-mail) and paste with
   **Strg+V**.

Every kit script prints these four steps at its end, in both the German and
the English wording. The alternative is a screenshot, but the copied text can
be searched and takes no photo of the whole screen.

For an open cmd window that is stuck at a prompt: the same steps work at
any time, they do not need the script to be finished.

## 8. What is verified and what is not

- Read in the USBPcap 1.5.4.0 source (`USBPcapCMD/cmd.c`): the program is a
  windowless program that attaches to the console of its parent, both stdout
  and stderr redirected to a file keeps it quiet, `-A` is required, the output
  file is created with `CREATE_NEW`, the driver service is named `USBPcap`, a
  process that is not elevated restarts itself with a UAC prompt per root hub.
- Tested: the scanner inside `check_capture.bat` with synthetic USBPcap files
  in a Node harness: full boot, enumeration only, partial boot, runtime only,
  records across 1 MiB read chunks, header only, truncated file, not a pcap,
  empty file, stream only, 2600 and 2500 on the same address, interrupt
  endpoints, corrupt record. All passed.
- Not tested: the cmd.exe scripts were never executed, there is no Windows in
  the sandbox where they were written. They were checked line by line and with
  a static checker for the usual batch traps. The capture recipe was not tried
  on a real sensor. If a script stops with an error, send the text of its
  window.
- `Wireshark-win64-3.2.18.exe` was downloaded from the official version index
  of wireshark.org over HTTPS. SHA-256:
  `cbee6bb8d250287d8bcc41dc0353c425d87dceffa5005b3d9ab755090709e7b9`.
  The checksum was not compared with a second source.

## Turn 30 note: keep the vendor software ALIVE during capture

The turn-28 capture ended while the app was still polling because the vendor
engine self-terminates: on a failed edition check it shows the upsell
MessageBox and calls exit(-1) when you press OK (see SOLVE_DIALOG.md turn 30).
Before the next capture session, apply the turn-30 fix kit on the Win7 machine
(fix_idong_alive30.bat: HKLM\SOFTWARE\TOLTech\Location registry value +
lobby\ConfigManagement.dll shim). Then run the vendor session long enough to
reach the live camera image (wait until the picture shows, keep it ~60 s,
then stop) so the capture finally includes the DSP/FPGA upload and the
isochronous camera stream.

## Turn 31 note: apply the PATCH kit (not the turn-30 registry/shim) before capture

The turn-30 registry + shim fix did not hold up on the Win7 machine (see
SOLVE_DIALOG.md turn 31 for the defect analysis). The keep-alive fix is now
the 4-byte patch of xEyeUSB.dll (download/idong100_patch31 kit):
run patch_idong_alive31.bat as Administrator, confirm with
verify_idong31.bat, then start the vendor software and take the next
USBPcap capture once the live camera image shows (keep it ~60 s). The
registry entries from turn 30 are irrelevant now; optionally
reg delete "HKLM\SOFTWARE\TOLTech" /f.
