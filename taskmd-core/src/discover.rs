//! Discovery of a project's taskmd tasks directory.
//!
//! taskmd 1.0 marks a tasks directory with a `_TEMPLATE.md` sentinel file
//! (see [`crate::constants::TEMPLATE_FILENAME`]). Given a starting directory
//! — normally the current working directory — this module scans the immediate
//! children for that marker.
//!
//! Two layers are offered:
//!
//! - [`candidates`] / [`discover`] report *facts*: which subdirectories carry
//!   the marker, with no opinion about what to do when zero or several match.
//!   This mirrors the Python CLI's auto-detect, which surfaces an error in
//!   both of those cases.
//! - [`discover_or_default`] applies a *policy*: prefer the conventional
//!   `tasks` name, otherwise the lexically-first candidate, otherwise fall
//!   back to the bare name `tasks`. It never fails, so editor integrations
//!   and other tools that want a usable path without prompting can call it.
//!
//! All functions return / carry **relative directory names**, not absolute
//! paths: callers already hold the directory they scanned, so the absolute
//! path is `dir.join(name)`, and the bare name is what gets interpolated into
//! commit messages, prompts, `format!("{name}/{filename}")`, etc.

use crate::constants::{DEFAULT_TASKS_DIR_NAME, TASKS_DIR_PREFIX, TEMPLATE_FILENAME};
use std::path::{Path, PathBuf};

/// Outcome of scanning a directory for a taskmd tasks directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Discovery {
    /// Exactly one candidate. Holds its bare directory name; join it onto the
    /// scanned directory for an absolute path.
    Found(String),
    /// No immediate child of the scanned directory carries the marker file
    /// (or the directory could not be read).
    NotFound,
    /// Two or more candidates, names sorted lexically. The caller must pick
    /// one (e.g. ask the user, or apply [`discover_or_default`]'s policy).
    Ambiguous(Vec<String>),
}

/// Names of every immediate subdirectory of `dir` that looks like a taskmd
/// tasks directory: the name starts with [`TASKS_DIR_PREFIX`] and the
/// directory contains a [`TEMPLATE_FILENAME`] file. The result is sorted, so
/// it is deterministic regardless of filesystem iteration order.
///
/// IO errors (unreadable `dir`, races, permission denied on a child) are
/// treated as "no match" rather than surfaced — this matches the Python CLI
/// and keeps discovery total.
pub fn candidates(dir: &Path) -> Vec<String> {
    let mut matches: Vec<String> = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return matches;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(TASKS_DIR_PREFIX) {
            continue;
        }
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        if entry.path().join(TEMPLATE_FILENAME).is_file() {
            matches.push(name);
        }
    }
    matches.sort();
    matches
}

/// Classify the candidates under `dir` (see [`candidates`]) into the
/// [`Discovery`] cases. `dir` is typically [`std::env::current_dir`]'s result.
pub fn discover(dir: &Path) -> Discovery {
    let mut found = candidates(dir);
    match found.len() {
        0 => Discovery::NotFound,
        1 => Discovery::Found(found.pop().expect("len checked == 1")),
        _ => Discovery::Ambiguous(found),
    }
}

/// Resolve a tasks-directory name under `dir` with a never-fails policy:
///
/// 1. If a candidate named exactly `tasks` exists, use it.
/// 2. Otherwise use the lexically-first candidate.
/// 3. Otherwise fall back to the bare name `tasks` — so a project that has
///    not run `taskmd init` yet still gets a sensible relative path, and the
///    directory springs into existence the first time something writes to it.
///
/// Returns a relative [`PathBuf`] (a single component); the absolute path is
/// `dir.join(...)`.
pub fn discover_or_default(dir: &Path) -> PathBuf {
    let found = candidates(dir);
    if found.iter().any(|n| n == DEFAULT_TASKS_DIR_NAME) {
        return PathBuf::from(DEFAULT_TASKS_DIR_NAME);
    }
    match found.into_iter().next() {
        Some(name) => PathBuf::from(name),
        None => PathBuf::from(DEFAULT_TASKS_DIR_NAME),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Create `dir/<name>/` and, unless `marked` is false, drop a
    /// `_TEMPLATE.md` inside it.
    fn make_dir(root: &Path, name: &str, marked: bool) {
        let d = root.join(name);
        std::fs::create_dir(&d).unwrap();
        if marked {
            std::fs::write(d.join(TEMPLATE_FILENAME), "# Task Title\n").unwrap();
        }
    }

    #[test]
    fn no_candidates_is_not_found() {
        let tmp = TempDir::new().unwrap();
        make_dir(tmp.path(), "tasks", false); // task-prefixed but unmarked
        make_dir(tmp.path(), "src", false);
        assert_eq!(candidates(tmp.path()), Vec::<String>::new());
        assert_eq!(discover(tmp.path()), Discovery::NotFound);
    }

    #[test]
    fn single_marked_dir_is_found() {
        let tmp = TempDir::new().unwrap();
        make_dir(tmp.path(), "tasks", true);
        assert_eq!(candidates(tmp.path()), vec!["tasks".to_string()]);
        assert_eq!(discover(tmp.path()), Discovery::Found("tasks".to_string()));
    }

    #[test]
    fn non_task_prefixed_marked_dir_is_ignored() {
        let tmp = TempDir::new().unwrap();
        make_dir(tmp.path(), "todo", true); // marked, but wrong prefix
        assert_eq!(discover(tmp.path()), Discovery::NotFound);
    }

    #[test]
    fn multiple_marked_dirs_are_ambiguous_and_sorted() {
        let tmp = TempDir::new().unwrap();
        make_dir(tmp.path(), "tasks-archive", true);
        make_dir(tmp.path(), "tasks", true);
        assert_eq!(
            discover(tmp.path()),
            Discovery::Ambiguous(vec!["tasks".to_string(), "tasks-archive".to_string()])
        );
    }

    #[test]
    fn or_default_prefers_conventional_name() {
        let tmp = TempDir::new().unwrap();
        make_dir(tmp.path(), "tasks-archive", true);
        make_dir(tmp.path(), "tasks", true);
        assert_eq!(discover_or_default(tmp.path()), PathBuf::from("tasks"));
    }

    #[test]
    fn or_default_picks_lexically_first_when_no_conventional() {
        let tmp = TempDir::new().unwrap();
        make_dir(tmp.path(), "task-z", true);
        make_dir(tmp.path(), "task-a", true);
        assert_eq!(discover_or_default(tmp.path()), PathBuf::from("task-a"));
    }

    #[test]
    fn or_default_falls_back_to_tasks_when_nothing_matches() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(discover_or_default(tmp.path()), PathBuf::from("tasks"));
    }

    #[test]
    fn unreadable_dir_is_not_found() {
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("does-not-exist");
        assert_eq!(discover(&missing), Discovery::NotFound);
        assert_eq!(discover_or_default(&missing), PathBuf::from("tasks"));
    }
}
