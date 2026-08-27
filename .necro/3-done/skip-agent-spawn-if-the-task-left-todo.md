# Skip agent spawn if the task left todo

Between detecting a todo enter and spawning the agent, re-check that the task file is still in todo. If it has already been started, done, or dropped, skip the spawn.

## Done when

- A stub test moves the file out of todo before spawn and the agent binary is not executed.
- A still-in-todo file still spawns.

## Agent output

I'll start by listing the necro board, reading the task, and locating the todo-enter spawn path so we can skip the agent when the file has already left todo.The board has this task in todo. I’m starting it in necro and locating the todo-enter spawn path so we can re-check the file before the agent runs.Current spawn already claims on todo enter. I’ll read the store claim path and recent related work so the new still-in-todo check sits in the right place.The watch path already claims before spawn, but it does not explicitly re-check that the file is still in todo. I’ll add failing stub tests for that race, then implement the guard.I’ll record the work, then add stub tests that move the task out of todo after the enter is detected and assert the agent binary does not run.The stub tests now move the file after enter detection. Next I’ll add the still-in-todo re-check on the spawn path so a started, done, or dropped task never runs the agent.I’ll point the skip tests at the between-detect-and-spawn hook so they move the file in that window, not only before the function is called.I'm formatting the crate and running the watch tests to verify skip-on-left-todo and still-in-todo spawn.The focused tests failed. I’m searching the durable log for the assertion so I can fix the skip path.The focused watch tests passed. I’m running the full crate suite next to confirm the skip path didn’t regress anything else.A todo-enter watch now re-checks that the task file is still in `todo` before spawning. If the file has already been started, done, or dropped, the agent binary does not run.

**Phase:** Implementation complete. Task `skip-agent-spawn-if-the-task-left-todo` is in **done**.

**Evidence:** `cargo test` passed.

- `todo_enter_skips_agent_when_task_left_todo` — a stub moves the file to `doing`, `done`, or `dropped` between detect and spawn; `agent-ran` is never created.
- `todo_enter_still_spawns_when_file_remains_in_todo` — a file that stays in `todo` still claims and runs the stub.

README and CONTEXT now describe this re-check. Remaining board item: `support-claude-as-watch-agent` in todo.
