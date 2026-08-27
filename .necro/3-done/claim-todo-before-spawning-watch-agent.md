# Claim todo before spawning watch agent

On todo enter with --agent, move the task to doing before spawning the headless process so two watchers cannot both implement the same file.

## Done when

- A todo enter with --agent grok claims the task (`necro start`) before `grok -p` runs.
- If the claim fails because the task is already in doing, skip the spawn.
- Tests cover the claim and the skip with a stub agent.

## Agent output

I'll list the necro board, read the todo-lane task, then claim it before implementing the watch-agent claim/skip behavior.The task is on the board. I'll claim it, then inspect how todo-enter currently spawns the watch agent.Task is claimed. Next I’ll inspect `start` and existing watch tests so the claim-before-spawn path can fail closed.I'll write the failing stub-agent tests first, then add exclusive todo-claim so spawn only happens after `start`.I'm running the necro test suite to verify todo-enter claims before spawn and skips when already doing.Tests passed. I'll keep `run_agent` spawn-only so claiming stays on the watch hook path, then format the crate.A todo enter with `--agent` now claims the task before spawn, and a second watcher skips if the file is already in doing.

`claim-todo-before-spawning-watch-agent` is in **done**.

On a todo enter, watch runs an exclusive todo → doing move (`Store::claim`) before `grok -p`. If the task is already in doing, or the todo file is gone, the agent is not started. `--exec` and non-todo events are unchanged.

Verified with `cargo test`:
- `todo_enter_claims_before_agent_spawn` — stub sees the file in doing and `-p` as argv[1]
- `todo_enter_skips_agent_when_already_doing` — stub is not executed
- `claim_moves_todo_and_skips_when_already_doing` — store-level exclusive claim
