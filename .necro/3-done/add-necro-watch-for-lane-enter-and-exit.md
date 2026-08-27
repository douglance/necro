# Add necro watch for lane enter and exit

Watch `.necro` lanes and trigger agents when a task file enters or leaves a folder.

## Done when

- `necro watch` polls lane directories and reports `enter` and `exit` events for valid task files.
- `--lane` and `--event` filter the stream. `--event` defaults to `enter`.
- `--once` waits for the first matching event and exits with that record.
- `--existing` emits `enter` for files already in the filtered lanes at start.
- `--exec <command>` runs the command with `NECRO_EVENT`, `NECRO_LANE`, `NECRO_ID`, `NECRO_PATH`, and `NECRO_ROOT`. One exec at a time.
- Status stays the parent folder. Do not add a database.

Implemented necro watch with --once, --existing, --lane, --event, and --exec. cargo test passed.
