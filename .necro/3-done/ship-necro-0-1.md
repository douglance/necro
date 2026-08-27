# Ship necro 0.1

Prepare production ship: git repo and GitHub home, crate metadata, CI, gitignore, remove smoke files, changelog, README install plus durable watcher, cargo publish --dry-run.

I’m
 running cargo
 test
 and
 a crates
.io publish
 dry
-run to
 verify
 the 
0
.
1
 package
.
The
 Git
Hub
 repo
 exists
 and
 already
 has
 `
main
` plus
 `
v0.
1
.0`.
 I’ll
 fetch
,
 reconcile
 local
 ship
 edits
, and rer
un
 verification
.
I’m running
 cargo
 test and a
 crates
.io publish
 dry
-
run to verify
 the
 0.
1
 package
.
Tests
 passed;
 the publish
 dry
-
run failed
.
 I’m
 searching
 that
 log
 for
 the packaging
 error
.
The
 dry
-run
 failed
 because
 those
 crate
 files
 are still
 dirty
. I’ll commit
 the
 
0.1
 polish
,
 then rerun
 the
 publish
 check
.