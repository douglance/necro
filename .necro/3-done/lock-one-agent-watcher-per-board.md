# Lock one agent watcher per board

Two `necro watch --lane todo --agent grok` processes on the same board must not both spawn for the same enter. Take an exclusive lock under `.necro` while watching with --agent.

## Done when

- A second --agent watcher on the same board fails with a stable error code.
- --exec and event-only watch do not require the lock.
- Tests cover lock held and lock free. Update README.

## Agent output

I'll start by listing the necro board, reading the task, and locating the current watch/agent lock behavior.The board lists this task in todo. I’ll start it, then inspect the watch command and any existing lock patterns.The task is in doing. Next I’ll read the watch command, error codes, and tests so the lock matches existing patterns.I’ll read the watch CLI handler, board paths, and how other tests assert error codes.The crate forbids `unsafe`, so I’ll use rustix for flock. Next I’ll confirm that API and write the lock tests.I’ll add failing tests for lock held and lock free first, then implement the exclusive agent-watcher lock.The lock test failed as expected. I’ll search the log to confirm it failed on the assertion, not a compile error.RED is confirmed: the second watcher waited instead of returning `AGENT_WATCHER_LOCKED`. I’ll add the exclusive lock next.I’m running the lock unit tests and CLI lock tests to confirm they pass.Unit tests passed. Next I’ll run the CLI lock tests.The CLI tests still wait, which suggests the spawned `necro` binary may be stale or exiting before it takes the lock. I’ll inspect the binary and the child process next.The binary is current. I’ll start a watcher on a temp board and check whether it creates the lock file.The rebuilt binary does take the lock. I’ll rerun the CLI tests against that current binary.The parent was racing the child for the lock and killing the holder. I’ll wait until the child owns the lock before starting the second watcher.CLI lock tests passed. I’ll run the full suite and clean up any unused imports.A second `necro watch --agent` on the same board now exits with `AGENT_WATCHER_LOCKED`. `--exec` and event-only watch do not take the lock.

`--agent` watchers take an exclusive flock on `.necro/agent-watch.lock` for the life of the process. A second `--agent` watcher fails immediately with that stable code. `--exec` and event-only watch skip the lock.

Evidence: `cargo test` passed (lib, store, and CLI, including lock held and lock free). README documents the lock. Task `lock-one-agent-watcher-per-board` is in done.
