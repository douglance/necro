# Number lane folders in pipeline order

Number `.necro` lane directories so lexical order matches the pipeline: `1-todo`, `2-doing`, `3-done`, `4-dropped`.

CLI and MCP status names stay `todo`, `doing`, `done`, `dropped`. Only the directory names change. `init` creates the numbered folders. Existing boards are migrated by rename. Tests assert folder names and that `list`/`start`/`done` still use the unnumbered status strings. Update README and CONTEXT.md.

## Done when

- Lane dirs are `1-todo`, `2-doing`, `3-done`, `4-dropped`.
- `Lane::as_str` / `--status` remain `todo` etc.
- `init` and board discovery accept the numbered names; old unnumbered names are renamed once.
- CLI tests prove file location after add/start/done/drop.

## Agent output

I'll start by listing the necro board, reading the task, and inspecting how lane folders are named today.The task is already on the board. I’ll start it, then inspect how lane folders are named and used.I’ll read the board, lane, and test code so the numbered folders can sit on the existing status strings.I’m gathering every place that hardcodes unnumbered lane folders so the rename stays complete.I’ll write the failing tests first so numbered folders and unnumbered status strings are the contract.I’m adding `Lane::dir_name` and updating tests to require numbered folders while status strings stay unnumbered.I’ll update the tests, CLI copy, docs, and watch stubs to use numbered folder names.I’m formatting the necro crate and running its tests to verify numbered lane folders.The format-and-test run needs an idempotency key; I’m retrying with that set.Tests passed. I’m installing the new necro binary so the live board can migrate, then I’ll mark the task done.Install validation needed an exit-code array; I’m retrying the install.The new binary is installed. I’m listing the doing lane so the live board migrates, then I’ll mark the task done.