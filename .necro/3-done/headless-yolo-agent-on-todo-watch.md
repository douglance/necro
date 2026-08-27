# Headless yolo agent on todo watch

Replace interactive watcher agents with a file watcher that launches a headless agent via `-p` in yolo mode when a task enters a lane.

## Done when

- `necro watch --exec` can invoke an agent with `-p` and skip approvals (yolo).
- The prompt includes the lane AGENTS.md and the task markdown.
- A watcher can be run as a long-lived process that spawns one headless agent per enter event.
