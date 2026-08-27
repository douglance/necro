use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use rustix::fs::{FlockOperation, flock};

use futures::Stream;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tokio::process::Command;

use crate::board;
use crate::error::Error;
use crate::task::{Lane, is_valid_id};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum WatchKind {
    Enter,
    Exit,
}

impl WatchKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enter => "enter",
            Self::Exit => "exit",
        }
    }

    fn verb(self) -> &'static str {
        match self {
            Self::Enter => "entered",
            Self::Exit => "exited",
        }
    }

    pub fn parse_filter(value: &str) -> Result<&'static [WatchKind], Error> {
        match value {
            "enter" => Ok(&[WatchKind::Enter]),
            "exit" => Ok(&[WatchKind::Exit]),
            "all" => Ok(&[WatchKind::Enter, WatchKind::Exit]),
            other => Err(Error::InvalidEvent(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct WatchEvent {
    pub event: WatchKind,
    pub lane: Lane,
    pub id: String,
    pub path: String,
    pub agents_path: Option<String>,
    pub agents: Option<String>,
    pub exec_exit: Option<i32>,
    pub agent_output: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoardSnapshot {
    lanes: BTreeMap<Lane, BTreeSet<String>>,
}

impl BoardSnapshot {
    pub fn lane(&self, lane: Lane) -> BTreeSet<String> {
        self.lanes.get(&lane).cloned().unwrap_or_default()
    }
}

#[derive(Debug, Clone)]
pub struct WatchFilter {
    pub lanes: Vec<Lane>,
    pub kinds: Vec<WatchKind>,
}

impl WatchFilter {
    pub fn matches(&self, event: &WatchEvent) -> bool {
        (self.lanes.is_empty() || self.lanes.contains(&event.lane))
            && self.kinds.contains(&event.event)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentKind {
    Grok,
    Claude,
}

impl AgentKind {
    pub fn parse(value: &str) -> Result<Self, Error> {
        match value {
            "grok" => Ok(Self::Grok),
            "claude" => Ok(Self::Claude),
            other => Err(Error::InvalidAgent(other.to_string())),
        }
    }

    pub fn program(self) -> &'static str {
        match self {
            Self::Grok => "grok",
            Self::Claude => "claude",
        }
    }

    pub fn args(self, prompt: &str) -> Vec<String> {
        match self {
            Self::Grok => vec![
                "-p".to_string(),
                prompt.to_string(),
                "--always-approve".to_string(),
                "--output-format".to_string(),
                "plain".to_string(),
            ],
            Self::Claude => vec![
                "-p".to_string(),
                prompt.to_string(),
                "--dangerously-skip-permissions".to_string(),
            ],
        }
    }
}

/// Exclusive lock for one `--agent` watcher per board.
#[derive(Debug)]
pub struct AgentWatchLock {
    _file: fs::File,
}

impl AgentWatchLock {
    pub fn path(root: &Path) -> PathBuf {
        board::board_dir(root).join("agent-watch.lock")
    }

    pub fn acquire(root: &Path) -> Result<Self, Error> {
        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(Self::path(root))?;
        match flock(&file, FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => Ok(Self { _file: file }),
            Err(errno) => {
                let io_err = std::io::Error::from(errno);
                if io_err.kind() == ErrorKind::WouldBlock {
                    Err(Error::AgentWatcherLocked)
                } else {
                    Err(Error::Io(io_err))
                }
            }
        }
    }
}

pub fn agent_prompt(root: &Path, event: &WatchEvent) -> String {
    let task = fs::read_to_string(&event.path).unwrap_or_default();
    let agents = event.agents.as_deref().unwrap_or("(none)");
    let board_section = match board::load_board_agents(root) {
        Some(body) => format!("# Board AGENTS.md\n\n{body}\n\n"),
        None => String::new(),
    };
    format!(
        "A necro task {verb} the {lane} lane. Follow the lane instructions, then complete the task. Use necro to move the task when finished.\n\n\
Event: {event}\n\
Lane: {lane}\n\
Task ID: {id}\n\
Task path: {path}\n\n\
{board_section}\
# Lane AGENTS.md\n\n\
{agents}\n\n\
# Task\n\n\
{task}",
        verb = event.event.verb(),
        event = event.event.as_str(),
        lane = event.lane.as_str(),
        id = event.id,
        path = event.path,
        board_section = board_section,
        agents = agents,
        task = task,
    )
}

pub fn snapshot(root: &Path) -> Result<BoardSnapshot, Error> {
    let mut lanes = BTreeMap::new();
    for lane in Lane::all() {
        let mut ids = BTreeSet::new();
        let dir = board::lane_dir(root, *lane);
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if is_valid_id(stem) {
                ids.insert(stem.to_string());
            }
        }
        lanes.insert(*lane, ids);
    }
    Ok(BoardSnapshot { lanes })
}

pub fn diff(root: &Path, old: &BoardSnapshot, new: &BoardSnapshot) -> Vec<WatchEvent> {
    let mut events = Vec::new();
    for lane in Lane::all() {
        let before = old.lane(*lane);
        let after = new.lane(*lane);
        for id in before.difference(&after) {
            events.push(event(root, WatchKind::Exit, *lane, id));
        }
        for id in after.difference(&before) {
            events.push(event(root, WatchKind::Enter, *lane, id));
        }
    }
    events
}

pub fn existing_enters(root: &Path, snap: &BoardSnapshot) -> Vec<WatchEvent> {
    let empty = BoardSnapshot::default();
    diff(root, &empty, snap)
        .into_iter()
        .filter(|event| event.event == WatchKind::Enter)
        .collect()
}

fn event(root: &Path, kind: WatchKind, lane: Lane, id: &str) -> WatchEvent {
    let (agents_path, agents) = board::load_agents(root, lane);
    WatchEvent {
        event: kind,
        lane,
        id: id.to_string(),
        path: board::lane_dir(root, lane)
            .join(format!("{id}.md"))
            .to_string_lossy()
            .into_owned(),
        agents_path,
        agents,
        exec_exit: None,
        agent_output: None,
    }
}

pub async fn run_exec(root: &Path, script: &str, event: &mut WatchEvent) -> Result<(), Error> {
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(script)
        .env("NECRO_EVENT", event.event.as_str())
        .env("NECRO_LANE", event.lane.as_str())
        .env("NECRO_ID", &event.id)
        .env("NECRO_PATH", &event.path)
        .env("NECRO_ROOT", root.as_os_str())
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if let Some(path) = &event.agents_path {
        command.env("NECRO_AGENTS_PATH", path);
    }
    if let Some(body) = &event.agents {
        command.env("NECRO_AGENTS", body);
    }
    let status = command.status().await?;
    let code = status.code().unwrap_or(1);
    event.exec_exit = Some(code);
    if code != 0 {
        return Err(Error::ExecFailed {
            id: event.id.clone(),
            status: code,
        });
    }
    Ok(())
}

pub async fn run_agent(root: &Path, kind: AgentKind, event: &mut WatchEvent) -> Result<(), Error> {
    run_agent_bin(root, Path::new(kind.program()), kind, event).await
}

fn todo_task_path(root: &Path, id: &str) -> PathBuf {
    board::lane_dir(root, Lane::Todo).join(format!("{id}.md"))
}

fn task_still_in_todo(root: &Path, id: &str) -> bool {
    todo_task_path(root, id).is_file()
}

/// Claim a todo enter (`necro start`) before spawning. Returns `false` when the
/// task has left todo (started, done, or dropped) so the agent is not started.
async fn run_claimed_agent(
    root: &Path,
    program: &Path,
    kind: AgentKind,
    event: &mut WatchEvent,
) -> Result<bool, Error> {
    run_claimed_agent_between(root, program, kind, event, |_, _| {}).await
}

async fn run_claimed_agent_between(
    root: &Path,
    program: &Path,
    kind: AgentKind,
    event: &mut WatchEvent,
    between_detect_and_spawn: impl FnOnce(&Path, &str),
) -> Result<bool, Error> {
    if event.event == WatchKind::Enter && event.lane == Lane::Todo {
        between_detect_and_spawn(root, &event.id);
        if !task_still_in_todo(root, &event.id) {
            return Ok(false);
        }
        match crate::store::Store::open(root)?.claim(&event.id)? {
            Some(task) => event.path = task.path,
            None => return Ok(false),
        }
    }
    run_agent_bin(root, program, kind, event).await?;
    Ok(true)
}

async fn run_agent_bin(
    root: &Path,
    program: &Path,
    kind: AgentKind,
    event: &mut WatchEvent,
) -> Result<(), Error> {
    let prompt = agent_prompt(root, event);
    let mut command = Command::new(program);
    command
        .args(kind.args(&prompt))
        .current_dir(root)
        .env("NECRO_EVENT", event.event.as_str())
        .env("NECRO_LANE", event.lane.as_str())
        .env("NECRO_ID", &event.id)
        .env("NECRO_PATH", &event.path)
        .env("NECRO_ROOT", root.as_os_str())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if let Some(path) = &event.agents_path {
        command.env("NECRO_AGENTS_PATH", path);
    }
    if let Some(body) = &event.agents {
        command.env("NECRO_AGENTS", body);
    }
    let mut child = command.spawn()?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("agent stdout was not piped"))?;
    stream_agent_stdout(root, event, &mut stdout).await?;
    let status = child.wait().await?;
    let code = status.code().unwrap_or(1);
    event.exec_exit = Some(code);
    Ok(())
}

async fn stream_agent_stdout(
    root: &Path,
    event: &mut WatchEvent,
    stdout: &mut (impl AsyncReadExt + Unpin),
) -> Result<(), Error> {
    let mut buf = [0u8; 4096];
    let mut captured = Vec::new();
    let mut started = false;
    loop {
        let n = stdout.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        let chunk = &buf[..n];
        captured.extend_from_slice(chunk);
        let _ = std::io::Write::write_all(&mut std::io::stdout(), chunk);
        let store = crate::store::Store::open(root)?;
        if !started {
            store.append(&event.id, b"\n")?;
            started = true;
        } else if !captured[..captured.len() - n].ends_with(b"\n") && !chunk.starts_with(b"\n") {
            store.append(&event.id, b"\n")?;
        }
        store.append(&event.id, chunk)?;
    }
    if !captured.is_empty() && !captured.ends_with(b"\n") {
        let _ = crate::store::Store::open(root)?.append(&event.id, b"\n");
    }
    let text = String::from_utf8_lossy(&captured);
    let text = text.trim_end();
    if !text.is_empty() {
        event.agent_output = Some(text.to_string());
    }
    Ok(())
}

async fn apply_hooks(
    root: &Path,
    exec: &Option<String>,
    agent: Option<AgentKind>,
    event: &mut WatchEvent,
) -> Result<(), Error> {
    if let Some(script) = exec {
        run_exec(root, script, event).await?;
    }
    if let Some(kind) = agent {
        run_claimed_agent(root, Path::new(kind.program()), kind, event).await?;
    }
    Ok(())
}

pub async fn wait_once(
    root: PathBuf,
    filter: WatchFilter,
    existing: bool,
    interval: Duration,
    exec: Option<String>,
    agent: Option<AgentKind>,
) -> Result<WatchEvent, Error> {
    let mut snap = snapshot(&root)?;
    if existing {
        for mut event in existing_enters(&root, &snap) {
            if filter.matches(&event) {
                apply_hooks(&root, &exec, agent, &mut event).await?;
                return Ok(event);
            }
        }
    }
    loop {
        tokio::time::sleep(interval).await;
        let next = snapshot(&root)?;
        let events = diff(&root, &snap, &next);
        snap = next;
        for mut event in events {
            if !filter.matches(&event) {
                continue;
            }
            apply_hooks(&root, &exec, agent, &mut event).await?;
            return Ok(event);
        }
    }
}

pub fn watch_stream(
    root: PathBuf,
    filter: WatchFilter,
    existing: bool,
    interval: Duration,
    exec: Option<String>,
    agent: Option<AgentKind>,
    lock: Option<AgentWatchLock>,
) -> impl Stream<Item = serde_json::Value> {
    async_stream::stream! {
        let _lock = lock;
        let Ok(mut snap) = snapshot(&root) else {
            return;
        };
        if existing {
            for mut event in existing_enters(&root, &snap) {
                if !filter.matches(&event) {
                    continue;
                }
                let _ = apply_hooks(&root, &exec, agent, &mut event).await;
                if let Ok(value) = serde_json::to_value(&event) {
                    yield value;
                }
            }
        }
        loop {
            tokio::time::sleep(interval).await;
            let Ok(next) = snapshot(&root) else {
                continue;
            };
            let events = diff(&root, &snap, &next);
            snap = next;
            for mut event in events {
                if !filter.matches(&event) {
                    continue;
                }
                let _ = apply_hooks(&root, &exec, agent, &mut event).await;
                if let Ok(value) = serde_json::to_value(&event) {
                    yield value;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board;

    fn primed() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        board::init(dir.path()).unwrap();
        let root = dir.path().canonicalize().unwrap();
        (dir, root)
    }

    #[test]
    fn agent_watch_lock_succeeds_when_free() {
        let (_dir, root) = primed();
        AgentWatchLock::acquire(&root).unwrap();
        assert!(AgentWatchLock::path(&root).is_file());
    }

    #[test]
    fn agent_watch_lock_released_on_drop() {
        let (_dir, root) = primed();
        let lock = AgentWatchLock::acquire(&root).unwrap();
        drop(lock);
        AgentWatchLock::acquire(&root).unwrap();
    }

    #[test]
    fn agent_watch_lock_fails_when_held() {
        let (_dir, root) = primed();
        if let Ok(hold) = std::env::var("NECRO_TEST_HOLD_AGENT_LOCK") {
            let _lock = AgentWatchLock::acquire(Path::new(&hold)).unwrap();
            println!("locked");
            std::thread::sleep(std::time::Duration::from_secs(30));
            return;
        }

        let exe = std::env::current_exe().unwrap();
        let mut child = std::process::Command::new(&exe)
            .args([
                "--exact",
                "watch::tests::agent_watch_lock_fails_when_held",
                "--nocapture",
            ])
            .env("NECRO_TEST_HOLD_AGENT_LOCK", &root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut reader = std::io::BufReader::new(stdout);
        let mut line = String::new();
        let start = std::time::Instant::now();
        loop {
            line.clear();
            if std::io::BufRead::read_line(&mut reader, &mut line).unwrap() == 0 {
                let _ = child.kill();
                let _ = child.wait();
                panic!("holder exited before locking");
            }
            if line.trim() == "locked" {
                break;
            }
            assert!(
                start.elapsed() < std::time::Duration::from_secs(5),
                "holder did not lock"
            );
        }
        let err = AgentWatchLock::acquire(&root).unwrap_err();
        assert_eq!(err.code(), "AGENT_WATCHER_LOCKED");
        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn diff_rename_is_exit_then_enter() {
        let (_dir, root) = primed();
        fs::write(board::lane_dir(&root, Lane::Todo).join("alpha.md"), "# A\n").unwrap();
        let first = snapshot(&root).unwrap();
        fs::rename(
            board::lane_dir(&root, Lane::Todo).join("alpha.md"),
            board::lane_dir(&root, Lane::Doing).join("alpha.md"),
        )
        .unwrap();
        let second = snapshot(&root).unwrap();
        let events = diff(&root, &first, &second);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event, WatchKind::Exit);
        assert_eq!(events[0].lane, Lane::Todo);
        assert_eq!(events[0].id, "alpha");
        assert_eq!(events[1].event, WatchKind::Enter);
        assert_eq!(events[1].lane, Lane::Doing);
        assert_eq!(events[1].id, "alpha");
    }

    #[test]
    fn existing_enters_lists_current_todos() {
        let (_dir, root) = primed();
        fs::write(board::lane_dir(&root, Lane::Todo).join("beta.md"), "# B\n").unwrap();
        let snap = snapshot(&root).unwrap();
        let events = existing_enters(&root, &snap);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, WatchKind::Enter);
        assert_eq!(events[0].id, "beta");
    }

    #[test]
    fn filter_skips_other_lanes() {
        let event = WatchEvent {
            event: WatchKind::Enter,
            lane: Lane::Done,
            id: "x".into(),
            path: "x.md".into(),
            agents_path: None,
            agents: None,
            exec_exit: None,
            agent_output: None,
        };
        let filter = WatchFilter {
            lanes: vec![Lane::Todo],
            kinds: vec![WatchKind::Enter],
        };
        assert!(!filter.matches(&event));
    }

    #[test]
    fn grok_args_are_headless_yolo() {
        let args = AgentKind::Grok.args("do the task");
        assert_eq!(
            args,
            vec![
                "-p",
                "do the task",
                "--always-approve",
                "--output-format",
                "plain"
            ]
        );
    }

    #[test]
    fn agent_parse_accepts_claude() {
        let kind = AgentKind::parse("claude").expect("claude is a valid agent");
        assert_eq!(kind.program(), "claude");
    }

    #[test]
    fn agent_parse_rejects_unknown() {
        let err = AgentKind::parse("nope").unwrap_err();
        assert_eq!(err.code(), "INVALID_AGENT");
    }

    #[test]
    fn claude_args_are_headless_yolo() {
        let kind = AgentKind::parse("claude").expect("claude is a valid agent");
        assert_eq!(
            kind.args("do the task"),
            vec!["-p", "do the task", "--dangerously-skip-permissions"]
        );
    }

    #[test]
    fn agent_prompt_includes_lane_agents_and_task() {
        let (_dir, root) = primed();
        fs::write(
            board::agents_path(&root, Lane::Todo),
            "Start the task before editing.\n",
        )
        .unwrap();
        let task = board::lane_dir(&root, Lane::Todo).join("gamma.md");
        fs::write(&task, "# Gamma\nDo the work.\n").unwrap();
        let event = event(&root, WatchKind::Enter, Lane::Todo, "gamma");
        let prompt = agent_prompt(&root, &event);
        assert!(prompt.contains("Start the task before editing."));
        assert!(prompt.contains("# Gamma\nDo the work.\n"));
        assert!(prompt.contains("gamma"));
        assert!(prompt.contains("entered"));
    }

    #[test]
    fn agent_prompt_includes_board_root_agents_md_when_present() {
        let (_dir, root) = primed();
        fs::write(
            root.join("AGENTS.md"),
            "ALWAYS USE NECRO TO IMPLEMENT ALL CHANGES.\n",
        )
        .unwrap();
        fs::write(
            board::agents_path(&root, Lane::Todo),
            "Start the task before editing.\n",
        )
        .unwrap();
        let task = board::lane_dir(&root, Lane::Todo).join("gamma.md");
        fs::write(&task, "# Gamma\nDo the work.\n").unwrap();
        let event = event(&root, WatchKind::Enter, Lane::Todo, "gamma");
        let prompt = agent_prompt(&root, &event);
        assert!(prompt.contains("ALWAYS USE NECRO TO IMPLEMENT ALL CHANGES."));
        assert!(prompt.contains("Start the task before editing."));
        assert!(prompt.contains("# Gamma\nDo the work.\n"));
    }

    #[test]
    fn agent_prompt_succeeds_when_board_root_agents_md_is_missing() {
        let (_dir, root) = primed();
        assert!(!root.join("AGENTS.md").exists());
        fs::write(
            board::agents_path(&root, Lane::Todo),
            "Start the task before editing.\n",
        )
        .unwrap();
        let task = board::lane_dir(&root, Lane::Todo).join("gamma.md");
        fs::write(&task, "# Gamma\nDo the work.\n").unwrap();
        let event = event(&root, WatchKind::Enter, Lane::Todo, "gamma");
        let prompt = agent_prompt(&root, &event);
        assert!(prompt.contains("Start the task before editing."));
        assert!(prompt.contains("# Gamma\nDo the work.\n"));
        assert!(!prompt.contains("# Board AGENTS.md"));
    }

    #[tokio::test]
    async fn run_agent_spawns_program_with_prompt() {
        let (dir, root) = primed();
        fs::write(
            board::agents_path(&root, Lane::Todo),
            "Start the task before editing.\n",
        )
        .unwrap();
        let task = board::lane_dir(&root, Lane::Todo).join("gamma.md");
        fs::write(&task, "# Gamma\nDo the work.\n").unwrap();
        let mut event = event(&root, WatchKind::Enter, Lane::Todo, "gamma");

        let stub = dir.path().join("fake-grok");
        fs::write(
            &stub,
            "#!/bin/sh\npwd > \"$NECRO_ROOT/agent-cwd.txt\"\ni=1\nfor arg in \"$@\"; do\n  printf '%s' \"$arg\" > \"$NECRO_ROOT/agent-arg-$i.txt\"\n  i=$((i+1))\ndone\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&stub).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&stub, perms).unwrap();
        }

        run_agent_bin(&root, &stub, AgentKind::Grok, &mut event)
            .await
            .unwrap();
        assert_eq!(event.exec_exit, Some(0));
        let cwd = fs::read_to_string(root.join("agent-cwd.txt")).unwrap();
        assert_eq!(cwd.trim(), root.to_str().unwrap());
        assert_eq!(
            fs::read_to_string(root.join("agent-arg-1.txt")).unwrap(),
            "-p"
        );
        let prompt = fs::read_to_string(root.join("agent-arg-2.txt")).unwrap();
        assert!(prompt.contains("Start the task before editing."));
        assert!(prompt.contains("# Gamma\nDo the work.\n"));
        assert_eq!(
            fs::read_to_string(root.join("agent-arg-3.txt")).unwrap(),
            "--always-approve"
        );
        assert_eq!(event.agent_output, None);
        assert_eq!(
            fs::read_to_string(&task).unwrap(),
            "# Gamma\nDo the work.\n"
        );
    }

    #[tokio::test]
    async fn run_agent_spawns_claude_with_yolo_flag() {
        let (dir, root) = primed();
        fs::write(
            board::agents_path(&root, Lane::Todo),
            "Start the task before editing.\n",
        )
        .unwrap();
        let task = board::lane_dir(&root, Lane::Todo).join("gamma.md");
        fs::write(&task, "# Gamma\nDo the work.\n").unwrap();
        let mut event = event(&root, WatchKind::Enter, Lane::Todo, "gamma");

        let stub = dir.path().join("fake-claude");
        fs::write(
            &stub,
            "#!/bin/sh\npwd > \"$NECRO_ROOT/agent-cwd.txt\"\ni=1\nfor arg in \"$@\"; do\n  printf '%s' \"$arg\" > \"$NECRO_ROOT/agent-arg-$i.txt\"\n  i=$((i+1))\ndone\nprintf '%s\\n' 'hello from claude'\n",
        )
        .unwrap();
        chmod_exec(&stub);

        let kind = AgentKind::parse("claude").expect("claude is a valid agent");
        run_agent_bin(&root, &stub, kind, &mut event).await.unwrap();
        assert_eq!(event.exec_exit, Some(0));
        let cwd = fs::read_to_string(root.join("agent-cwd.txt")).unwrap();
        assert_eq!(cwd.trim(), root.to_str().unwrap());
        assert_eq!(
            fs::read_to_string(root.join("agent-arg-1.txt")).unwrap(),
            "-p"
        );
        let prompt = fs::read_to_string(root.join("agent-arg-2.txt")).unwrap();
        assert!(prompt.contains("Start the task before editing."));
        assert!(prompt.contains("# Gamma\nDo the work.\n"));
        assert_eq!(
            fs::read_to_string(root.join("agent-arg-3.txt")).unwrap(),
            "--dangerously-skip-permissions"
        );
        assert!(!root.join("agent-arg-4.txt").exists());
        assert_eq!(event.agent_output.as_deref(), Some("hello from claude"));
        let body = fs::read_to_string(&task).unwrap();
        assert!(body.contains("# Gamma\nDo the work.\n"));
        assert!(body.contains("hello from claude\n"));
        assert!(!body.contains("## Agent output"));
    }

    fn chmod_exec(path: &Path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(path, perms).unwrap();
        }
    }

    fn write_spawn_stub(dir: &Path) -> PathBuf {
        let stub = dir.join("fake-grok");
        fs::write(
            &stub,
            r#"#!/bin/sh
touch "$NECRO_ROOT/agent-ran"
if [ -f "$NECRO_ROOT/.necro/2-doing/$NECRO_ID.md" ] && [ ! -f "$NECRO_ROOT/.necro/1-todo/$NECRO_ID.md" ]; then
  touch "$NECRO_ROOT/claimed-before-spawn"
fi
printf '%s' "$1" > "$NECRO_ROOT/agent-arg-1.txt"
"#,
        )
        .unwrap();
        chmod_exec(&stub);
        stub
    }

    #[tokio::test]
    async fn todo_enter_claims_before_agent_spawn() {
        let (dir, root) = primed();
        let todo = board::lane_dir(&root, Lane::Todo).join("gamma.md");
        let doing = board::lane_dir(&root, Lane::Doing).join("gamma.md");
        fs::write(&todo, "# Gamma\nDo the work.\n").unwrap();
        let mut event = event(&root, WatchKind::Enter, Lane::Todo, "gamma");
        let stub = write_spawn_stub(dir.path());

        let spawned = run_claimed_agent(&root, &stub, AgentKind::Grok, &mut event)
            .await
            .unwrap();
        assert!(spawned);
        assert!(!todo.exists());
        assert!(doing.is_file());
        assert!(root.join("agent-ran").is_file());
        assert!(root.join("claimed-before-spawn").is_file());
        assert_eq!(
            fs::read_to_string(root.join("agent-arg-1.txt")).unwrap(),
            "-p"
        );
        assert!(event.path.ends_with("/.necro/2-doing/gamma.md"));
    }

    #[tokio::test]
    async fn todo_enter_skips_agent_when_already_doing() {
        let (dir, root) = primed();
        let doing = board::lane_dir(&root, Lane::Doing).join("gamma.md");
        fs::write(&doing, "# Gamma\nAlready claimed.\n").unwrap();
        let mut event = event(&root, WatchKind::Enter, Lane::Todo, "gamma");
        let stub = write_spawn_stub(dir.path());

        let spawned = run_claimed_agent(&root, &stub, AgentKind::Grok, &mut event)
            .await
            .unwrap();
        assert!(!spawned);
        assert!(!root.join("agent-ran").exists());
        assert!(!root.join("claimed-before-spawn").exists());
        assert!(doing.is_file());
        assert!(!board::lane_dir(&root, Lane::Todo).join("gamma.md").exists());
        assert_eq!(event.exec_exit, None);
        assert_eq!(
            fs::read_to_string(&doing).unwrap(),
            "# Gamma\nAlready claimed.\n"
        );
    }

    async fn assert_skips_spawn_after_leaving_todo(dest_lane: Lane) {
        let (dir, root) = primed();
        let todo = board::lane_dir(&root, Lane::Todo).join("gamma.md");
        let dest_path = board::lane_dir(&root, dest_lane).join("gamma.md");
        fs::write(&todo, "# Gamma\nDo the work.\n").unwrap();
        let mut event = event(&root, WatchKind::Enter, Lane::Todo, "gamma");
        let stub = write_spawn_stub(dir.path());

        let spawned =
            run_claimed_agent_between(&root, &stub, AgentKind::Grok, &mut event, |root, id| {
                fs::rename(
                    todo_task_path(root, id),
                    board::lane_dir(root, dest_lane).join(format!("{id}.md")),
                )
                .unwrap();
            })
            .await
            .unwrap();
        assert!(
            !spawned,
            "agent spawned after todo file moved to {}",
            dest_lane.as_str()
        );
        assert!(
            !root.join("agent-ran").exists(),
            "agent binary ran after todo file moved to {}",
            dest_lane.as_str()
        );
        assert!(!todo.exists());
        assert!(dest_path.is_file());
        assert_eq!(event.exec_exit, None);
    }

    #[tokio::test]
    async fn todo_enter_skips_agent_when_task_left_todo() {
        for dest in [Lane::Doing, Lane::Done, Lane::Dropped] {
            assert_skips_spawn_after_leaving_todo(dest).await;
        }
    }

    #[tokio::test]
    async fn todo_enter_still_spawns_when_file_remains_in_todo() {
        let (dir, root) = primed();
        let todo = board::lane_dir(&root, Lane::Todo).join("gamma.md");
        fs::write(&todo, "# Gamma\nDo the work.\n").unwrap();
        let mut event = event(&root, WatchKind::Enter, Lane::Todo, "gamma");
        let stub = write_spawn_stub(dir.path());
        assert!(todo.is_file());

        let spawned = run_claimed_agent(&root, &stub, AgentKind::Grok, &mut event)
            .await
            .unwrap();
        assert!(spawned);
        assert!(root.join("agent-ran").is_file());
        assert!(!todo.exists());
        assert!(
            board::lane_dir(&root, Lane::Doing)
                .join("gamma.md")
                .is_file()
        );
    }

    #[tokio::test]
    async fn run_agent_appends_stdout_to_task_file() {
        let (dir, root) = primed();
        let task = board::lane_dir(&root, Lane::Todo).join("gamma.md");
        fs::write(&task, "# Gamma\nDo the work.\n").unwrap();
        let mut event = event(&root, WatchKind::Enter, Lane::Todo, "gamma");
        let stub = dir.path().join("fake-grok");
        fs::write(&stub, "#!/bin/sh\nprintf '%s\\n' 'hello from grok'\n").unwrap();
        chmod_exec(&stub);

        run_agent_bin(&root, &stub, AgentKind::Grok, &mut event)
            .await
            .unwrap();
        assert_eq!(event.agent_output.as_deref(), Some("hello from grok"));
        let body = fs::read_to_string(&task).unwrap();
        assert!(body.contains("# Gamma\nDo the work.\n"));
        assert!(body.contains("hello from grok\n"));
        assert!(!body.contains("## Agent output"));
    }

    #[tokio::test]
    async fn run_agent_appends_stdout_before_process_exits() {
        let (dir, root) = primed();
        let task = board::lane_dir(&root, Lane::Todo).join("gamma.md");
        fs::write(&task, "# Gamma\nDo the work.\n").unwrap();
        let mut event = event(&root, WatchKind::Enter, Lane::Todo, "gamma");
        let stub = dir.path().join("fake-grok");
        fs::write(
            &stub,
            "#!/bin/sh\nprintf '%s\\n' 'first'\nsleep 0.4\nprintf '%s\\n' 'second'\n",
        )
        .unwrap();
        chmod_exec(&stub);

        let mut saw_partial = false;
        {
            let mut run = std::pin::pin!(run_agent_bin(&root, &stub, AgentKind::Grok, &mut event));
            loop {
                tokio::select! {
                    result = &mut run => {
                        result.unwrap();
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(40)) => {
                        if let Ok(body) = fs::read_to_string(&task) {
                            if body.contains("first") && !body.contains("second") {
                                saw_partial = true;
                            }
                        }
                    }
                }
            }
        }
        assert!(
            saw_partial,
            "task file should contain the first stdout chunk before the agent process exits"
        );
        let body = fs::read_to_string(&task).unwrap();
        assert!(body.contains("first\nsecond\n"));
        assert_eq!(event.agent_output.as_deref(), Some("first\nsecond"));
    }

    #[tokio::test]
    async fn run_agent_notes_task_after_it_moves() {
        let (dir, root) = primed();
        let todo = board::lane_dir(&root, Lane::Todo).join("gamma.md");
        let doing = board::lane_dir(&root, Lane::Doing).join("gamma.md");
        fs::write(&todo, "# Gamma\nDo the work.\n").unwrap();
        let mut event = event(&root, WatchKind::Enter, Lane::Todo, "gamma");
        let stub = dir.path().join("fake-grok");
        fs::write(
            &stub,
            "#!/bin/sh\nmv \"$NECRO_PATH\" \"$NECRO_ROOT/.necro/2-doing/gamma.md\"\nprintf '%s\\n' 'moved then spoke'\n",
        )
        .unwrap();
        chmod_exec(&stub);

        run_agent_bin(&root, &stub, AgentKind::Grok, &mut event)
            .await
            .unwrap();
        assert!(!todo.exists());
        let body = fs::read_to_string(&doing).unwrap();
        assert!(body.contains("moved then spoke"));
        assert_eq!(event.agent_output.as_deref(), Some("moved then spoke"));
    }

    fn assert_not_dropped(root: &Path, id: &str) {
        assert!(
            !board::lane_dir(root, Lane::Dropped)
                .join(format!("{id}.md"))
                .exists()
        );
    }

    #[tokio::test]
    async fn run_agent_records_stdout_on_nonzero_exit_without_dropping() {
        let (dir, root) = primed();
        let task = board::lane_dir(&root, Lane::Todo).join("gamma.md");
        fs::write(&task, "# Gamma\nDo the work.\n").unwrap();
        let mut event = event(&root, WatchKind::Enter, Lane::Todo, "gamma");
        let stub = dir.path().join("fake-grok");
        fs::write(&stub, "#!/bin/sh\nprintf '%s\\n' 'agent crashed'\nexit 2\n").unwrap();
        chmod_exec(&stub);

        run_agent_bin(&root, &stub, AgentKind::Grok, &mut event)
            .await
            .unwrap();
        assert_eq!(event.exec_exit, Some(2));
        assert_eq!(event.agent_output.as_deref(), Some("agent crashed"));
        let body = fs::read_to_string(&task).unwrap();
        assert!(body.contains("# Gamma\nDo the work.\n"));
        assert!(body.contains("agent crashed\n"));
        assert!(!body.contains("## Agent output"));
        assert!(task.is_file());
        assert_not_dropped(&root, "gamma");
    }

    #[tokio::test]
    async fn claimed_agent_nonzero_exit_notes_doing_task_without_dropping() {
        let (dir, root) = primed();
        let todo = board::lane_dir(&root, Lane::Todo).join("gamma.md");
        let doing = board::lane_dir(&root, Lane::Doing).join("gamma.md");
        fs::write(&todo, "# Gamma\nDo the work.\n").unwrap();
        let mut event = event(&root, WatchKind::Enter, Lane::Todo, "gamma");
        let stub = dir.path().join("fake-grok");
        fs::write(
            &stub,
            "#!/bin/sh\nprintf '%s\\n' 'claimed then crashed'\nexit 3\n",
        )
        .unwrap();
        chmod_exec(&stub);

        let spawned = run_claimed_agent(&root, &stub, AgentKind::Grok, &mut event)
            .await
            .unwrap();
        assert!(spawned);
        assert!(!todo.exists());
        assert!(doing.is_file());
        assert_eq!(event.exec_exit, Some(3));
        assert_eq!(event.agent_output.as_deref(), Some("claimed then crashed"));
        let body = fs::read_to_string(&doing).unwrap();
        assert!(body.contains("claimed then crashed\n"));
        assert!(!body.contains("## Agent output"));
        assert_not_dropped(&root, "gamma");
    }
}
