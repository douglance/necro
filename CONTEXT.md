# Necro domain context

| Field | Value |
| --- | --- |
| Status | Active terminology registry |
| Audience | Necro maintainers and agent authors |
| Outcome | Keep task status in folder membership, not in document metadata |
| Writing profile | Technical and developer documentation guided by UWDS 1.0 |

## Controlling distinction

A task is a Markdown file. Its identity is the filename stem. Its status is the parent lane directory (`1-todo`, `2-doing`, `3-done`, or `4-dropped`). Moving the file is the status change.

Do not encode status in YAML, filenames, or a sidecar index.

## Preferred terms

| Term | Definition |
| --- | --- |
| Board | Project directory that contains `.necro/1-todo`, `.necro/2-doing`, `.necro/3-done`, and `.necro/4-dropped` |
| Lane | One of `todo`, `doing`, `done`, or `dropped`. Folder names are `1-todo`, `2-doing`, `3-done`, `4-dropped` so lexical order matches the pipeline |
| Task | One `*.md` file in a lane |
| Task ID | Filename stem. Example: `fix-auth` for `.necro/1-todo/fix-auth.md` |
| Move | POSIX `rename` of a task file from one lane to the other |
| Note | Markdown text appended to a task file without changing status |
| Next | The first `todo` task by ascending ID |
| Snapshot | One listing of every task in todo, doing, done, and dropped |
| Enter | A valid task file appearing in a lane directory |
| Exit | A valid task file leaving a lane directory |
| Watch | Poll of lane directories that emits enter and exit events and can run `--exec` or spawn `--agent grok` (`-p` plus `--always-approve`) or `--agent claude` (`-p` plus `--dangerously-skip-permissions`). A todo enter with `--agent` claims the task before spawn. Between detecting that enter and spawning, the watcher re-checks that the file is still in todo and skips spawn if it has been started, done, or dropped. Agent stdout is appended to the task file as it arrives, even when the process later exits nonzero; the task is not dropped and the watcher keeps polling. One `--agent` watcher per board takes an exclusive lock at `.necro/agent-watch.lock`; `--exec` and event-only watch do not |
| Lane AGENTS.md | Optional instruction file in a lane folder, read with the task markdown |
| Board AGENTS.md | Optional instruction file at the board root (`./AGENTS.md`). The watch agent prompt includes it when present |
| Root | Board directory. Default: walk up from the process working directory. Override: `--root` or `NECRO_ROOT` |

Do not use `pending`, `complete`, `column`, or `issue` as product terms.

## Lanes

`todo` holds open work. `doing` holds claimed in-progress work. `done` holds finished work. `dropped` holds abandoned work. Those names are CLI and MCP status strings. On disk they are `.necro/1-todo`, `.necro/2-doing`, `.necro/3-done`, and `.necro/4-dropped`. Unnumbered folders from earlier boards are renamed once.
