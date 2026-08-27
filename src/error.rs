use std::io;
use std::path::PathBuf;

/// Stable error codes returned by the store and mapped onto incurs results.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "no board at {0}: expected .necro/1-todo, .necro/2-doing, .necro/3-done, and .necro/4-dropped directories"
    )]
    BoardNotFound(PathBuf),
    #[error("task `{0}` not found")]
    TaskNotFound(String),
    #[error("task `{0}` exists in both lanes")]
    Conflict(String),
    #[error("task `{0}` is done; reopen it before starting")]
    AlreadyDone(String),
    #[error("task `{0}` is dropped; reopen it first")]
    AlreadyDropped(String),
    #[error("invalid task id `{0}`")]
    InvalidId(String),
    #[error("status `{0}` is not todo, doing, done, dropped, or all")]
    InvalidStatus(String),
    #[error("root `{0}` is not a directory")]
    InvalidRoot(PathBuf),
    #[error("watch event `{0}` is not enter, exit, or all")]
    InvalidEvent(String),
    #[error("agent `{0}` is not grok or claude")]
    InvalidAgent(String),
    #[error("watch cannot use --agent and --exec together")]
    AgentAndExec,
    #[error("another agent watcher is already running on this board")]
    AgentWatcherLocked,
    #[error("exec for `{id}` exited {status}")]
    ExecFailed { id: String, status: i32 },
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::BoardNotFound(_) => "BOARD_NOT_FOUND",
            Self::TaskNotFound(_) => "TASK_NOT_FOUND",
            Self::Conflict(_) => "CONFLICT",
            Self::AlreadyDone(_) => "ALREADY_DONE",
            Self::AlreadyDropped(_) => "ALREADY_DROPPED",
            Self::InvalidId(_) => "INVALID_ID",
            Self::InvalidStatus(_) => "INVALID_STATUS",
            Self::InvalidRoot(_) => "INVALID_ROOT",
            Self::InvalidEvent(_) => "INVALID_EVENT",
            Self::InvalidAgent(_) => "INVALID_AGENT",
            Self::AgentAndExec => "AGENT_AND_EXEC",
            Self::AgentWatcherLocked => "AGENT_WATCHER_LOCKED",
            Self::ExecFailed { .. } => "EXEC_FAILED",
            Self::Io(_) => "IO_ERROR",
        }
    }
}
