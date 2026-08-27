# Support claude as watch agent

`--agent grok` already runs `grok -p` with `--always-approve`. Add `--agent claude` that runs `claude -p` with `--dangerously-skip-permissions` (yolo) in the board root, then appends stdout to the task file the same way grok does.

## Done when

- `--agent claude` is accepted; unknown names still return INVALID_AGENT.
- Argv is `-p`, prompt, and the claude yolo flag.
- Tests use a stub binary. Update README.

## Agent output

I'll start by listing the necro board, reading the task, and inspecting how `--agent grok` is implemented so Claude can follow the same watch path.I’m listing the necro board and searching the grok watch-agent path so Claude can follow the same contract.The necro board listing needs an idempotency key; I’m retrying that and reading the grok agent implementation.The board listing finished; I’m reading its output and the grok agent tests before starting the task.The task is in todo. I’m starting it and reading the remaining grok-agent wiring so the Claude path can match it.I’m adding failing tests for `--agent claude` first, then I’ll implement the smallest change that makes them pass.The tests are in. I’m running them now to confirm they fail on Claude not being a valid agent.RED is confirmed: `claude` still returns `INVALID_AGENT`. I’m adding the Claude agent kind and the yolo argv next.Implementation is in. I’m rerunning the Claude tests plus the unknown-agent case to confirm they pass.The CLI test mixed agent stdout with JSON. I’m reading the full output so I can parse the event the same way grok does.The focused tests passed. I’m running the full suite and formatting so nothing else broke.`--agent claude` is now a watch agent. It runs `claude -p <prompt> --dangerously-skip-permissions` in the board root and appends stdout under `## Agent output`, same as grok. Unknown names still return `INVALID_AGENT`.

**Phase:** implementation complete. Necro task `support-claude-as-watch-agent` is in `done`.

**Evidence:**
- Argv is `-p`, prompt, `--dangerously-skip-permissions` (no extra flags).
- Unit and CLI tests use a `claude` stub binary.
- README documents `--agent claude`.
- `cargo test` passed: 28 lib + 15 CLI + 22 store.

**Open:** none.
