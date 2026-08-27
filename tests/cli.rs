use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use necro::build_cli;
use necro::task::Lane;
use serde_json::Value;

async fn run_json(args: &[&str]) -> (Option<i32>, Value) {
    let argv = args.iter().map(|s| s.to_string()).collect();
    let mut output = Vec::new();
    let exit = build_cli()
        .serve_to(argv, &mut output, false)
        .await
        .unwrap();
    let value = serde_json::from_slice(&output)
        .unwrap_or_else(|_| panic!("expected JSON, got: {}", String::from_utf8_lossy(&output)));
    (exit, value)
}

fn root_args<'a>(root: &'a str, command: &'a [&'a str]) -> Vec<&'a str> {
    let mut args = vec!["--root", root];
    args.extend_from_slice(command);
    args.push("--json");
    args
}

fn lane_file(root: &str, lane: &str, id: &str) -> PathBuf {
    let dir = Lane::parse(lane).expect("status string").dir_name();
    Path::new(root)
        .join(".necro")
        .join(dir)
        .join(format!("{id}.md"))
}

fn assert_only_in(root: &str, id: &str, lane: &str) {
    for candidate in Lane::all() {
        let path = lane_file(root, candidate.as_str(), id);
        if candidate.as_str() == lane {
            assert!(path.is_file(), "expected {path:?}");
        } else {
            assert!(!path.exists(), "did not expect {path:?}");
        }
    }
}

#[tokio::test]
async fn init_add_list_done_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap();

    let (exit, init) = run_json(&root_args(root, &["init"])).await;
    assert_eq!(exit, None);
    assert_eq!(
        init["created"],
        serde_json::json!([
            ".necro",
            ".necro/1-todo",
            ".necro/2-doing",
            ".necro/3-done",
            ".necro/4-dropped"
        ])
    );
    for lane in Lane::all() {
        assert!(
            Path::new(root)
                .join(".necro")
                .join(lane.dir_name())
                .is_dir()
        );
        assert!(!Path::new(root).join(".necro").join(lane.as_str()).exists());
    }

    let (exit, added) = run_json(&root_args(root, &["add", "Fix auth refresh"])).await;
    assert_eq!(exit, None);
    assert_eq!(added["id"], "fix-auth-refresh");
    assert_eq!(added["status"], "todo");
    assert_only_in(root, "fix-auth-refresh", "todo");
    let added_body = std::fs::read_to_string(lane_file(root, "todo", "fix-auth-refresh")).unwrap();
    assert_eq!(added_body, "# Fix auth refresh\n");
    assert!(!added_body.contains("status:"));

    let (exit, listed) = run_json(&root_args(root, &["list"])).await;
    assert_eq!(exit, None);
    assert_eq!(listed["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(listed["tasks"][0]["id"], "fix-auth-refresh");
    assert_eq!(listed["tasks"][0]["status"], "todo");

    let (exit, shown) = run_json(&root_args(root, &["show", "fix-auth-refresh"])).await;
    assert_eq!(exit, None);
    assert_eq!(shown["title"], "Fix auth refresh");

    let (exit, noted) = run_json(&root_args(
        root,
        &["note", "fix-auth-refresh", "Checked the token path."],
    ))
    .await;
    assert_eq!(exit, None);
    assert_eq!(noted["status"], "todo");
    assert!(
        noted["body"]
            .as_str()
            .unwrap()
            .contains("Checked the token path.")
    );
    assert_only_in(root, "fix-auth-refresh", "todo");
    let noted_body = std::fs::read_to_string(lane_file(root, "todo", "fix-auth-refresh")).unwrap();
    assert!(noted_body.starts_with("# Fix auth refresh\n"));
    assert!(noted_body.contains("Checked the token path."));
    assert!(!noted_body.contains("status:"));

    let (exit, started) = run_json(&root_args(root, &["start", "fix-auth-refresh"])).await;
    assert_eq!(exit, None);
    assert_eq!(started["status"], "doing");
    assert_only_in(root, "fix-auth-refresh", "doing");

    let (exit, doing) = run_json(&root_args(root, &["list", "--status", "doing"])).await;
    assert_eq!(exit, None);
    assert_eq!(doing["tasks"].as_array().unwrap().len(), 1);

    let (exit, done) = run_json(&root_args(root, &["done", "fix-auth-refresh"])).await;
    assert_eq!(exit, None);
    assert_eq!(done["status"], "done");
    assert_only_in(root, "fix-auth-refresh", "done");

    let (exit, open) = run_json(&root_args(root, &["list"])).await;
    assert_eq!(exit, None);
    assert_eq!(open["tasks"].as_array().unwrap().len(), 0);

    let (exit, finished) = run_json(&root_args(root, &["list", "--status", "done"])).await;
    assert_eq!(exit, None);
    assert_eq!(finished["tasks"].as_array().unwrap().len(), 1);

    let (exit, reopened) = run_json(&root_args(root, &["reopen", "fix-auth-refresh"])).await;
    assert_eq!(exit, None);
    assert_eq!(reopened["status"], "todo");
    assert_only_in(root, "fix-auth-refresh", "todo");

    let (exit, dropped) = run_json(&root_args(root, &["drop", "fix-auth-refresh"])).await;
    assert_eq!(exit, None);
    assert_eq!(dropped["status"], "dropped");
    assert_only_in(root, "fix-auth-refresh", "dropped");
}

#[tokio::test]
async fn list_start_done_migrate_legacy_unnumbered_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap();
    let necro = Path::new(root).join(".necro");
    for name in ["todo", "doing", "done", "dropped"] {
        std::fs::create_dir_all(necro.join(name)).unwrap();
    }
    std::fs::write(necro.join("todo/legacy-task.md"), "# Legacy task\n").unwrap();

    let (exit, listed) = run_json(&root_args(root, &["list"])).await;
    assert_eq!(exit, None);
    assert_eq!(listed["tasks"][0]["id"], "legacy-task");
    assert_eq!(listed["tasks"][0]["status"], "todo");
    assert_only_in(root, "legacy-task", "todo");
    assert!(necro.join("1-todo").is_dir());
    assert!(!necro.join("todo").exists());

    let (exit, started) = run_json(&root_args(root, &["start", "legacy-task"])).await;
    assert_eq!(exit, None);
    assert_eq!(started["status"], "doing");
    assert_only_in(root, "legacy-task", "doing");

    let (exit, done) = run_json(&root_args(root, &["done", "legacy-task"])).await;
    assert_eq!(exit, None);
    assert_eq!(done["status"], "done");
    assert_only_in(root, "legacy-task", "done");

    let (exit, dropped) = run_json(&root_args(root, &["drop", "legacy-task"])).await;
    assert_eq!(exit, None);
    assert_eq!(dropped["status"], "dropped");
    assert_only_in(root, "legacy-task", "dropped");
}

#[tokio::test]
async fn snapshot_lists_one_task_per_lane() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap();
    let _ = run_json(&root_args(root, &["init"])).await;
    let _ = run_json(&root_args(root, &["add", "Open work"])).await;
    let _ = run_json(&root_args(root, &["add", "Active work"])).await;
    let _ = run_json(&root_args(root, &["add", "Finished work"])).await;
    let _ = run_json(&root_args(root, &["add", "Abandoned work"])).await;
    let _ = run_json(&root_args(root, &["start", "active-work"])).await;
    let _ = run_json(&root_args(root, &["done", "finished-work"])).await;
    let _ = run_json(&root_args(root, &["drop", "abandoned-work"])).await;
    assert_only_in(root, "open-work", "todo");
    assert_only_in(root, "active-work", "doing");
    assert_only_in(root, "finished-work", "done");
    assert_only_in(root, "abandoned-work", "dropped");

    let (exit, snap) = run_json(&root_args(root, &["snapshot"])).await;
    assert_eq!(exit, None);
    let tasks = snap["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 4);
    let by_id: std::collections::BTreeMap<&str, &str> = tasks
        .iter()
        .map(|task| {
            (
                task["id"].as_str().unwrap(),
                task["status"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(by_id.get("open-work"), Some(&"todo"));
    assert_eq!(by_id.get("active-work"), Some(&"doing"));
    assert_eq!(by_id.get("finished-work"), Some(&"done"));
    assert_eq!(by_id.get("abandoned-work"), Some(&"dropped"));
    for task in tasks {
        assert!(!task.as_object().unwrap().contains_key("body"));
    }
}

#[tokio::test]
async fn next_returns_first_todo_or_null() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap();
    let _ = run_json(&root_args(root, &["init"])).await;
    let (exit, empty) = run_json(&root_args(root, &["next"])).await;
    assert_eq!(exit, None);
    assert!(empty["task"].is_null());

    let _ = run_json(&root_args(root, &["add", "Zebra"])).await;
    let _ = run_json(&root_args(root, &["add", "Alpha"])).await;
    assert_only_in(root, "zebra", "todo");
    assert_only_in(root, "alpha", "todo");
    let (exit, next) = run_json(&root_args(root, &["next"])).await;
    assert_eq!(exit, None);
    assert_eq!(next["task"]["id"], "alpha");
    assert_eq!(next["task"]["status"], "todo");
    assert_only_in(root, "alpha", "todo");

    let (exit, started) = run_json(&root_args(root, &["next", "--start"])).await;
    assert_eq!(exit, None);
    assert_eq!(started["task"]["id"], "alpha");
    assert_eq!(started["task"]["status"], "doing");
    assert_only_in(root, "alpha", "doing");
    assert_only_in(root, "zebra", "todo");
}

#[tokio::test]
async fn missing_board_returns_board_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap();
    let (exit, value) = run_json(&root_args(root, &["list"])).await;
    assert_eq!(exit, Some(1));
    let code = value
        .pointer("/error/code")
        .or_else(|| value.get("code"))
        .cloned();
    assert_eq!(code, Some(Value::String("BOARD_NOT_FOUND".into())));
}

#[tokio::test]
async fn missing_task_returns_task_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap();
    let _ = run_json(&root_args(root, &["init"])).await;
    let (exit, value) = run_json(&root_args(root, &["show", "nope"])).await;
    assert_eq!(exit, Some(1));
    let code = value
        .pointer("/error/code")
        .or_else(|| value.get("code"))
        .cloned();
    assert_eq!(code, Some(Value::String("TASK_NOT_FOUND".into())));
}

#[tokio::test]
async fn show_includes_lane_agents_md() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap();
    let _ = run_json(&root_args(root, &["init"])).await;
    std::fs::write(
        dir.path().join(".necro/1-todo/AGENTS.md"),
        "Use the task file in this folder.\n",
    )
    .unwrap();
    let _ = run_json(&root_args(root, &["add", "Guided work"])).await;
    let (exit, shown) = run_json(&root_args(root, &["show", "guided-work"])).await;
    assert_eq!(exit, None);
    assert_eq!(shown["agents"], "Use the task file in this folder.\n");
}

#[tokio::test]
async fn watch_once_existing_returns_enter() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap();
    let _ = run_json(&root_args(root, &["init"])).await;
    let _ = run_json(&root_args(root, &["add", "Watch me"])).await;
    let (exit, value) = run_json(&root_args(
        root,
        &[
            "watch",
            "--once",
            "--existing",
            "--lane",
            "todo",
            "--interval-ms",
            "20",
        ],
    ))
    .await;
    assert_eq!(exit, None);
    assert_eq!(value["event"], "enter");
    assert_eq!(value["lane"], "todo");
    assert_eq!(value["id"], "watch-me");
    assert_only_in(root, "watch-me", "todo");
}

#[tokio::test]
async fn watch_exec_runs_on_existing_enter() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap();
    let _ = run_json(&root_args(root, &["init"])).await;
    let _ = run_json(&root_args(root, &["add", "Watch me"])).await;
    let (exit, value) = run_json(&root_args(
        root,
        &[
            "watch",
            "--once",
            "--existing",
            "--lane",
            "todo",
            "--exec",
            "printf '%s' \"$NECRO_ID\" > \"$NECRO_ROOT/saw.txt\"",
            "--interval-ms",
            "20",
        ],
    ))
    .await;
    assert_eq!(exit, None);
    assert_eq!(value["event"], "enter");
    assert_eq!(value["id"], "watch-me");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("saw.txt")).unwrap(),
        "watch-me"
    );
    assert_only_in(root, "watch-me", "todo");
}

fn write_claude_stub(dir: &Path) -> PathBuf {
    let bin_dir = dir.join("bin");
    std::fs::create_dir(&bin_dir).unwrap();
    let claude = bin_dir.join("claude");
    std::fs::write(
        &claude,
        r#"#!/bin/sh
i=1
for arg in "$@"; do
  printf '%s' "$arg" > "$NECRO_ROOT/agent-arg-$i.txt"
  i=$((i+1))
done
printf '%s\n' 'hello from claude'
"#,
    )
    .unwrap();
    chmod_exec(&claude);
    bin_dir
}

#[tokio::test]
async fn watch_accepts_claude_agent() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap();
    let _ = run_json(&root_args(root, &["init"])).await;
    let _ = run_json(&root_args(root, &["add", "Claude job"])).await;
    let bin_dir = write_claude_stub(dir.path());
    let path = match std::env::var("PATH") {
        Ok(rest) => format!("{}:{rest}", bin_dir.display()),
        Err(_) => bin_dir.display().to_string(),
    };
    let bin = env!("CARGO_BIN_EXE_necro");
    let output = Command::new(bin)
        .args([
            "--root",
            root,
            "watch",
            "--once",
            "--existing",
            "--lane",
            "todo",
            "--agent",
            "claude",
            "--interval-ms",
            "20",
            "--json",
        ])
        .env("PATH", path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = stdout.find('{').map(|i| &stdout[i..]).unwrap_or(&stdout);
    let value: Value =
        serde_json::from_str(json).unwrap_or_else(|_| panic!("expected JSON, got: {stdout}"));
    assert_eq!(value["event"], "enter");
    assert_eq!(value["id"], "claude-job");
    assert_eq!(value["agent_output"], "hello from claude");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("agent-arg-1.txt")).unwrap(),
        "-p"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("agent-arg-3.txt")).unwrap(),
        "--dangerously-skip-permissions"
    );
    assert!(!dir.path().join("agent-arg-4.txt").exists());
    let body = std::fs::read_to_string(lane_file(root, "doing", "claude-job")).unwrap();
    assert!(body.contains("hello from claude\n"));
    assert!(!body.contains("## Agent output"));
}

#[tokio::test]
async fn watch_rejects_unknown_agent() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap();
    let _ = run_json(&root_args(root, &["init"])).await;
    let (exit, value) = run_json(&root_args(
        root,
        &["watch", "--once", "--agent", "nope", "--interval-ms", "20"],
    ))
    .await;
    assert_eq!(exit, Some(1));
    let code = value
        .pointer("/error/code")
        .or_else(|| value.get("code"))
        .cloned();
    assert_eq!(code, Some(Value::String("INVALID_AGENT".into())));
}

#[tokio::test]
async fn watch_rejects_agent_and_exec() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap();
    let _ = run_json(&root_args(root, &["init"])).await;
    let (exit, value) = run_json(&root_args(
        root,
        &[
            "watch",
            "--once",
            "--agent",
            "grok",
            "--exec",
            "true",
            "--interval-ms",
            "20",
        ],
    ))
    .await;
    assert_eq!(exit, Some(1));
    let code = value
        .pointer("/error/code")
        .or_else(|| value.get("code"))
        .cloned();
    assert_eq!(code, Some(Value::String("AGENT_AND_EXEC".into())));
}

struct AgentWatcher {
    child: std::process::Child,
}

impl Drop for AgentWatcher {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn spawn_agent_watcher(root: &str) -> AgentWatcher {
    spawn_agent_watcher_path(root, None)
}

fn spawn_agent_watcher_path(root: &str, path_prefix: Option<&Path>) -> AgentWatcher {
    let bin = env!("CARGO_BIN_EXE_necro");
    let stdout = std::fs::File::create(Path::new(root).join("watcher.stdout")).unwrap();
    let stderr = std::fs::File::create(Path::new(root).join("watcher.stderr")).unwrap();
    let mut command = Command::new(bin);
    command
        .args([
            "--root",
            root,
            "watch",
            "--lane",
            "todo",
            "--agent",
            "grok",
            "--interval-ms",
            "50",
            "--json",
        ])
        .stdout(stdout)
        .stderr(stderr);
    if let Some(prefix) = path_prefix {
        let path = match std::env::var("PATH") {
            Ok(rest) => format!("{}:{rest}", prefix.display()),
            Err(_) => prefix.display().to_string(),
        };
        command.env("PATH", path);
    }
    AgentWatcher {
        child: command.spawn().unwrap(),
    }
}

fn watcher_logs(root: &str) -> String {
    let stdout =
        std::fs::read_to_string(Path::new(root).join("watcher.stdout")).unwrap_or_default();
    let stderr =
        std::fs::read_to_string(Path::new(root).join("watcher.stderr")).unwrap_or_default();
    format!("stdout:\n{stdout}\nstderr:\n{stderr}")
}

fn error_code(value: &Value) -> Option<String> {
    value
        .pointer("/error/code")
        .or_else(|| value.get("code"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

async fn second_agent_watch(root: &str) -> (Option<i32>, Value) {
    run_json(&root_args(
        root,
        &[
            "watch",
            "--once",
            "--lane",
            "todo",
            "--agent",
            "grok",
            "--interval-ms",
            "20",
        ],
    ))
    .await
}

async fn wait_for_agent_lock(root: &str, holder: &mut AgentWatcher) -> (Option<i32>, Value) {
    let lock_path = Path::new(root).join(".necro/agent-watch.lock");
    let start = Instant::now();
    while !lock_path.exists() {
        if let Some(status) = holder.child.try_wait().unwrap() {
            panic!(
                "holder exited before creating the lock ({status}). {}",
                watcher_logs(root)
            );
        }
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "holder did not create the lock file. {}",
            watcher_logs(root)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    loop {
        if let Some(status) = holder.child.try_wait().unwrap() {
            panic!(
                "holder exited while locking ({status}). {}",
                watcher_logs(root)
            );
        }
        match tokio::time::timeout(Duration::from_millis(400), second_agent_watch(root)).await {
            Ok((exit, value)) if error_code(&value).as_deref() == Some("AGENT_WATCHER_LOCKED") => {
                return (exit, value);
            }
            Ok((exit, value)) => {
                panic!(
                    "expected AGENT_WATCHER_LOCKED, got {exit:?} {value}. {}",
                    watcher_logs(root)
                );
            }
            Err(_) => {
                assert!(
                    start.elapsed() < Duration::from_secs(3),
                    "second --agent watcher kept waiting instead of failing with AGENT_WATCHER_LOCKED. {}",
                    watcher_logs(root)
                );
            }
        }
    }
}

#[tokio::test]
async fn watch_agent_second_watcher_is_locked() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap();
    let _ = run_json(&root_args(root, &["init"])).await;
    let mut holder = spawn_agent_watcher(root);
    let (exit, value) = wait_for_agent_lock(root, &mut holder).await;
    assert_eq!(exit, Some(1));
    assert_eq!(error_code(&value), Some("AGENT_WATCHER_LOCKED".to_string()));
}

#[tokio::test]
async fn watch_exec_does_not_require_agent_lock() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap();
    let _ = run_json(&root_args(root, &["init"])).await;
    let _ = run_json(&root_args(root, &["add", "Watch me"])).await;
    let mut holder = spawn_agent_watcher(root);
    let (locked_exit, _) = wait_for_agent_lock(root, &mut holder).await;
    assert_eq!(locked_exit, Some(1));
    let (exit, value) = run_json(&root_args(
        root,
        &[
            "watch",
            "--once",
            "--existing",
            "--lane",
            "todo",
            "--exec",
            "printf '%s' \"$NECRO_ID\" > \"$NECRO_ROOT/saw.txt\"",
            "--interval-ms",
            "20",
        ],
    ))
    .await;
    assert_eq!(exit, None);
    assert_eq!(value["event"], "enter");
    assert_eq!(value["id"], "watch-me");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("saw.txt")).unwrap(),
        "watch-me"
    );
}

#[tokio::test]
async fn watch_event_only_does_not_require_agent_lock() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap();
    let _ = run_json(&root_args(root, &["init"])).await;
    let _ = run_json(&root_args(root, &["add", "Watch me"])).await;
    let mut holder = spawn_agent_watcher(root);
    let (locked_exit, _) = wait_for_agent_lock(root, &mut holder).await;
    assert_eq!(locked_exit, Some(1));
    let (exit, value) = run_json(&root_args(
        root,
        &[
            "watch",
            "--once",
            "--existing",
            "--lane",
            "todo",
            "--interval-ms",
            "20",
        ],
    ))
    .await;
    assert_eq!(exit, None);
    assert_eq!(value["event"], "enter");
    assert_eq!(value["id"], "watch-me");
    assert_only_in(root, "watch-me", "todo");
}

fn chmod_exec(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).unwrap();
    }
}

fn write_failing_grok(dir: &Path) -> PathBuf {
    let bin_dir = dir.join("bin");
    std::fs::create_dir(&bin_dir).unwrap();
    let grok = bin_dir.join("grok");
    std::fs::write(
        &grok,
        r#"#!/bin/sh
printf '%s\n' "failed-$NECRO_ID"
echo "$NECRO_ID" >> "$NECRO_ROOT/agent-runs.txt"
exit 2
"#,
    )
    .unwrap();
    chmod_exec(&grok);
    bin_dir
}

async fn wait_for(root: &str, label: &str, mut ready: impl FnMut() -> bool) {
    let start = Instant::now();
    while !ready() {
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "{label}. {}",
            watcher_logs(root)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn watch_agent_nonzero_exit_notes_keeps_task_and_keeps_watching() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap();
    let _ = run_json(&root_args(root, &["init"])).await;
    let bin_dir = write_failing_grok(dir.path());
    let mut holder = spawn_agent_watcher_path(root, Some(&bin_dir));
    let lock_path = Path::new(root).join(".necro/agent-watch.lock");
    wait_for(root, "holder did not create the lock file", || {
        lock_path.exists()
    })
    .await;
    if let Some(status) = holder.child.try_wait().unwrap() {
        panic!(
            "holder exited before events ({status}). {}",
            watcher_logs(root)
        );
    }

    let _ = run_json(&root_args(root, &["add", "First fail"])).await;
    wait_for(root, "first failing agent run was not noted", || {
        let path = lane_file(root, "doing", "first-fail");
        path.is_file()
            && std::fs::read_to_string(path)
                .unwrap_or_default()
                .contains("failed-first-fail\n")
    })
    .await;
    assert_only_in(root, "first-fail", "doing");

    let _ = run_json(&root_args(root, &["add", "Second fail"])).await;
    wait_for(
        root,
        "watcher did not handle a later event after the failure",
        || {
            let path = lane_file(root, "doing", "second-fail");
            path.is_file()
                && std::fs::read_to_string(path)
                    .unwrap_or_default()
                    .contains("failed-second-fail\n")
        },
    )
    .await;
    assert_only_in(root, "second-fail", "doing");
    assert!(
        holder.child.try_wait().unwrap().is_none(),
        "watcher exited after the failed agent run. {}",
        watcher_logs(root)
    );
    let runs = std::fs::read_to_string(dir.path().join("agent-runs.txt")).unwrap();
    assert!(runs.contains("first-fail"));
    assert!(runs.contains("second-fail"));
}
