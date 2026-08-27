# Changelog

## 0.1.0 - 2026-08-27

First release.

### Added

- File-board lanes under `.necro/` (`1-todo`, `2-doing`, `3-done`, `4-dropped`)
- CLI and MCP command graph through incurs
- Task commands: `init`, `add`, `list`, `show`, `next`, `start`, `done`, `drop`, `reopen`, `note`, `snapshot`
- `watch` for lane enter and exit events
- `watch --agent grok` or `--agent claude` headless yolo spawn with live stdout on the task file
- Todo enter claims the task before spawn, skips if the file left todo, and records a nonzero agent exit without dropping the task
- Exclusive lock at `.necro/agent-watch.lock` for one `--agent` watcher per board
- Lane and board `AGENTS.md` in the watch-agent prompt
