# Include board AGENTS.md in watch prompt

The watch agent prompt already includes the lane AGENTS.md and the task. Also include the board-root AGENTS.md (`./AGENTS.md` at the board root) when that file exists.

## Done when

- `agent_prompt` contains the board-root AGENTS.md body when present.
- Missing board-root AGENTS.md does not fail the watch.
- Tests cover both cases. Update README.

## Agent output

I'll start by listing the necro board, reading the task, and locating how the watch prompt is built.The board has this task in todo. I'll start it, then find how `agent_prompt` is assembled.The task is in doing. Next I’ll read the watch prompt builder, existing tests, and README so the tests can fail first.I’ll add failing tests for both AGENTS.md cases first, then implement the prompt change.I’m running the new `agent_prompt` tests to confirm the present-file case fails before any implementation.The present-file test failed on the expected assertion. I’ll include board-root `AGENTS.md` in the prompt and update the README.The watch agent prompt now includes board-root `AGENTS.md` when that file exists.

`agent_prompt` reads `./AGENTS.md` at the board root and inserts it under `# Board AGENTS.md` before the lane instructions. If the file is missing or unreadable, that section is omitted and the watch continues.

Tests cover both cases. `cargo test` passed (17 lib, 10 CLI, 22 store). README documents the prompt contents.

Task `include-board-agents-md-in-watch-prompt` is in `done`.
