# Add necro drop

Move a task to `.necro/dropped` instead of deleting it.

## Done when

- REQ-DROP-001 `init` creates `.necro/dropped`. A board has todo, doing, done, and dropped.
- REQ-DROP-002 `necro drop <id>` moves the file into dropped. Idempotent if already dropped.
- REQ-DROP-003 `reopen` returns a dropped task to todo.
- REQ-DROP-004 `list --status dropped` and `--status all` include it. Default list stays todo.
- REQ-DROP-005 `start` and `done` reject dropped tasks with `ALREADY_DROPPED`.
- REQ-DROP-006 No file delete. No `rm` command.

Implemented dropped lane, drop, ALREADY_DROPPED. cargo test passed.
