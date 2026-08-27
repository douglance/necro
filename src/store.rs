use std::fs;
use std::path::{Path, PathBuf};

use crate::board;
use crate::error::Error;
use crate::task::{
    Lane, StatusFilter, TaskRecord, TaskSummary, body_with_optional_extra, is_valid_id, slugify,
    title_from_body,
};

#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, Error> {
        let root = root.into();
        if !board::is_board(&root) {
            return Err(Error::BoardNotFound(root));
        }
        board::migrate_lane_dirs(&root)?;
        Ok(Self {
            root: root.canonicalize()?,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn add(&self, title: &str, extra: Option<&str>) -> Result<TaskRecord, Error> {
        let base = slugify(title);
        let id = self.unique_id(&base)?;
        let path = self.task_path(Lane::Todo, &id);
        let body = body_with_optional_extra(title, extra);
        fs::write(&path, &body)?;
        Ok(self.record(&id, Lane::Todo, &path, &body))
    }

    pub fn next(&self) -> Result<Option<TaskRecord>, Error> {
        let Some(summary) = self.list(StatusFilter::Todo)?.into_iter().next() else {
            return Ok(None);
        };
        self.show(&summary.id).map(Some)
    }

    pub fn list(&self, filter: StatusFilter) -> Result<Vec<TaskSummary>, Error> {
        let mut tasks = Vec::new();
        for lane in filter.lanes() {
            tasks.extend(self.list_lane(*lane)?);
        }
        tasks.sort_by(|a, b| {
            a.id.cmp(&b.id)
                .then(a.status.as_str().cmp(b.status.as_str()))
        });
        Ok(tasks)
    }

    pub fn snapshot(&self) -> Result<Vec<TaskSummary>, Error> {
        self.list(StatusFilter::All)
    }

    pub fn show(&self, id: &str) -> Result<TaskRecord, Error> {
        let (lane, path) = self.locate(id)?;
        let body = fs::read_to_string(&path)?;
        Ok(self.record(id, lane, &path, &body))
    }

    pub fn note(&self, id: &str, text: &str) -> Result<TaskRecord, Error> {
        let (lane, path) = self.locate(id)?;
        let mut body = fs::read_to_string(&path)?;
        if !body.ends_with('\n') {
            body.push('\n');
        }
        body.push('\n');
        body.push_str(text.trim_end());
        body.push('\n');
        fs::write(&path, &body)?;
        Ok(self.record(id, lane, &path, &body))
    }

    /// Append raw bytes to the current file for `id` (follows lane moves).
    pub fn append(&self, id: &str, bytes: &[u8]) -> Result<(), Error> {
        use std::io::Write;
        let (_, path) = self.locate(id)?;
        let mut file = fs::OpenOptions::new().append(true).open(&path)?;
        file.write_all(bytes)?;
        file.flush()?;
        Ok(())
    }

    pub fn start(&self, id: &str) -> Result<TaskRecord, Error> {
        let (src, _) = self.locate(id)?;
        if src == Lane::Done {
            return Err(Error::AlreadyDone(id.to_string()));
        }
        if src == Lane::Dropped {
            return Err(Error::AlreadyDropped(id.to_string()));
        }
        self.move_to(id, Lane::Doing)
    }

    /// Exclusive todo → doing. `None` if the task is already in doing or not in todo.
    pub fn claim(&self, id: &str) -> Result<Option<TaskRecord>, Error> {
        if !is_valid_id(id) {
            return Err(Error::InvalidId(id.to_string()));
        }
        let src = self.task_path(Lane::Todo, id);
        let dest = self.task_path(Lane::Doing, id);
        if dest.is_file() {
            return Ok(None);
        }
        match fs::rename(&src, &dest) {
            Ok(()) => {
                let body = fs::read_to_string(&dest)?;
                Ok(Some(self.record(id, Lane::Doing, &dest, &body)))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn done(&self, id: &str) -> Result<TaskRecord, Error> {
        let (src, _) = self.locate(id)?;
        if src == Lane::Dropped {
            return Err(Error::AlreadyDropped(id.to_string()));
        }
        self.move_to(id, Lane::Done)
    }

    pub fn drop(&self, id: &str) -> Result<TaskRecord, Error> {
        self.move_to(id, Lane::Dropped)
    }

    pub fn reopen(&self, id: &str) -> Result<TaskRecord, Error> {
        self.move_to(id, Lane::Todo)
    }

    fn move_to(&self, id: &str, dest: Lane) -> Result<TaskRecord, Error> {
        let (src, src_path) = self.locate(id)?;
        if src == dest {
            let body = fs::read_to_string(&src_path)?;
            return Ok(self.record(id, src, &src_path, &body));
        }
        let dest_path = self.task_path(dest, id);
        if dest_path.exists() {
            return Err(Error::Conflict(id.to_string()));
        }
        fs::rename(&src_path, &dest_path)?;
        let body = fs::read_to_string(&dest_path)?;
        Ok(self.record(id, dest, &dest_path, &body))
    }

    fn locate(&self, id: &str) -> Result<(Lane, PathBuf), Error> {
        if !is_valid_id(id) {
            return Err(Error::InvalidId(id.to_string()));
        }
        let mut found = None;
        for lane in Lane::all() {
            let path = self.task_path(*lane, id);
            if !path.is_file() {
                continue;
            }
            if found.is_some() {
                return Err(Error::Conflict(id.to_string()));
            }
            found = Some((*lane, path));
        }
        found.ok_or_else(|| Error::TaskNotFound(id.to_string()))
    }

    fn list_lane(&self, lane: Lane) -> Result<Vec<TaskSummary>, Error> {
        let mut tasks = Vec::new();
        let dir = board::lane_dir(&self.root, lane);
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if !is_valid_id(stem) {
                continue;
            }
            let body = fs::read_to_string(&path)?;
            tasks.push(TaskSummary {
                id: stem.to_string(),
                title: title_from_body(&body).unwrap_or_else(|| stem.to_string()),
                status: lane,
                path: display_path(&path),
            });
        }
        Ok(tasks)
    }

    fn unique_id(&self, base: &str) -> Result<String, Error> {
        let mut id = base.to_string();
        let mut n = 2u32;
        while Lane::all()
            .iter()
            .any(|lane| self.task_path(*lane, &id).exists())
        {
            id = format!("{base}-{n}");
            n += 1;
        }
        Ok(id)
    }

    fn task_path(&self, lane: Lane, id: &str) -> PathBuf {
        board::lane_dir(&self.root, lane).join(format!("{id}.md"))
    }

    fn record(&self, id: &str, lane: Lane, path: &Path, body: &str) -> TaskRecord {
        let (agents_path, agents) = board::load_agents(&self.root, lane);
        TaskRecord {
            id: id.to_string(),
            title: title_from_body(body).unwrap_or_else(|| id.to_string()),
            status: lane,
            path: display_path(path),
            body: body.to_string(),
            agents_path,
            agents,
        }
    }
}

fn display_path(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}
