# Add a doing lane

Iteration 1 has `.necro/todo` and `.necro/done`. Agents need an in-progress lane so a claimed task is not still listed as open work.

## Done when

- `necro init` creates `.necro/doing` with the existing lanes.
- `necro start <id>` moves a task from `todo` to `doing`. Repeating it on a task already in `doing` succeeds.
- `necro done <id>` accepts a task from `doing` or `todo`.
- `necro list` still defaults to `todo`. `--status doing` and `--status all` include the new lane.
- Status remains the parent folder. Do not store status in YAML or the filename.
