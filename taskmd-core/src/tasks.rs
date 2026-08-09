use std::path::{Path, PathBuf};

use crate::constants::{Priority, Status, TEMPLATE_FILENAME};
use crate::error::Error;
use crate::filename::{derive_slug, format_filename, parse_filename};

/// REQ-TM-001: A parsed task file. All fields are derived from the filename;
/// the body is free-form markdown and is not parsed.
#[derive(Debug, Clone)]
pub struct TaskFile {
    pub path: PathBuf,
    pub id: String,
    pub priority: Priority,
    pub status: Status,
    pub slug: String,
}

impl TaskFile {
    /// The filename component of `path`, without directory.
    /// Panics only if the path has no filename (impossible for a TaskFile
    /// since parse_task_file requires file_name() to be Some).
    pub fn filename(&self) -> &str {
        self.path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("TaskFile::path always has a UTF-8 filename")
    }
}

pub fn is_template(path: &Path) -> bool {
    path.file_name().map_or(false, |n| n == TEMPLATE_FILENAME)
}

/// REQ-TM-008: Ancillary files have a second dot in the stem, such as
/// `0042-p2-ready--foo.qaplan.md`.
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

/// Find tasks whose slug matches `slug` exactly. Returns all matches —
/// slugs are not unique across statuses or priorities. Returns an empty
/// Vec if no tasks match or `tasks_dir` is unreadable.
pub fn find_task_by_slug(tasks_dir: &Path, slug: &str) -> Vec<TaskFile> {
    let paths = match task_files(tasks_dir) {
        Ok(p) => p,
        Err(_) => return vec![],
    };
    paths
        .iter()
        .filter_map(|p| parse_task_file(p))
        .filter(|t| t.slug == slug)
        .collect()
}

/// All ancillary files associated with the task `id` in `tasks_dir`.
///
/// Ancillary files share the task's full filename stem and append a
/// secondary tag before the extension, e.g. `34042-p2-ready--foo.qaplan.md`
/// or `34042-p2-ready--foo.screenshot.png` are both ancillaries of
/// `34042-p2-ready--foo.md`. Extension is not constrained — anything
/// matching the prefix-with-trailing-dot convention is returned.
///
/// Returns paths in alphabetical order. Empty if the main task is not
/// found, or if no ancillaries exist.
pub fn ancillary_files_for(tasks_dir: &Path, id: &str) -> Vec<PathBuf> {
    let task = match find_task_by_id(tasks_dir, id) {
        Some(t) => t,
        None => return vec![],
    };

    let prefix = format!(
        "{}-{}-{}--{}.",
        task.id,
        task.priority.as_str(),
        task.status.as_str(),
        task.slug
    );

    let entries = match std::fs::read_dir(tasks_dir) {
        Ok(e) => e,
        Err(_) => return vec![],
    };

    let mut matches: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            !is_template(p)
                && is_ancillary(p)
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .map_or(false, |n| n.starts_with(&prefix))
        })
        .collect();
    matches.sort();
    matches
}

/// Fields to change on an existing task. `None` means "leave unchanged".
#[derive(Debug, Clone, Default)]
pub struct TaskUpdate {
    pub priority: Option<Priority>,
    pub status: Option<Status>,
    pub slug: Option<String>,
}

/// The result of `update_task`. Returns the old and new filenames so callers
/// can log or display the rename. If no fields actually changed, `old == new`
/// and no filesystem operation occurred.
#[derive(Debug, Clone)]
pub struct UpdateResult {
    pub old_filename: String,
    pub new_filename: String,
}

/// REQ-TM-003: Apply `update` to the task with `id` by renaming the file.
///
/// Atomic: a single rename produces all field changes at once. If `update` is
/// effectively a no-op (every field is `None` or matches the current value),
/// returns `UpdateResult` where `old_filename == new_filename` and skips the
/// filesystem call.
///
/// Errors:
///   - `Error::TaskNotFound { id }` if no task matches `id`.
///   - `Error::InvalidSlug { got, reason }` if `update.slug` is `Some` but
///     contains no alphanumeric characters (matches `create_task`'s rule).
///   - `Error::TargetExists { path }` if the new filename collides with an
///     existing different file.
///   - `Error::Io` for filesystem failures.
pub fn update_task(
    tasks_dir: &Path,
    id: &str,
    update: TaskUpdate,
) -> Result<UpdateResult, Error> {
    let task = find_task_by_id(tasks_dir, id).ok_or_else(|| Error::TaskNotFound {
        id: id.to_string(),
    })?;

    let new_priority = update.priority.unwrap_or(task.priority);
    let new_status = update.status.unwrap_or(task.status);
    let new_slug = match update.slug {
        Some(s) => {
            // Match create_task's rule: reject inputs that derive_slug would
            // silently turn into "untitled".
            if !s.chars().any(|c| c.is_ascii_alphanumeric()) {
                return Err(Error::InvalidSlug {
                    got: s.clone(),
                    reason: "must contain at least one alphanumeric character".to_string(),
                });
            }
            derive_slug(&s)
        }
        None => task.slug.clone(),
    };

    let old_name = task
        .path
        .file_name()
        .expect("task path has filename")
        .to_string_lossy()
        .to_string();

    let new_name = format_filename(&task.id, new_priority, new_status, &new_slug);

    if new_name == old_name {
        return Ok(UpdateResult {
            old_filename: old_name.clone(),
            new_filename: old_name,
        });
    }

    let new_path = tasks_dir.join(&new_name);

    if new_path.exists() && new_path != task.path {
        return Err(Error::TargetExists {
            path: new_path.clone(),
        });
    }

    std::fs::rename(&task.path, &new_path)?;

    Ok(UpdateResult {
        old_filename: old_name,
        new_filename: new_name,
    })
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
    fn update_task_status_only_renames_file() {
        let (tmp, _) = setup_task("34001", Priority::P2, Status::Ready, "my-task");
        let result = update_task(
            tmp.path(),
            "34001",
            TaskUpdate {
                status: Some(Status::Done),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(result.old_filename.contains("ready"));
        assert!(result.new_filename.contains("done"));
        assert!(tmp.path().join(&result.new_filename).exists());
        assert!(!tmp.path().join(&result.old_filename).exists());
    }

    #[test]
    fn update_task_priority_only() {
        let (tmp, _) = setup_task("34001", Priority::P2, Status::Ready, "my-task");
        let result = update_task(
            tmp.path(),
            "34001",
            TaskUpdate {
                priority: Some(Priority::P0),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            result.new_filename.contains("p0-ready"),
            "expected new filename to contain 'p0-ready', got {:?}",
            result.new_filename
        );
        assert!(tmp.path().join(&result.new_filename).exists());
        assert!(!tmp.path().join(&result.old_filename).exists());
    }

    #[test]
    fn update_task_slug_only() {
        let (tmp, _) = setup_task("34001", Priority::P2, Status::Ready, "my-old-slug");
        let result = update_task(
            tmp.path(),
            "34001",
            TaskUpdate {
                slug: Some("Brand New Slug".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            result.new_filename.contains("--brand-new-slug.md"),
            "expected new filename to contain '--brand-new-slug.md', got {:?}",
            result.new_filename
        );
        assert!(tmp.path().join(&result.new_filename).exists());
        assert!(!tmp.path().join(&result.old_filename).exists());
    }

    #[test]
    fn update_task_combined_change() {
        let (tmp, _) = setup_task("34001", Priority::P2, Status::Ready, "old-slug");
        let result = update_task(
            tmp.path(),
            "34001",
            TaskUpdate {
                priority: Some(Priority::P0),
                status: Some(Status::Done),
                slug: Some("New Title".to_string()),
            },
        )
        .unwrap();
        assert!(result.new_filename.contains("p0-done"));
        assert!(result.new_filename.contains("--new-title.md"));
        assert!(tmp.path().join(&result.new_filename).exists());
        assert!(!tmp.path().join(&result.old_filename).exists());

        // Only one main task file should exist after the rename.
        let files = task_files(tmp.path()).unwrap();
        assert_eq!(files.len(), 1);

        // And find_task_by_id locates it with merged fields.
        let found = find_task_by_id(tmp.path(), "34001").unwrap();
        assert_eq!(found.priority, Priority::P0);
        assert_eq!(found.status, Status::Done);
        assert_eq!(found.slug, "new-title");
    }

    #[test]
    fn update_task_noop_returns_same_filename() {
        let (tmp, original_filename) =
            setup_task("34001", Priority::P2, Status::Ready, "my-task");
        let result = update_task(tmp.path(), "34001", TaskUpdate::default()).unwrap();
        assert_eq!(result.old_filename, result.new_filename);
        assert_eq!(result.old_filename, original_filename);
        assert!(tmp.path().join(&original_filename).exists());
    }

    #[test]
    fn update_task_unknown_id_errors() {
        let (tmp, _) = setup_task("34001", Priority::P2, Status::Ready, "x");
        assert!(matches!(
            update_task(
                tmp.path(),
                "34999",
                TaskUpdate {
                    status: Some(Status::Done),
                    ..Default::default()
                }
            ),
            Err(Error::TaskNotFound { .. })
        ));
    }

    #[test]
    fn update_task_target_exists_errors() {
        // Task A lives at p0/ready/task-a — sorts before any p2 sibling, so
        // find_task_by_id picks it first.
        let (tmp, _) = setup_task("34001", Priority::P0, Status::Ready, "task-a");
        // Pre-place a squatter at the path A would take if its priority were
        // bumped to P2. (Same id is unavoidable — filenames embed id — but
        // sort order guarantees A is what gets discovered.)
        let occupied = format_filename("34001", Priority::P2, Status::Ready, "task-a");
        fs::write(tmp.path().join(&occupied), "# squatter\n").unwrap();

        let result = update_task(
            tmp.path(),
            "34001",
            TaskUpdate {
                priority: Some(Priority::P2),
                ..Default::default()
            },
        );
        assert!(
            matches!(result, Err(Error::TargetExists { .. })),
            "expected TargetExists, got {result:?}"
        );
        // And the original file should be untouched.
        let original = format_filename("34001", Priority::P0, Status::Ready, "task-a");
        assert!(tmp.path().join(&original).exists());
        assert!(tmp.path().join(&occupied).exists());
    }

    #[test]
    fn update_task_invalid_slug_errors() {
        let (tmp, _) = setup_task("34001", Priority::P2, Status::Ready, "my-task");
        let result = update_task(
            tmp.path(),
            "34001",
            TaskUpdate {
                slug: Some("   ".to_string()),
                ..Default::default()
            },
        );
        assert!(
            matches!(result, Err(Error::InvalidSlug { .. })),
            "expected InvalidSlug, got {result:?}"
        );
    }

    #[test]
    fn update_task_file_still_discoverable_after_change() {
        let (tmp, _) = setup_task("34001", Priority::P2, Status::Ready, "my-task");
        update_task(
            tmp.path(),
            "34001",
            TaskUpdate {
                priority: Some(Priority::P0),
                status: Some(Status::Done),
                ..Default::default()
            },
        )
        .unwrap();
        let found = find_task_by_id(tmp.path(), "34001").unwrap();
        assert_eq!(found.priority, Priority::P0);
        assert_eq!(found.status, Status::Done);
    }

    #[test]
    fn task_file_filename_returns_basename() {
        let tmp = TempDir::new().unwrap();
        let filename = "0001-p2-ready--my-slug.md";
        let path = tmp.path().join(filename);
        fs::write(&path, "# body\n").unwrap();
        let task = parse_task_file(&path).unwrap();
        assert_eq!(task.filename(), filename);
    }

    #[test]
    fn find_task_by_slug_returns_match() {
        let (tmp, _) = setup_task("0001", Priority::P2, Status::Ready, "my-task");
        let matches = find_task_by_slug(tmp.path(), "my-task");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, "0001");
        assert_eq!(matches[0].slug, "my-task");
    }

    #[test]
    fn find_task_by_slug_no_match_returns_empty() {
        let (tmp, _) = setup_task("0001", Priority::P2, Status::Ready, "my-task");
        let matches = find_task_by_slug(tmp.path(), "does-not-exist");
        assert!(matches.is_empty());
    }

    #[test]
    fn find_task_by_slug_multiple_matches() {
        let tmp = TempDir::new().unwrap();
        let a = format_filename("0001", Priority::P2, Status::Ready, "shared");
        let b = format_filename("0002", Priority::P0, Status::Done, "shared");
        fs::write(tmp.path().join(&a), "# body\n").unwrap();
        fs::write(tmp.path().join(&b), "# body\n").unwrap();
        let matches = find_task_by_slug(tmp.path(), "shared");
        assert_eq!(matches.len(), 2);
        let ids: Vec<&str> = matches.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"0001"));
        assert!(ids.contains(&"0002"));
    }

    #[test]
    fn ancillary_files_for_returns_attachments() {
        let tmp = TempDir::new().unwrap();
        let main = format_filename("0001", Priority::P2, Status::Ready, "main");
        fs::write(tmp.path().join(&main), "# body\n").unwrap();
        let notes = "0001-p2-ready--main.notes.md";
        let plan = "0001-p2-ready--main.plan.md";
        fs::write(tmp.path().join(notes), "# notes\n").unwrap();
        fs::write(tmp.path().join(plan), "# plan\n").unwrap();
        // Unrelated task
        let other = format_filename("0002", Priority::P2, Status::Ready, "other");
        fs::write(tmp.path().join(&other), "# body\n").unwrap();

        let ancillaries = ancillary_files_for(tmp.path(), "0001");
        assert_eq!(ancillaries.len(), 2);
        let names: Vec<String> = ancillaries
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec![notes.to_string(), plan.to_string()]);
    }

    #[test]
    fn ancillary_files_for_picks_up_non_md_attachments() {
        let tmp = TempDir::new().unwrap();
        let main = format_filename("0001", Priority::P2, Status::Ready, "main");
        fs::write(tmp.path().join(&main), "# body\n").unwrap();
        let png = "0001-p2-ready--main.screenshot.png";
        let pdf = "0001-p2-ready--main.spec.pdf";
        let notes_md = "0001-p2-ready--main.notes.md";
        fs::write(tmp.path().join(png), b"\x89PNG").unwrap();
        fs::write(tmp.path().join(pdf), b"%PDF-1.4").unwrap();
        fs::write(tmp.path().join(notes_md), "# notes\n").unwrap();

        let ancillaries = ancillary_files_for(tmp.path(), "0001");
        let names: Vec<String> = ancillaries
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(
            names,
            vec![notes_md.to_string(), png.to_string(), pdf.to_string()],
            "ancillary helper must surface attachments regardless of extension"
        );
    }

    #[test]
    fn ancillary_files_for_no_main_returns_empty() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("0001-p2-ready--main.notes.md"),
            "# notes\n",
        )
        .unwrap();
        let ancillaries = ancillary_files_for(tmp.path(), "0001");
        assert!(ancillaries.is_empty());
    }

    #[test]
    fn ancillary_files_for_no_ancillaries_returns_empty() {
        let (tmp, _) = setup_task("0001", Priority::P2, Status::Ready, "main");
        let ancillaries = ancillary_files_for(tmp.path(), "0001");
        assert!(ancillaries.is_empty());
    }

    #[test]
    fn ancillary_files_for_does_not_match_other_tasks() {
        let tmp = TempDir::new().unwrap();
        let main_a = format_filename("0001", Priority::P2, Status::Ready, "alpha");
        let main_b = format_filename("0002", Priority::P2, Status::Ready, "alpha");
        fs::write(tmp.path().join(&main_a), "# body\n").unwrap();
        fs::write(tmp.path().join(&main_b), "# body\n").unwrap();
        // Ancillary belongs to 0002, not 0001.
        fs::write(
            tmp.path().join("0002-p2-ready--alpha.notes.md"),
            "# notes\n",
        )
        .unwrap();

        let ancillaries = ancillary_files_for(tmp.path(), "0001");
        assert!(
            ancillaries.is_empty(),
            "expected no ancillaries for id 0001, got {ancillaries:?}"
        );
    }
}
