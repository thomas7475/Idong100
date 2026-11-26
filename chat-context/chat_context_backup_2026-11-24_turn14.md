# ZCode Chat Context Backup — 2026-11-24 (turn 14)

**How to use this file:** attach/upload it in a future session so the
assistant can restore full context of this conversation and continue
where it left off. This file was written by the assistant itself as a
session snapshot.

---

## User's standing rules (apply to EVERY reply from now on)

1. **English only** for all communication.
2. **Zip backup of generated files** must be placed in
   `/home/z/my-project/download/` every time files are produced.
3. **Chat-context backup** (this kind of file) must be produced every
   time as well, downloadable from `/home/z/my-project/download/`.
4. **NEVER run full `cargo` builds** for the complete project — not
   enough RAM. Use `cargo check`, single-bin/example builds, or lowered
   parallelism.
5. **Git state tracking** (new, this turn): the project uses
   https://github.com/thomas7475/Idong100.git with two dedicated
   branches that are updated at the end of every turn:
   - `project/state` — current project state (research tree, analysis
     reports, scripts, deliverables)
   - `chat/context` — chat context backups (this file's branch)
   PAT is re-supplied by the user per session (never committed).

## Turn 14 request and outcome

Q: "lets start with Phase 0 use the following git repository
github.com/thomas7475/Idong100.git upload the existing research and
drafts use a dedicated branch to keep track of the project state and
chat context use different branches for state and chat context. upload
the latest states and chat context, keep this updated in the feature."

**Done — all parts:**
- Cloned the repo with the PAT to `/home/z/my-project/idong100-repo`
  (remote had research branches for turns 01/02/09 merged in `main`;
  local tree was ahead with turns 10/12/13).
- **`project/state` branch created and pushed**: full research tree
  (`research/idong100/` agents 01-13 FINDINGS + artifacts + CONTEXT.md),
  `analysis/reports/` (incl. `all_extracted_sha256.tsv`, 2475 files),
  `scripts/` (all extraction/analysis tools), `deliverables/`
  (turn-12 camera research PDF, turn-13 Rust ROS2 driver draft PDF,
  45-file `idong100-rs-scaffold/`), rewritten root `README.md`
  (branch table + safety rules), `.state-README.md` (branch index +
  conventions). Commit: `state(turn 14): Phase 0 — ...`
- **`chat/context` branch created and pushed**: `chat-context/` with
  backups for turns 11, 12, 13 + this turn-14 file + branch README
  (usage + file table). Commit: `context(turn 14): ...`
- Pre-commit secret scan: no PAT in any committed file (verified with
  grep over research/, analysis/, scripts/, deliverables/).
- Local copy for user download: zip with both branch trees +
  `git_bundle` (see download listing).

## Key facts about the git setup (for future turns)

- Clone URL pattern:
  `git clone https://<PAT>@github.com/thomas7475/Idong100.git idong100-repo`
- Branches: `main` (base), `project/state`, `chat/context`, plus the
  frozen `research/idong100-*` history branches from the Phase-1e era.
- Update procedure at end of each turn: commit new/changed files to
  `project/state` (message `state(turn N): summary`), commit the new
  `chat_context_backup_<date>_turnN.md` to `chat/context` (message
  `context(turn N): chat backup <date>`), push both.
- The local working tree `/home/z/my-project/research/`,
  `/home/z/my-project/analysis/`, `/home/z/my-project/scripts/`,
  `/home/z/my-project/download/` remains the primary workspace; the
  repo is the published mirror. Regeneration scripts are committed, so
  the repo is self-contained.
- Repo commit convention: research FINDINGS carry agent numbers; the
  state branch carries turn numbers. Turn = conversation turn.

## Where the project stands (after turn 14)

- Phase 0 of the implementation draft (turn-13 PDF, section 7) is now
  COMPLETE: git repo set up, state + context branches pushed, research
  and drafts uploaded.
- Next: **P1 (pure core)** — implement `idong` crate modules
  (frames/records/calib/boot/cmd/stream) with golden vectors;
  `cargo check` only (rule 4). Scaffold files are in
  `deliverables/idong100-rs-scaffold/` (to be moved to workspace root
  when implementation starts). Older turn-10 crate draft exists at
  `research/idong100/rust/idong/` (kept for reference, not pushed to
  repo state branch — only in local tree; superseded by the scaffold).
- Then P2: read-only bring-up + the USBPcap write gate.

## Continuity from earlier turns (unchanged, still in force)

- Full protocol map in `upload/RUST_DRIVER_DRAFT-v2.md` (turn-11 draft)
  and turn-12/13 FINDINGS: PU/OD boot framing, 7F 55 runtime framing,
  cmd 3/5/6/15, xEye3Ddata 0x3BC layout, T24C16A EEPROM layout,
  register scripts (0x02 = 0x0A blob / 0x0B image mode), firmware
  sha256 manifest.
- Safety rules: read-only EP 0x82 until USBPcap gate; no speculative
  EP 0x02 writes; NEVER write the T24C16A EEPROM; no full cargo builds.
- pefile/capstone need `pip install` after sandbox recycle; 7zz at
  /tmp/7zip/7zz (re-download from GitHub ip7z/7zip 24.08 if missing).
