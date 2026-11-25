# Chat context backup - turn 25 - 2026-11-25

Project: idong100 (Toltech / i-dong "idong Sensor V2.6", USB 0471:2600 boot
loader, 0471:2500 running). Phase 2 item P2.3: USBPcap capture kit for
Windows 7. Repository: https://github.com/thomas7475/Idong100.git
(branches main, project/state, chat/context). Working clone:
research/idong100_restore.

## User message of turn 25

"push the latest turn24 to giithub and for the new capture.bat i get cannot
create he folder C:\idong_capture the folder is there but it is write
protected, it is created when i deleted it." Plus a fresh PAT.

Standing rules: English only; zip backups in download/; chat context backup
every turn; never run full cargo builds; PAT re-supplied each turn, never
printed or stored; no PowerShell in deliverables, pure cmd .bat; scripts stay
open after they run.

## What was done

1. Sandbox was recycled between turns: the working clone research/
   idong100_restore was lost. Re-cloned from GitHub with the fresh PAT, then
   restored the turn-24 files from the surviving kit copy in download/
   idong100_p2_usbpcap_kit/ (all .bat, docs, MANIFEST, .gitattributes).
2. Root cause of the report: C:\idong_capture was created by an elevated
   window; on Windows 7 such a folder is by default not writable for a normal
   window ("Access is denied" on md inside it). Deleting it does not help
   because the next elevated script re-creates it with the same ACL.
3. New helper tools/capture/predir.bat, called by run_capture.bat,
   test_usbpcap.bat and check_capture.bat before any write: probe file, icacls
   repair when elevated (SID form first), USERPROFILE\idong_capture fallback,
   CAPROOT/PREDIRRC for the caller, and every script prints which folder it
   uses.
4. Docs: CAPTURE_KIT.md section 7a + troubleshooting row; SOLVE_DIALOG.md
   turn-25 section; MANIFEST.txt turn 25.
5. Lint: 0 errors on all 9 .bat files; the linter itself was extended
   (tokens=2* declares two for-variables, redirection before & in if lines,
   echo( handling). Node scanner harness re-run: ALL CHECKS PASSED.
6. Git: main c9df6af, project/state 7af65c2, chat/context 366e341 pushed
   with the one-off PAT URL; the remote URL was reset to the credential-free
   form afterwards.

## Deliverables in download/

- idong100_p2_usbpcap_kit.zip (138.7 MB) and folder idong100_p2_usbpcap_kit/
  (predir.bat added, all .bat ASCII CRLF)
- idong100_phase2_turn25_2026-11-25.zip (repo tools/capture + docs)
- chat_context_backup_2026-11-25_turn24.md (pushed this turn)
- chat_context_backup_2026-11-25_turn25.md (this file)

## Verified / not verified

- Push verified: origin/main, origin/project/state and origin/chat/context
  are at the commits above after a fresh fetch.
- predir.bat is statically linted only: no Windows here, the icacls repair
  and the fallback were never executed for real.
- The Windows 7 ACL claim matches the user's report ("write protected,
  it is created when i deleted it"); the exact ACE list of the folder was
  not seen from here.

## Next

User: re-download the kit, run test_usbpcap.bat (administrator once, this
also repairs C:\idong_capture), then run_capture.bat, and send
C:\idong_capture\runN (or the USERPROFILE fallback folder the script names).
