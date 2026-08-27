# Verify necro harness against goal plan

Close the goal-plan gaps: CLI tests must assert lane files on disk; capture cargo test twice and real-binary init/add/list twice into the goal scratch dir; capture watch help.

Verified: CLI tests assert exclusive lane files after add/note/start/done/reopen/drop/next --start. cargo test passed twice. Real binary init/add/list --json twice wrote todo files. watch --help names --agent and --exec. Captures in goal scratch: cargo-test-1.log, cargo-test-2.log, cli-launch-1.log, cli-launch-2.log, necro-help.log.
