use std::fs;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::Serialize;

use crate::error::Error;
use crate::task::Lane;

/// Directory that holds the lane folders.
pub const BOARD_DIR: &str = ".necro";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct InitResult {
    pub root: String,
    pub created: Vec<String>,
}

pub fn board_dir(root: &Path) -> PathBuf {
    root.join(BOARD_DIR)
}

fn lane_dir_exists(board: &Path, lane: Lane) -> bool {
    board.join(lane.dir_name()).is_dir() || board.join(lane.as_str()).is_dir()
}

pub fn is_board(path: &Path) -> bool {
    let board = board_dir(path);
    Lane::all()
        .iter()
        .all(|lane| lane_dir_exists(&board, *lane))
}

/// Rename unnumbered lane dirs (`todo`) to numbered names (`1-todo`) once.
pub fn migrate_lane_dirs(root: &Path) -> Result<(), Error> {
    let necro = board_dir(root);
    if !necro.is_dir() {
        return Ok(());
    }
    for lane in Lane::all() {
        let numbered = necro.join(lane.dir_name());
        let legacy = necro.join(lane.as_str());
        if numbered.exists() {
            if !numbered.is_dir() {
                return Err(Error::InvalidRoot(numbered));
            }
            continue;
        }
        if legacy.is_dir() {
            fs::rename(&legacy, &numbered)?;
        } else if legacy.exists() {
            return Err(Error::InvalidRoot(legacy));
        }
    }
    Ok(())
}

pub fn find_board(start: &Path) -> Option<PathBuf> {
    let mut current = if start.exists() {
        start.canonicalize().ok()?
    } else {
        return None;
    };
    loop {
        if is_board(&current) {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Chosen root for `init`: `--root`, else `NECRO_ROOT`, else the working directory.
pub fn chosen_root(explicit: Option<&Path>, cwd: &Path) -> Result<PathBuf, Error> {
    match explicit {
        Some(path) => prepare_root(path),
        None => prepare_root(cwd),
    }
}

fn prepare_root(path: &Path) -> Result<PathBuf, Error> {
    if path.exists() && !path.is_dir() {
        return Err(Error::InvalidRoot(path.to_path_buf()));
    }
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    path.canonicalize().map_err(Error::from)
}

pub fn require_board(explicit: Option<&Path>, cwd: &Path) -> Result<PathBuf, Error> {
    if let Some(path) = explicit {
        if path.exists() && !path.is_dir() {
            return Err(Error::InvalidRoot(path.to_path_buf()));
        }
        let root = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if is_board(&root) {
            migrate_lane_dirs(&root)?;
            return Ok(root);
        }
        return Err(Error::BoardNotFound(root));
    }
    let root = find_board(cwd).ok_or_else(|| Error::BoardNotFound(cwd.to_path_buf()))?;
    migrate_lane_dirs(&root)?;
    Ok(root)
}

pub fn init(root: &Path) -> Result<InitResult, Error> {
    let root = prepare_root(root)?;
    let necro = board_dir(&root);
    let mut created = Vec::new();
    if !necro.exists() {
        fs::create_dir(&necro)?;
        created.push(BOARD_DIR.to_string());
    } else if !necro.is_dir() {
        return Err(Error::InvalidRoot(necro));
    }
    migrate_lane_dirs(&root)?;
    for lane in Lane::all() {
        let path = necro.join(lane.dir_name());
        if !path.exists() {
            fs::create_dir(&path)?;
            created.push(format!("{BOARD_DIR}/{}", lane.dir_name()));
        } else if !path.is_dir() {
            return Err(Error::InvalidRoot(path));
        }
    }
    Ok(InitResult {
        root: root.to_string_lossy().into_owned(),
        created,
    })
}

pub fn lane_dir(root: &Path, lane: Lane) -> PathBuf {
    board_dir(root).join(lane.dir_name())
}

pub fn agents_path(root: &Path, lane: Lane) -> PathBuf {
    lane_dir(root, lane).join("AGENTS.md")
}

pub fn board_agents_path(root: &Path) -> PathBuf {
    root.join("AGENTS.md")
}

/// Returns the board-root `AGENTS.md` body. Missing or unreadable files are `None`.
pub fn load_board_agents(root: &Path) -> Option<String> {
    let path = board_agents_path(root);
    if !path.is_file() {
        return None;
    }
    fs::read_to_string(path).ok()
}

/// Returns `(agents_path, agents)` for a lane. Missing files are `None`.
pub fn load_agents(root: &Path, lane: Lane) -> (Option<String>, Option<String>) {
    let path = agents_path(root, lane);
    if !path.is_file() {
        return (None, None);
    }
    let displayed = path
        .canonicalize()
        .unwrap_or_else(|_| path.clone())
        .to_string_lossy()
        .into_owned();
    match fs::read_to_string(&path) {
        Ok(body) => (Some(displayed), Some(body)),
        Err(_) => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_walks_up_to_parent_board() {
        let dir = tempfile::tempdir().unwrap();
        init(dir.path()).unwrap();
        let nested = dir.path().join("src").join("inner");
        fs::create_dir_all(&nested).unwrap();
        let found = find_board(&nested).unwrap();
        assert_eq!(found, dir.path().canonicalize().unwrap());
    }

    #[test]
    fn missing_board_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(find_board(dir.path()).is_none());
    }

    #[test]
    fn init_creates_numbered_lane_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let result = init(dir.path()).unwrap();
        assert_eq!(
            result.created,
            vec![
                ".necro",
                ".necro/1-todo",
                ".necro/2-doing",
                ".necro/3-done",
                ".necro/4-dropped",
            ]
        );
        for lane in Lane::all() {
            assert!(board_dir(dir.path()).join(lane.dir_name()).is_dir());
            assert!(!board_dir(dir.path()).join(lane.as_str()).exists());
        }
    }

    fn write_legacy_board(root: &Path) {
        let necro = board_dir(root);
        fs::create_dir_all(&necro).unwrap();
        for lane in Lane::all() {
            fs::create_dir(necro.join(lane.as_str())).unwrap();
        }
    }

    #[test]
    fn find_board_accepts_legacy_unnumbered_dirs() {
        let dir = tempfile::tempdir().unwrap();
        write_legacy_board(dir.path());
        assert!(is_board(dir.path()));
        let found = find_board(dir.path()).unwrap();
        assert_eq!(found, dir.path().canonicalize().unwrap());
        for lane in Lane::all() {
            assert!(board_dir(dir.path()).join(lane.as_str()).is_dir());
            assert!(!board_dir(dir.path()).join(lane.dir_name()).exists());
        }
    }

    #[test]
    fn init_renames_legacy_unnumbered_dirs_once() {
        let dir = tempfile::tempdir().unwrap();
        write_legacy_board(dir.path());
        fs::write(
            board_dir(dir.path()).join("todo").join("legacy.md"),
            "# Old\n",
        )
        .unwrap();
        let result = init(dir.path()).unwrap();
        assert!(result.created.is_empty());
        assert!(
            board_dir(dir.path())
                .join("1-todo")
                .join("legacy.md")
                .is_file()
        );
        for lane in Lane::all() {
            assert!(board_dir(dir.path()).join(lane.dir_name()).is_dir());
            assert!(!board_dir(dir.path()).join(lane.as_str()).exists());
        }
        let again = init(dir.path()).unwrap();
        assert!(again.created.is_empty());
        assert!(
            board_dir(dir.path())
                .join("1-todo")
                .join("legacy.md")
                .is_file()
        );
    }

    #[test]
    fn require_board_renames_legacy_unnumbered_dirs() {
        let dir = tempfile::tempdir().unwrap();
        write_legacy_board(dir.path());
        let root = require_board(Some(dir.path()), dir.path()).unwrap();
        assert_eq!(root, dir.path().canonicalize().unwrap());
        for lane in Lane::all() {
            assert!(board_dir(dir.path()).join(lane.dir_name()).is_dir());
            assert!(!board_dir(dir.path()).join(lane.as_str()).exists());
        }
    }
}
