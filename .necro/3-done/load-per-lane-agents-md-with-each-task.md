# Load per-lane AGENTS.md with each task

Each lane folder may contain AGENTS.md. Agents use that file with the task markdown to know how to complete work in that lane.

## Done when

- `.necro/todo/AGENTS.md`, `.necro/doing/AGENTS.md`, `.necro/done/AGENTS.md`, and `.necro/dropped/AGENTS.md` are optional instruction files, not tasks.
- `necro show`, `next`, `start`, `done`, `drop`, `reopen`, and `note` include `agents` and `agents_path` from the task's current lane.
- `necro watch` events include the same fields. `--exec` sets `NECRO_AGENTS_PATH`.
- Missing AGENTS.md yields null `agents` and `agents_path`. Do not fail.
- `list` still ignores AGENTS.md.

Lane AGENTS.md is attached to show, next, start, done, drop, reopen, note, and watch. cargo test passed.
