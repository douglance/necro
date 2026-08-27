# Add necro note

Append text to a task file without changing status.

## Done when

- REQ-NOTE-001 `necro note <id> <text>` appends the text to the task in any lane.
- REQ-NOTE-002 It does not rewrite the title or move the file.
- REQ-NOTE-003 Output is the full task record. CTA includes `show <id>`.
- REQ-NOTE-004 Missing ID fails with `TASK_NOT_FOUND`.

Implemented Store::note and necro note. cargo test passed.
