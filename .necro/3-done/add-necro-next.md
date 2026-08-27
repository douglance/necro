# Add necro next

Return the next open todo so an agent does not pick from a list.

## Done when

- REQ-NEXT-001 `necro next` returns the first todo by ascending ID, or `{ task: null }` when empty.
- REQ-NEXT-002 `--start` moves that todo to doing. Empty todo still returns null and succeeds.
- REQ-NEXT-003 Output is `{ task }`. CTA includes `show` and `start` unless already started.
- REQ-NEXT-004 It does not scan doing, done, or dropped.

Implemented necro next and next --start. cargo test passed.
