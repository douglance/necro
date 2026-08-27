use schemars::JsonSchema;
use serde::Serialize;

const MAX_SLUG_CHARS: usize = 80;

/// Parent folder that is the task status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Lane {
    Todo,
    Doing,
    Done,
    Dropped,
}

impl Lane {
    pub fn all() -> &'static [Lane] {
        &[Self::Todo, Self::Doing, Self::Done, Self::Dropped]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::Doing => "doing",
            Self::Done => "done",
            Self::Dropped => "dropped",
        }
    }

    /// Folder name under `.necro`. Numbered so lexical order matches the pipeline.
    pub fn dir_name(self) -> &'static str {
        match self {
            Self::Todo => "1-todo",
            Self::Doing => "2-doing",
            Self::Done => "3-done",
            Self::Dropped => "4-dropped",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "todo" => Some(Self::Todo),
            "doing" => Some(Self::Doing),
            "done" => Some(Self::Done),
            "dropped" => Some(Self::Dropped),
            _ => None,
        }
    }
}

/// Filter for `list`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFilter {
    Todo,
    Doing,
    Done,
    Dropped,
    All,
}

impl StatusFilter {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "todo" => Some(Self::Todo),
            "doing" => Some(Self::Doing),
            "done" => Some(Self::Done),
            "dropped" => Some(Self::Dropped),
            "all" => Some(Self::All),
            _ => None,
        }
    }

    pub fn lanes(self) -> &'static [Lane] {
        match self {
            Self::Todo => &[Lane::Todo],
            Self::Doing => &[Lane::Doing],
            Self::Done => &[Lane::Done],
            Self::Dropped => &[Lane::Dropped],
            Self::All => Lane::all(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct TaskSummary {
    pub id: String,
    pub title: String,
    pub status: Lane,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct TaskRecord {
    pub id: String,
    pub title: String,
    pub status: Lane,
    pub path: String,
    pub body: String,
    pub agents_path: Option<String>,
    pub agents: Option<String>,
}

pub fn is_valid_id(id: &str) -> bool {
    if id.is_empty() {
        return false;
    }
    let mut chars = id.chars().peekable();
    loop {
        let Some(start) = chars.next() else {
            return true;
        };
        if !start.is_ascii_lowercase() && !start.is_ascii_digit() {
            return false;
        }
        while matches!(chars.peek(), Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit()) {
            chars.next();
        }
        match chars.peek() {
            None => return true,
            Some('-') => {
                chars.next();
                if !matches!(chars.peek(), Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit())
                {
                    return false;
                }
            }
            Some(_) => return false,
        }
    }
}

pub fn slugify(title: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;
    for ch in title.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            slug.push(c);
            prev_dash = false;
        } else if !slug.is_empty() && !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.chars().count() > MAX_SLUG_CHARS {
        slug = slug.chars().take(MAX_SLUG_CHARS).collect();
        while slug.ends_with('-') {
            slug.pop();
        }
    }
    if slug.is_empty() {
        "task".to_string()
    } else {
        slug
    }
}

pub fn title_from_body(body: &str) -> Option<String> {
    for line in body.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("# ") else {
            continue;
        };
        let title = rest.trim();
        if !title.is_empty() {
            return Some(title.to_string());
        }
    }
    None
}

pub fn default_body(title: &str) -> String {
    format!("# {title}\n")
}

pub fn body_with_optional_extra(title: &str, extra: Option<&str>) -> String {
    match extra {
        Some(extra) if extra.trim_start().starts_with("# ") => {
            if extra.ends_with('\n') {
                extra.to_string()
            } else {
                format!("{extra}\n")
            }
        }
        Some(extra) if extra.is_empty() => default_body(title),
        Some(extra) => {
            let extra = extra.trim_end_matches('\n');
            format!("# {title}\n\n{extra}\n")
        }
        None => default_body(title),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_strings_stay_unnumbered_and_dirs_are_numbered() {
        assert_eq!(Lane::Todo.as_str(), "todo");
        assert_eq!(Lane::Doing.as_str(), "doing");
        assert_eq!(Lane::Done.as_str(), "done");
        assert_eq!(Lane::Dropped.as_str(), "dropped");
        assert_eq!(Lane::Todo.dir_name(), "1-todo");
        assert_eq!(Lane::Doing.dir_name(), "2-doing");
        assert_eq!(Lane::Done.dir_name(), "3-done");
        assert_eq!(Lane::Dropped.dir_name(), "4-dropped");
        assert_eq!(Lane::parse("todo"), Some(Lane::Todo));
        assert_eq!(Lane::parse("1-todo"), None);
        assert_eq!(StatusFilter::parse("doing"), Some(StatusFilter::Doing));
        assert_eq!(StatusFilter::parse("2-doing"), None);
    }

    #[test]
    fn slugify_table() {
        assert_eq!(slugify("Fix auth refresh"), "fix-auth-refresh");
        assert_eq!(slugify("  Hello, World!  "), "hello-world");
        assert_eq!(slugify("!!!"), "task");
        assert_eq!(slugify("Café au lait"), "caf-au-lait");
        assert_eq!(slugify("a".repeat(90).as_str()).len(), 80);
    }

    #[test]
    fn title_from_body_table() {
        assert_eq!(
            title_from_body("# Fix auth\n\nDetails"),
            Some("Fix auth".into())
        );
        assert_eq!(title_from_body("no heading"), None);
        assert_eq!(
            title_from_body("intro\n# Later heading\n"),
            Some("Later heading".into())
        );
    }

    #[test]
    fn id_pattern() {
        assert!(is_valid_id("fix-auth"));
        assert!(is_valid_id("a"));
        assert!(is_valid_id("ab2"));
        assert!(!is_valid_id(""));
        assert!(!is_valid_id("-a"));
        assert!(!is_valid_id("a-"));
        assert!(!is_valid_id("Fix"));
        assert!(!is_valid_id("a--b"));
    }
}
