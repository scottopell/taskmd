use std::path::{Path, PathBuf};

use crate::constants::{Priority, Status};
use crate::error::Error;
use crate::filename::{format_filename, parse_filename};

/// A parsed task file. All fields are derived from the filename — the file's
/// body is free-form markdown and is not parsed.
#[derive(Debug, Clone)]
pub struct TaskFile {
    pub path: PathBuf,
    pub id: String,
    pub priority: Priority,
    pub status: Status,
    pub slug: String,
}

pub fn is_template(path: &Path) -> bool {
    path.file_name().map_or(false, |n| n == "_TEMPLATE.md")
}

/// Ancillary files have a second dot in the stem, e.g. `0042-p2-ready--foo.qaplan.md`.
pub fn is_ancillary(path: &Path) -> bool {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map_or(false, |s| s.contains('.'))
}

/// Return all main task `.md` files (sorted, excluding template and ancillary).
pub fn task_files(tasks_dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(tasks_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().map_or(false, |e| e == "md")
                && !is_template(p)
                && !is_ancillary(p)
        })
        .collect();
    files.sort();
    Ok(files)
}

/// Parse a task file from a path. Returns `None` if the filename doesn't match.
pub fn parse_task_file(path: &Path) -> Option<TaskFile> {
    let name = path.file_name()?.to_str()?;
    let parsed = parse_filename(name)?;

    Some(TaskFile {
        path: path.to_path_buf(),
        id: parsed.id,
        priority: parsed.priority,
        status: parsed.status,
        slug: parsed.slug,
    })
}

/// Return all parseable task files in `tasks_dir`, sorted by ID.
pub fn list_tasks(tasks_dir: &Path) -> Vec<TaskFile> {
    if !tasks_dir.exists() {
        return vec![];
    }
    let paths = match task_files(tasks_dir) {
        Ok(p) => p,
        Err(_) => return vec![],
    };
    let mut tasks: Vec<TaskFile> = paths.iter().filter_map(|p| parse_task_file(p)).collect();
    tasks.sort_by(|a, b| a.id.cmp(&b.id));
    tasks
}

/// Find a single task by its ID. Returns `None` if not found.
pub fn find_task_by_id(tasks_dir: &Path, id: &str) -> Option<TaskFile> {
    let paths = task_files(tasks_dir).ok()?;
    for path in paths {
        if let Some(task) = parse_task_file(&path) {
            if task.id == id {
                return Some(task);
            }
        }
    }
    None
}

/// Change a task file's status by renaming the file.
///
/// Returns `(old_filename, new_filename)` on success.
pub fn rename_status(
    tasks_dir: &Path,
    id: &str,
    new_status: Status,
) -> Result<(String, String), Error> {
    let task = find_task_by_id(tasks_dir, id).ok_or_else(|| Error::TaskNotFound {
        id: id.to_string(),
    })?;

    let old_name = task
        .path
        .file_name()
        .expect("task path has filename")
        .to_string_lossy()
        .to_string();

    let new_name = format_filename(&task.id, task.priority, new_status, &task.slug);
    let new_path = tasks_dir.join(&new_name);

    if new_path.exists() && new_path != task.path {
        return Err(Error::TargetExists {
            path: new_path.clone(),
        });
    }

    std::fs::rename(&task.path, &new_path)?;

    Ok((old_name, new_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_task(id: &str, priority: Priority, status: Status, slug: &str) -> (TempDir, String) {
        let tmp = TempDir::new().unwrap();
        let filename = format_filename(id, priority, status, slug);
        fs::write(tmp.path().join(&filename), "# task body\n").unwrap();
        (tmp, filename)
    }

    #[test]
    fn rename_status_renames_file() {
        let (tmp, _) = setup_task("34001", Priority::P2, Status::Ready, "my-task");
        let (old, new) = rename_status(tmp.path(), "34001", Status::Done).unwrap();
        assert!(old.contains("ready"));
        assert!(new.contains("done"));
        assert!(tmp.path().join(&new).exists());
        assert!(!tmp.path().join(&old).exists());
    }

    #[test]
    fn rename_status_accepts_all_valid_statuses() {
        for &status in Status::ALL {
            let (tmp, _) = setup_task("34001", Priority::P2, Status::Ready, "my-task");
            assert!(
                rename_status(tmp.path(), "34001", status).is_ok(),
                "rename_status rejected valid status '{status}'",
            );
        }
    }

    #[test]
    fn rename_status_file_still_discoverable() {
        let (tmp, _) = setup_task("34001", Priority::P2, Status::Ready, "my-task");
        rename_status(tmp.path(), "34001", Status::Done).unwrap();
        let found = find_task_by_id(tmp.path(), "34001");
        assert!(found.is_some());
        assert_eq!(found.unwrap().status, Status::Done);
    }

    #[test]
    fn rename_status_unknown_id_errors() {
        let (tmp, _) = setup_task("34001", Priority::P2, Status::Ready, "x");
        assert!(matches!(
            rename_status(tmp.path(), "34999", Status::Done),
            Err(Error::TaskNotFound { .. })
        ));
    }
}
