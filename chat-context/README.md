# chat/context — session backup branch

This branch stores **chat context backups**: one markdown snapshot per
conversation turn, written by the assistant at the end of every session
so that any future session can restore the full conversation state and
continue where the project left off.

## Usage

- Restoring: open the newest `chat-context/chat_context_backup_*.md`,
  attach/upload it to the assistant session, and continue.
- Updating: after every turn, the newest backup is committed here
  (commit format `context(turn N): chat backup <date>`).

## Files

| File | Turn | Date |
|---|---|---|
| `chat_context_backup_2026-11-23_turn11.md` | 11 | 2026-11-23 |
| `chat_context_backup_2026-11-23_turn12.md` | 12 | 2026-11-23 |
| `chat_context_backup_2026-11-24_turn13.md` | 13 | 2026-11-24 |
| `chat_context_backup_2026-11-24_turn14.md` | 14 | 2026-11-24 (this turn: git Phase 0 setup) |

This branch intentionally contains **only** chat context files and this
README — project state (research, reports, deliverables) lives on
`project/state`.
