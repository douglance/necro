# Add necro snapshot of every lane

Add `necro snapshot` (or equivalent) that lists every task in todo, doing, done, and dropped in one result. Status remains folder membership. Do not add YAML.

## Done when

- One command returns all four lanes.
- CLI and MCP expose it.
- Tests create one task per lane and assert ids and statuses. Update README.

## Agent output

I'll follow the todo-lane process: read the task, start it with necro, then implement snapshot across CLI and MCP.I'll inspect how list currently works, then add snapshot the same way across CLI, MCP, and tests.I'll read the store, task types, and existing tests so snapshot can match the current list model.I have the board and the existing list/status model. Next I’ll start the necro task, then add snapshot with a failing test first.I'll write the failing snapshot tests first, then add the command so they pass.I'll add the snapshot tests first so they fail, then implement the command.I’m running the new snapshot tests to confirm they fail before any implementation.I’m rerunning the snapshot tests with a single cargo filter so the compile failure can show.The tests fail because `snapshot` is missing. I’ll add the store method, CLI/MCP command, and README next.I’m running the snapshot tests, then the full suite, to confirm the command returns all four lanes.**Outcome:** `necro snapshot` lists every task in todo, doing, done, and dropped in one result. Status is still folder membership. No YAML was added.

**Phase:** Implementation — complete. Task `add-necro-snapshot-of-every-lane` is in done.

**Evidence**
- CLI and MCP share the command through the incurs graph. `necro --llms` includes `` `necro snapshot` ``.
- Store and CLI tests create one task per lane and assert ids and statuses.
- `cargo test` passed.

**Usage**

```sh
necro snapshot
```

The result is the same shape as `list`: `{ tasks: [{ id, title, status, path }, ...] }` covering all four lanes.
