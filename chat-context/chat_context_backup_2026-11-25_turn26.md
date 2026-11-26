# Chat context backup - turn 26 - 2026-11-25

Project: idong100 (Toltech / i-dong "idong Sensor V2.6", USB 0471:2600 boot
loader, 0471:2500 running). Phase 2 item P2.3: USBPcap capture kit for
Windows 7. Repository: https://github.com/thomas7475/Idong100.git
(branches main, project/state, chat/context). Working clone:
research/idong100_restore (re-cloned this turn; sandbox was recycled again).

## User message of turn 26

"the new scrips are not working the predir.bat does nothing and does not
stay open the other capture .bat complain that it could not write to the
folder even if it is there they generate a work folder but for some reason
the scripts no not work rewrite them, and why cant i mark and copy any of
the content of the script cmd window". Screenshot capture.png attached;
fresh PAT in the message. German Windows 7 ("Druecken Sie eine beliebige
Taste" seen in the screenshot title bar area).

Standing rules: English only; zip backups in download/; chat context backup
every turn; PAT re-supplied each turn, never printed or stored; no
PowerShell in deliverables, pure cmd .bat; scripts stay open after they run.

## What the screenshot showed (read with the vision tool)

run_capture.bat ended with: "[ERR] no writable capture folder: neither
C:\idong_capture nor the USERPROFILE fallback could be written." So the
turn-25 predir.bat returned PREDIRRC=2 on BOTH of its two candidates, and
every script aborted before USBPcap was even started. The "work folder" the
user saw was the leftover C:\idong_capture\work of the earlier attempt.

## What was done (turn 26)

1. Sandbox recycle recovery: re-cloned the repo from GitHub, restored the
   turn-25 files from download/idong100_p2_usbpcap_kit/, .gitattributes
   re-created.
2. predir.bat REWRITTEN: four candidate folders in this order -
   C:\idong_capture, %USERPROFILE%\idong_capture,
   %LOCALAPPDATA%\idong_capture, %TEMP%\idong_capture. The last two are
   per-user folders that are writable by design on Windows 7, so one of the
   four always accepts a file. Icacls repair of C:\idong_capture when
   elevated (SID form *S-1-5-32-545 first, then "Users"); when the repair
   fails, the ACL dump is printed instead of swallowed. Every probe error is
   collected and shown. Hand run (double-click) is a supported self-test
   that ends with pause; calling scripts use "predir.bat /called" for the
   quiet variant. Sets CAPROOT, PREDIRRC, PRENOTE.
3. run_capture.bat / test_usbpcap.bat / check_capture.bat rewired: /called
   form, PRENOTE shown, new :no_workdir texts that point at the hand-run
   self-test, and every :done exit shows the 4 steps to copy the window
   text (title bar - Bearbeiten - Markieren - Enter, paste with Strg+V;
   English: Edit - Select All). run_capture.bat also takes a capture folder
   as second argument now: run_capture.bat 300 D:\caps.
4. Docs: CAPTURE_KIT.md turn 26, section 7a rewritten (four candidates,
   hand-run self-test, second argument), new section 7b "Copying text out of
   a cmd window" (the answer to the mark-and-copy question: cmd windows are
   not normal text windows, dragging selects nothing). SOLVE_DIALOG.md turn
   26 section. MANIFEST turn 26.
5. Lint 0 errors on all 9 .bat; Node scanner harness 66/66 PASSED; kit text
   checks OK (ASCII, CRLF, no PowerShell); zips rebuilt with CRC OK.

## Deliverables in download/

- idong100_p2_usbpcap_kit.zip (138.7 MB, 19 files) + folder
- idong100_phase2_turn26_2026-11-25.zip (14 files)
- chat_context_backup_2026-11-25_turn26.md (this file)

## Verified / not verified

- Push verified after the fact (fresh fetch, remote tips printed).
- The scripts are statically checked only: no Windows in the sandbox. The
  folder candidates, the icacls repair and the fallback chain were never
  executed for real. If a case is not covered, the hand-run predir.bat
  window text decides the next step.
- The German menu names (Bearbeiten/Markieren/Strg+V) are from the user's
  screenshot locale; the English equivalents are printed too.

## Next

User: replace the kit in the VM with this turn-26 kit. If a script still
refuses: double-click predir.bat alone, copy its window text out (steps are
printed at its end) and send it. Otherwise: test_usbpcap.bat ->
run_capture.bat -> send the runN folder it names.
