# Chat context backup - turn 27 - 2026-11-25

Project: idong100 (Toltech / i-dong "idong Sensor V2.6", USB 0471:2600 boot
loader, 0471:2500 running). Phase 2 item P2.3: USBPcap capture kit for
Windows 7. Repository: https://github.com/thomas7475/Idong100.git
(branches main, project/state, chat/context). Working clone:
research/idong100_restore (survived this turn).

## User message of turn 27

Pasted predir.bat self-test output (all four candidates "cannot create the
folder", PREDIRRC 2), both foldercreated screenshots, fresh PAT. Quote:
"the script is still not working correct it says the folder where not
created but from the picture you can clearly see that they where created."

Standing rules: English only; zips + chat backup in download/ every turn;
PAT re-supplied each turn, never printed or stored; pure cmd .bat, no
PowerShell; scripts stay open after they run.

## What the screenshots showed (vision tool)

foldercreated.png: Explorer at C:\Users\t - the folder idong_capture is
there. foldercreated2.png: Explorer at C:\ - the folder idong_capture is
there, Aenderungsdatum 25.11.2026 14:39 - exactly when the self-test ran.
So md DID create the folders; the script's existence check denied it.

## Root cause (turn 27): the \nul existence trick lied

The turn-26 scripts checked folders with "if exist folder\nul" - a DOS-era
device-name trick. On NTFS it answers true for any existing folder; on
FAT32/exFAT volumes (common for VM disks) it answers FALSE even for a
folder that md created a millisecond before. The user's Windows runs from
such a volume: md succeeded (timestamps prove it), the check lied, all
four candidates were rejected with "cannot create the folder", PREDIRRC=2
again, and every script aborted before USBPcap started. The same \nul
check sat in run_capture.bat (ARG2/work/runN), test_usbpcap.bat (work) and
check_capture.bat (TARGET/runN scan). A second bug made :note append the
first candidate's error twice (visible in the user's paste).

## The turn-27 fix

1. predir.bat :try - md unconditionally, then copy /y nul probe; the write
   alone decides. No existence checks anywhere; :note fixed to append once.
2. run_capture.bat: ARG2 probe write; work-probe write; mkrun loop now
   probes runN by writing dir_probe.tmp (a failed write advances N).
3. test_usbpcap.bat: work folder by probe write.
4. check_capture.bat: folder tests use the trailing-backslash form
   "folder\" only (works on every file system); \nul variants removed.
5. predir.bat header comment, CAPTURE_KIT.md section 7a rewritten around
   "a folder counts only when a file can be written into it", new
   troubleshooting row for the \nul liar; SOLVE_DIALOG turn-27 section;
   MANIFEST turn 27.

Verification: lint 0 errors on all 9 .bat; Node scanner harness 66/66
PASSED; kit text checks OK (ASCII, CRLF, no PowerShell, no \nul checks in
the delivered files - only the REM comment that documents the old bug).

## Deliverables in download/

- idong100_p2_usbpcap_kit.zip (19 files, 138.7 MB) + folder - turn 27
- idong100_phase2_turn27_2026-11-25.zip (14 files, repo snapshot)
- chat_context_backup_2026-11-25_turn27.md (this file)

## Not verified

No Windows in the sandbox: the FAT32/exFAT explanation for the lying \nul
check is the consistent hypothesis (folders created at 14:39 while the
self-test denied them), not a verified fact. The fix removes the check on
every file system, so it works regardless of the explanation.

## Next

User replaces the kit with the turn-27 one. predir.bat by hand should now
print "OK: C:\idong_capture is writable." Then test_usbpcap.bat ->
run_capture.bat -> check_capture.bat; send the runN folder it names.
