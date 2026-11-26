# Kit 37 - the engine unlock, the third gate + the kit-36 repairs

One folder, one linear flow. Everything below runs by double-click;
nothing needs PowerShell; every window stays open at every exit.

## What is new in kit 37

1. **The third gate: xEyeWarrningUI.dll.** The "hardware test" box
   (`对不起，您的感应器硬件没有授权` - sorry, your sensor hardware is
   not authorized) is not a hardware test at all. It is the license box
   drawn by `xEyeWarrningUI.dll!ShowDeviceIDInvalidWindow` (the bitmaps
   sorrytext / DeviceID / phone / email / bbs in `Chs\images\`). The kit
   now ships a patched `xEyeWarrningUI.dll` for both engine builds: the
   box function returns at once, the box can never appear. The patch is
   3 bytes at the function entry (`xor eax,eax; ret`) - verified this
   turn: the function is cdecl with a bare `ret` epilogue, the real
   caller in xeyez.exe does `call proc; add esp,4` and ignores the
   return value, and an emulation of the patched DLL shows zero window
   calls with a perfectly balanced stack.
2. **The kit-36 hangs are fixed.** patch_all / verify_all /
   restore_all / run_direct froze after the header. Two causes:
   - `net session` as the admin gate (needs the LanmanServer service,
     can block for minutes in a VM). Now `fltmc` - local and instant.
   - The install search used `dir /s /b` on registry roots, and a
     registry value like `C:\` in `RootPath` made cmd crawl the whole
     drive silently - your window showed only the header and a cursor
     for minutes. The new finder scans at most two folder levels deep
     per root, refuses drive roots, and prints every folder it checks.
     Worst case now is a few seconds, always with visible progress.
3. **run_capture repairs.**
   - It opened one USBPcap window per root hub (four on your machine)
     and only one ever captured. It now reads the sensor's
     `LocationInformation` from the registry (both 0471:2600 and
     0471:2500 - the sensor re-enumerates between them on the same
     port) and captures only the root hub the sensor sits on. The pcap
     file is named after the real device (USBPcap1.pcap means the
     device \\.\USBPcap1 - your hub list arrives as 4,1,2,3, so naming
     by list position was wrong). `run_capture.bat all` forces every
     hub like kit 36 did.
   - Every run reused `run1` and the folder had to be emptied by hand.
     Each run now goes into a NEW folder: the first runN that does not
     exist, or exists but is empty.

## The three gates the kit opens

| DLL | Gate | Patched by |
| --- | --- | --- |
| xEyeUSB.dll | upsell ad + exit(-1), and the version-tag gate `DSHY` (this unit answers `000T`) | turn-33 patch (shipped) |
| xEyeRight.dll | license server call to us.i-dong.com - dead since years, a stock install can never be authorized | turn-34 patch (shipped) |
| xEyeWarrningUI.dll | the not-authorized box itself | turn-37 patch (this kit) |

All three gates are independent: the engine must pass all of them
before the lobby's hardware test succeeds. The kit patches all three
in one run, with backup, byte-verify and rollback per file.

## The linear flow

1. `patch_all37.bat` - right-click, Run as administrator. Finds the
   install (registry values TOLTech\Location, Toltech\xeyez_engine
   \Location and \RootPath, the services, Program Files, fixed folders,
   or the folder you pass as an argument), checks each DLL state and
   copies the right patched file over it. Unknown builds are refused,
   never touched. If your engine lives somewhere odd, run it from a
   cmd line with the folder:
   `patch_all37.bat "D:\my folder\xeyez_engine"`
2. `verify_all37.bat` - read-only state report of all three DLLs.
   The result line must say: all three gates open.
3. Start the lobby - it should now get past the hardware test. Or
   `run_direct37.bat`: the engine plus the xEyeThirdLens camera viewer
   without the lobby.
4. `run_capture.bat` while the camera runs - then `check_capture.bat`
   on the folder it prints. Send me the folder if anything still fails.

## Files

| File | Role |
| --- | --- |
| patch_all37.bat (admin) | find + patch all three gates, backup + verify + rollback |
| verify_all37.bat | read-only: which gates are open right now |
| restore_all37.bat (admin) | undo: from the .idong3*-backup, else from the vendor refs |
| run_direct37.bat | engine + camera viewer without the lobby |
| run_capture.bat | USBPcap capture, sensor hub only, new runN folder per run |
| check_capture.bat | scan of a capture folder (phase report) |
| test_usbpcap.bat | USBPcap self-test |
| check_sensor_attached.bat | is the sensor 0471:2600 / 0471:2500 / absent |
| predir.bat | capture-folder self-test |
| patched/ | the finished patched DLLs (both engine builds) |
| ref/ | vendor + intermediate references for state detection |

Restores also accept the kit-36 backup files (`.idong36-backup`).

## Proofs (this turn)

- unicorn emulation of the patched xEyeWarrningUI.dll: zero window
  calls, EAX = 0, stack balanced; the vendor DLL walks the whole show
  path (CreateDialogParamA, SendMessageA, SetWindowPos, both engine
  singletons) and returns 0 - byte-identical behavior except the box.
- lief structural check: machine, entry point, import and export
  tables unchanged in both patched DLLs.
- decision-logic simulation (Python) of every script state: fresh
  vendor, turn-31, turn-33, kit-36 state, v2.5 engine, foreign builds,
  restore paths, the run-folder picker, and the hub choice with your
  real hub list order (4,1,2,3).
- static lint: CRLF, ASCII, label/goto integrity, quote/paren balance
  of every .bat.
