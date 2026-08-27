use std::fs;

use necro::Error;
use necro::board;
use necro::store::Store;
use necro::task::{Lane, StatusFilter};

fn board_store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    board::init(dir.path()).unwrap();
    let store = Store::open(dir.path()).unwrap();
    (dir, store)
}

#[test]
fn add_list_show_roundtrip() {
    let (_dir, store) = board_store();
    let created = store.add("Fix auth refresh", None).unwrap();
    assert_eq!(created.id, "fix-auth-refresh");
    assert_eq!(created.status, Lane::Todo);
    assert_eq!(created.title, "Fix auth refresh");
    assert_eq!(created.body, "# Fix auth refresh\n");
    assert!(created.path.ends_with("/.necro/1-todo/fix-auth-refresh.md"));

    let listed = store.list(StatusFilter::Todo).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, "fix-auth-refresh");

    let shown = store.show("fix-auth-refresh").unwrap();
    assert_eq!(shown.body, created.body);
}

#[test]
fn show_loads_lane_agents_md() {
    let (dir, store) = board_store();
    store.add("With agents", None).unwrap();
    let empty = store.show("with-agents").unwrap();
    assert_eq!(empty.agents, None);
    fs::write(
        dir.path().join(".necro/1-todo/AGENTS.md"),
        "Start the task, then implement it.\n",
    )
    .unwrap();
    let shown = store.show("with-agents").unwrap();
    assert!(
        shown
            .agents_path
            .as_ref()
            .unwrap()
            .ends_with("/.necro/1-todo/AGENTS.md")
    );
    assert_eq!(
        shown.agents.as_deref(),
        Some("Start the task, then implement it.\n")
    );
    store.start("with-agents").unwrap();
    fs::write(
        dir.path().join(".necro/2-doing/AGENTS.md"),
        "Finish the in-progress work.\n",
    )
    .unwrap();
    let started = store.show("with-agents").unwrap();
    assert_eq!(
        started.agents.as_deref(),
        Some("Finish the in-progress work.\n")
    );
}

#[test]
fn list_ignores_agents_md() {
    let (dir, store) = board_store();
    store.add("Real task", None).unwrap();
    fs::write(dir.path().join(".necro/1-todo/AGENTS.md"), "instructions\n").unwrap();
    let listed = store.list(StatusFilter::Todo).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, "real-task");
}

#[test]
fn list_defaults_to_todo_and_all_includes_done() {
    let (_dir, store) = board_store();
    store.add("Open work", None).unwrap();
    store.add("Finished work", None).unwrap();
    store.done("finished-work").unwrap();

    let open = store.list(StatusFilter::Todo).unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].id, "open-work");

    let done = store.list(StatusFilter::Done).unwrap();
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].id, "finished-work");

    let all = store.list(StatusFilter::All).unwrap();
    assert_eq!(all.len(), 2);
}

#[test]
fn snapshot_lists_one_task_per_lane() {
    let (_dir, store) = board_store();
    store.add("Open work", None).unwrap();
    store.add("Active work", None).unwrap();
    store.add("Finished work", None).unwrap();
    store.add("Abandoned work", None).unwrap();
    store.start("active-work").unwrap();
    store.done("finished-work").unwrap();
    store.drop("abandoned-work").unwrap();

    let snap = store.snapshot().unwrap();
    assert_eq!(snap.len(), 4);
    let by_id: std::collections::BTreeMap<&str, Lane> = snap
        .iter()
        .map(|task| (task.id.as_str(), task.status))
        .collect();
    assert_eq!(by_id.get("open-work"), Some(&Lane::Todo));
    assert_eq!(by_id.get("active-work"), Some(&Lane::Doing));
    assert_eq!(by_id.get("finished-work"), Some(&Lane::Done));
    assert_eq!(by_id.get("abandoned-work"), Some(&Lane::Dropped));
}

#[test]
fn next_returns_first_todo_by_id_and_ignores_other_lanes() {
    let (_dir, store) = board_store();
    store.add("Zebra", None).unwrap();
    store.add("Alpha", None).unwrap();
    store.add("Middle", None).unwrap();
    store.start("middle").unwrap();
    store.done("zebra").unwrap();
    let next = store.next().unwrap().unwrap();
    assert_eq!(next.id, "alpha");
    assert_eq!(next.status, Lane::Todo);
}

#[test]
fn next_empty_todo_is_none() {
    let (_dir, store) = board_store();
    assert!(store.next().unwrap().is_none());
}

#[test]
fn note_appends_without_moving() {
    let (_dir, store) = board_store();
    store.add("Keep title", None).unwrap();
    let noted = store
        .note("keep-title", "Checked the refresh path.")
        .unwrap();
    assert_eq!(noted.status, Lane::Todo);
    assert_eq!(noted.title, "Keep title");
    assert_eq!(noted.body, "# Keep title\n\nChecked the refresh path.\n");
    assert!(noted.path.ends_with("/.necro/1-todo/keep-title.md"));
}

#[test]
fn note_missing_id_is_task_not_found() {
    let (_dir, store) = board_store();
    match store.note("missing", "nope") {
        Err(Error::TaskNotFound(id)) => assert_eq!(id, "missing"),
        other => panic!("expected TASK_NOT_FOUND, got {other:?}"),
    }
}

#[test]
fn start_moves_todo_to_doing_and_is_idempotent() {
    let (_dir, store) = board_store();
    store.add("In progress", None).unwrap();
    let started = store.start("in-progress").unwrap();
    assert_eq!(started.status, Lane::Doing);
    assert!(started.path.ends_with("/.necro/2-doing/in-progress.md"));
    assert_eq!(store.list(StatusFilter::Todo).unwrap().len(), 0);
    assert_eq!(store.list(StatusFilter::Doing).unwrap().len(), 1);

    let again = store.start("in-progress").unwrap();
    assert_eq!(again.status, Lane::Doing);
}

#[test]
fn claim_moves_todo_and_skips_when_already_doing() {
    let (_dir, store) = board_store();
    store.add("In progress", None).unwrap();
    let claimed = store.claim("in-progress").unwrap().unwrap();
    assert_eq!(claimed.status, Lane::Doing);
    assert!(claimed.path.ends_with("/.necro/2-doing/in-progress.md"));
    assert_eq!(store.list(StatusFilter::Todo).unwrap().len(), 0);
    assert_eq!(store.list(StatusFilter::Doing).unwrap().len(), 1);

    assert!(store.claim("in-progress").unwrap().is_none());
    assert_eq!(store.list(StatusFilter::Doing).unwrap().len(), 1);
}

#[test]
fn done_accepts_doing_or_todo() {
    let (_dir, store) = board_store();
    store.add("From doing", None).unwrap();
    store.start("from-doing").unwrap();
    let finished = store.done("from-doing").unwrap();
    assert_eq!(finished.status, Lane::Done);

    store.add("From todo", None).unwrap();
    let also = store.done("from-todo").unwrap();
    assert_eq!(also.status, Lane::Done);
}

#[test]
fn drop_moves_to_dropped_and_is_idempotent() {
    let (_dir, store) = board_store();
    store.add("Abandon me", None).unwrap();
    let dropped = store.drop("abandon-me").unwrap();
    assert_eq!(dropped.status, Lane::Dropped);
    assert!(dropped.path.ends_with("/.necro/4-dropped/abandon-me.md"));
    assert_eq!(store.list(StatusFilter::Todo).unwrap().len(), 0);
    assert_eq!(store.list(StatusFilter::Dropped).unwrap().len(), 1);
    let again = store.drop("abandon-me").unwrap();
    assert_eq!(again.status, Lane::Dropped);
    let reopened = store.reopen("abandon-me").unwrap();
    assert_eq!(reopened.status, Lane::Todo);
}

#[test]
fn start_and_done_reject_dropped_tasks() {
    let (_dir, store) = board_store();
    store.add("No", None).unwrap();
    store.drop("no").unwrap();
    match store.start("no") {
        Err(Error::AlreadyDropped(id)) => assert_eq!(id, "no"),
        other => panic!("expected ALREADY_DROPPED from start, got {other:?}"),
    }
    match store.done("no") {
        Err(Error::AlreadyDropped(id)) => assert_eq!(id, "no"),
        other => panic!("expected ALREADY_DROPPED from done, got {other:?}"),
    }
}

#[test]
fn start_rejects_a_done_task() {
    let (_dir, store) = board_store();
    store.add("Finished", None).unwrap();
    store.done("finished").unwrap();
    match store.start("finished") {
        Err(Error::AlreadyDone(id)) => assert_eq!(id, "finished"),
        other => panic!("expected ALREADY_DONE, got {other:?}"),
    }
}

#[test]
fn done_renames_without_rewriting_body() {
    let (_dir, store) = board_store();
    store.add("Keep body", Some("do not rewrite me")).unwrap();
    let moved = store.done("keep-body").unwrap();
    assert_eq!(moved.status, Lane::Done);
    assert!(moved.path.ends_with("/.necro/3-done/keep-body.md"));
    assert_eq!(moved.body, "# Keep body\n\ndo not rewrite me\n");
    assert!(!store.root().join(".necro/1-todo/keep-body.md").exists());
}

#[test]
fn done_is_idempotent_when_already_done() {
    let (_dir, store) = board_store();
    store.add("Once", None).unwrap();
    store.done("once").unwrap();
    let again = store.done("once").unwrap();
    assert_eq!(again.status, Lane::Done);
}

#[test]
fn reopen_returns_to_todo() {
    let (_dir, store) = board_store();
    store.add("Cycle", None).unwrap();
    store.done("cycle").unwrap();
    let open = store.reopen("cycle").unwrap();
    assert_eq!(open.status, Lane::Todo);
    assert!(open.path.ends_with("/.necro/1-todo/cycle.md"));
}

#[test]
fn missing_id_is_task_not_found() {
    let (_dir, store) = board_store();
    match store.show("missing") {
        Err(Error::TaskNotFound(id)) => assert_eq!(id, "missing"),
        other => panic!("expected TASK_NOT_FOUND, got {other:?}"),
    }
}

#[test]
fn slug_collision_appends_number() {
    let (_dir, store) = board_store();
    let first = store.add("Same Title", None).unwrap();
    let second = store.add("Same Title", None).unwrap();
    assert_eq!(first.id, "same-title");
    assert_eq!(second.id, "same-title-2");
}

#[test]
fn destination_conflict_leaves_source() {
    let (dir, store) = board_store();
    store.add("Clash", None).unwrap();
    fs::write(dir.path().join(".necro/3-done/clash.md"), "# other\n").unwrap();
    match store.done("clash") {
        Err(Error::Conflict(id)) => assert_eq!(id, "clash"),
        other => panic!("expected CONFLICT, got {other:?}"),
    }
    assert!(dir.path().join(".necro/1-todo/clash.md").exists());
}

#[test]
fn init_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let first = board::init(dir.path()).unwrap();
    assert_eq!(
        first.created,
        vec![
            ".necro",
            ".necro/1-todo",
            ".necro/2-doing",
            ".necro/3-done",
            ".necro/4-dropped",
        ]
    );
    let second = board::init(dir.path()).unwrap();
    assert!(second.created.is_empty());
}

#[test]
fn open_renames_legacy_unnumbered_lane_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let necro = dir.path().join(".necro");
    for name in ["todo", "doing", "done", "dropped"] {
        fs::create_dir_all(necro.join(name)).unwrap();
    }
    fs::write(necro.join("todo/legacy.md"), "# Legacy\n").unwrap();
    let store = Store::open(dir.path()).unwrap();
    assert!(necro.join("1-todo").is_dir());
    assert!(!necro.join("todo").exists());
    assert!(!necro.join("doing").exists());
    let shown = store.show("legacy").unwrap();
    assert_eq!(shown.status, Lane::Todo);
    assert!(shown.path.ends_with("/.necro/1-todo/legacy.md"));
}
