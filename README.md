# necro

File-board task manager for agents. Each task is one Markdown file. Status is the parent lane. Starting a task is a rename from `.necro/1-todo` to `.necro/2-doing`. Completing it is a rename to `.necro/3-done`. Dropping it is a rename to `.necro/4-dropped`. CLI and MCP status names stay `todo`, `doing`, `done`, and `dropped`.

The same command graph is a CLI and an MCP server through [incurs](https://github.com/douglance/incurs). There is no database.

## Install

```sh
cargo install necro
# or, from git:
cargo install --git https://github.com/douglance/necro
```

Requires Rust 1.88+.

## Quick start

```sh
cd my-project
necro init
necro add "Fix auth refresh"
necro next
necro start fix-auth-refresh
necro note fix-auth-refresh "Checked the refresh path."
necro done fix-auth-refresh
necro snapshot
```

After `init` and `add`:

```text
.necro/1-todo/fix-auth-refresh.md
.necro/2-doing/
.necro/3-done/
.necro/4-dropped/
```

## Commands

| Command | Description |
| --- | --- |
| `necro init` | Create `.necro/1-todo`, `.necro/2-doing`, `.necro/3-done`, and `.necro/4-dropped` |
| `necro add <title>` | Write `.necro/1-todo/<slug>.md` |
| `necro list` | List open tasks. `--status todo\|doing\|done\|dropped\|all` |
| `necro snapshot` | List every task in todo, doing, done, and dropped |
| `necro next` | Return the first todo by ID. `--start` moves it to doing |
| `necro show <id>` | Read one task |
| `necro note <id> <text>` | Append Markdown to the task without changing status |
| `necro start <id>` | Move `.necro/1-todo/<id>.md` to `.necro/2-doing` |
| `necro done <id>` | Move a task from `todo` or `doing` to `.necro/3-done` |
| `necro drop <id>` | Move a task to `.necro/4-dropped` |
| `necro reopen <id>` | Move a task back to `.necro/1-todo` |
| `necro watch` | Stream enter/exit events. `--once` waits for one. `--agent grok` or `--agent claude` spawns a headless yolo agent. `--exec` runs a command |

`--root <path>` or `NECRO_ROOT` selects the board. Without them, commands other than `init` walk up from the working directory until the four lane directories exist.

Built-in agent surfaces:

```sh
necro --help
necro --llms
necro --mcp
necro mcp add
necro skills add
```

## Model

A **board** is a project directory that contains `.necro/1-todo`, `.necro/2-doing`, `.necro/3-done`, and `.necro/4-dropped`. A **task** is a `*.md` file whose name matches `^[a-z0-9]+(-[a-z0-9]+)*\.md$`. The **task ID** is the filename stem. **Status** is the lane (`todo`, `doing`, `done`, `dropped`), not the numbered folder prefix. Existing unnumbered folders (`todo`, `doing`, `done`, `dropped`) are renamed once on `init` or board discovery.

`add` slugifies the title. If that ID exists in any lane, necro appends `-2`, `-3`, and so on. Title for `list`, `snapshot`, and `show` is the first `# ` heading, or the ID when that heading is absent.

`start`, `done`, `drop`, and `reopen` are idempotent when the task is already in the destination lane. `start` rejects a task that is done or dropped. `done` rejects a dropped task. `next` only reads `todo`.

Each lane may contain `AGENTS.md`. That file is not a task. `show`, `next`, `start`, `done`, `drop`, `reopen`, `note`, and `watch` attach it as `agents` and `agents_path` for the task's current lane. `watch --exec` and `watch --agent` also set `NECRO_AGENTS_PATH` and `NECRO_AGENTS`. The board root may contain `AGENTS.md`. Watch `--agent` includes that file in the prompt when it is present. A missing board-root `AGENTS.md` does not fail the watch.

## Agent pipeline

One process creates todos. A watcher polls the lane folders and, for each enter, spawns a headless agent with `-p` in yolo mode. The agent prompt is the board-root `AGENTS.md` when that file exists, the lane `AGENTS.md`, and the task markdown.

```sh
# Worker: spawn a headless grok agent for each new todo
necro watch --lane todo --agent grok

# Worker: spawn a headless claude agent for each new todo
necro watch --lane todo --agent claude

# One-shot: same spawn, then exit
necro watch --once --existing --lane todo --agent grok
```

## Durable watcher

Run the watcher as a long-lived process, not a pane:

```sh
apoc execution start necro --cwd /path/to/project -- watch --lane todo --agent grok
```

`--agent` is yolo: grok uses `--always-approve`, claude uses `--dangerously-skip-permissions`. Only use it on a board you trust.

`--agent grok` runs `grok -p <prompt> --always-approve --output-format plain` in the board root and waits for that process before the next event. `--agent claude` runs `claude -p <prompt> --dangerously-skip-permissions` the same way. Unknown `--agent` names exit with `INVALID_AGENT`. A todo enter claims the task (`necro start`) before spawn. Between detecting the enter and spawning, necro re-checks that the file is still in todo; if it has already been started, done, or dropped, the spawn is skipped. As the agent prints, necro appends stdout to the task markdown as it arrives, following the file if it moved. A nonzero agent exit is still recorded that way, does not move the task to dropped, and does not stop a long-lived watcher. `--agent` and `--exec` cannot be combined.

Only one `--agent` watcher may run per board. It takes an exclusive lock at `.necro/agent-watch.lock`. A second `--agent` watcher exits with `AGENT_WATCHER_LOCKED`. `--exec` and event-only watch do not take this lock.

`--exec` receives `NECRO_EVENT`, `NECRO_LANE`, `NECRO_ID`, `NECRO_PATH`, and `NECRO_ROOT`. Without `--once`, `necro watch` keeps polling and streams every matching event. Default `--event` is `enter`. Use `--event exit` or `--event all` to watch files leaving a folder.

## Out of scope for 0.1

YAML frontmatter, a TUI, deleting task files, and git commits on move are not part of this version.
