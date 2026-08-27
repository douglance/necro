# Note failed agent runs without dropping the task

If a watch --agent process exits nonzero, still append stdout to the task under Agent output, leave the task on the board (do not drop it), and keep watching later events.

## Done when

- Nonzero agent exit still records stdout on the current task file for that ID.
- The task is not moved to dropped.
- The long-lived watcher continues after the failure.
- Tests cover nonzero stub exit.

## Agent output

I'll list the necro board, read the task, and start it before making any edits.The task is on the board. I'll start it, then inspect how `watch --agent` handles nonzero exits.The task is in `doing`. I'll read the watch-agent code and tests to see how nonzero exits are handled today.Current code already notes stdout, then returns `ExecFailed`. I'll trace whether that error drops the task or stops the watcher.I’ll add a failing test for a nonzero stub exit first, then change the agent hook so a failed run is noted and the watcher keeps going.The new tests failed as expected. I’ll search the log for the assertion that failed.Cargo only takes one test filter. I’ll rerun with `nonzero_exit` so both cases run.The tests fail on `ExecFailed`, which is the bug. I’ll stop treating a nonzero agent exit as a watch error, then add a watcher-continuation test.I’ll format the crate, then run the nonzero-exit tests to confirm they pass.The three new tests passed. I’ll run the full suite next to check for regressions.The suite is green. I’ll mark the necro task done.`note-failed-agent-runs-without-dropping-the-task` is in **done**.

A nonzero `watch --agent` exit is now a completed hook, not a watch failure. Necro still appends stdout under `## Agent output`, leaves the task on the board, and keeps polling.

`--exec` still fails the hook on a nonzero status. Agent failure only sets `exec_exit` and notes stdout.

Verified with `cargo test` (22 lib, 14 CLI, 22 store):
- `run_agent_records_stdout_on_nonzero_exit_without_dropping` — stub exits 2, stdout is noted, task stays in todo
- `claimed_agent_nonzero_exit_notes_doing_task_without_dropping` — claimed doing task is noted and not dropped
- `watch_agent_nonzero_exit_notes_keeps_task_and_keeps_watching` — long-lived watcher handles a second todo after the failed stub
