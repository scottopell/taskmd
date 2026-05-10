//! Atomic create-and-claim for new task files.
//!
//! `create_task` allocates the next ID, formats the filename, and writes the
//! file in one step. The write uses O_EXCL (`create_new`) so two concurrent
//! callers in the same partition cannot both claim the same ID — on collision
//! we recompute `next_id` and retry.
//!
//! Body is required: a task that can't be described in at least one line is a
//! placeholder, and placeholders inflate triage surface area.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::constants::{VALID_PRIORITIES, VALID_STATUSES};
use crate::error::Error;
use crate::filename::{derive_slug, format_filename};
use crate::ids::next_id;

/// Upper bound on retries after O_EXCL collisions (only hit during an active race).
const MAX_CREATE_RETRIES: u32 = 50;

#[derive(Debug, Clone)]
pub struct CreatedTask {
    pub id: String,
    pub path: PathBuf,
    pub filename: String,
}

/// Allocate an ID and atomically write a new task file containing only `body`.
pub fn create_task(
    tasks_dir: &Path,
    priority: &str,
    status: &str,
    slug: &str,
    body: &str,
) -> Result<CreatedTask, Error> {
    if !VALID_PRIORITIES.contains(&priority) {
        return Err(Error::InvalidValue(format!(
            "invalid priority '{priority}', expected one of: {}",
            VALID_PRIORITIES.join(", ")
        )));
    }
    if !VALID_STATUSES.contains(&status) {
        return Err(Error::InvalidValue(format!(
            "invalid status '{status}', expected one of: {}",
            VALID_STATUSES.join(", ")
        )));
    }

    // Validate the raw input before normalization. `derive_slug` falls back
    // to "untitled" for any input that produces an empty slug (whitespace
    // only, all punctuation, etc.), which would silently mask a missing slug.
    if !slug.chars().any(|c| c.is_ascii_alphanumeric()) {
        return Err(Error::InvalidValue(
            "slug must contain at least one alphanumeric character".into(),
        ));
    }
    let slug = derive_slug(slug);

    if !tasks_dir.exists() {
        return Err(Error::NotFound(format!(
            "tasks directory does not exist: {} (run 'taskmd init' first)",
            tasks_dir.display()
        )));
    }

    if body.trim().is_empty() {
        return Err(Error::InvalidValue(
            "body is required — pipe at least one line of description on stdin. \
             A task with no body is a placeholder; if you cannot describe it, \
             do not create it yet."
                .into(),
        ));
    }

    let body_trimmed = body.trim_end_matches('\n').to_string();
    let content = format!("{body_trimmed}\n");

    for _ in 0..MAX_CREATE_RETRIES {
        let id = next_id(tasks_dir);
        let filename = format_filename(&id, priority, status, &slug);
        let path = tasks_dir.join(&filename);

        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut f) => {
                f.write_all(content.as_bytes())?;
                return Ok(CreatedTask { id, path, filename });
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(Error::Io(e)),
        }
    }

    Err(Error::Conflict(format!(
        "failed to allocate a unique task ID in {} after {MAX_CREATE_RETRIES} attempts",
        tasks_dir.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tasks_dir() -> TempDir {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path()).ok();
        tmp
    }

    #[test]
    fn creates_task_file_with_body_only() {
        let tmp = tasks_dir();
        let r = create_task(tmp.path(), "p2", "ready", "fix-the-bug", "My custom body.").unwrap();

        assert!(r.path.exists());
        assert!(r.filename.ends_with("-p2-ready--fix-the-bug.md"));
        let content = std::fs::read_to_string(&r.path).unwrap();
        assert!(!content.starts_with("---"));
        assert_eq!(content, "My custom body.\n");
    }

    #[test]
    fn rejects_empty_body() {
        let tmp = tasks_dir();
        for body in ["", "   ", "\n", "\t\n  \n"] {
            let r = create_task(tmp.path(), "p2", "ready", "s", body);
            assert!(matches!(r, Err(Error::InvalidValue(_))), "body {body:?}");
        }
    }

    #[test]
    fn rejects_invalid_priority() {
        let tmp = tasks_dir();
        let r = create_task(tmp.path(), "p9", "ready", "s", "body");
        assert!(matches!(r, Err(Error::InvalidValue(_))));
    }

    #[test]
    fn rejects_invalid_status() {
        let tmp = tasks_dir();
        let r = create_task(tmp.path(), "p2", "pending", "s", "body");
        assert!(matches!(r, Err(Error::InvalidValue(_))));
    }

    #[test]
    fn rejects_slug_with_no_alphanumerics() {
        // Inputs that derive_slug would silently turn into "untitled" must
        // be rejected up front, otherwise callers can construct nonsensical
        // tasks by passing whitespace, punctuation, or empty strings.
        let tmp = tasks_dir();
        for slug in ["", "   ", "\t", "!!!", "---", "  / \n "] {
            let r = create_task(tmp.path(), "p2", "ready", slug, "body");
            assert!(
                matches!(r, Err(Error::InvalidValue(_))),
                "expected InvalidValue for slug {slug:?}, got {r:?}",
            );
        }
    }

    /// Regression: every input `create_task` accepts must produce a file that
    /// `taskmd validate` considers clean.
    #[test]
    fn created_file_always_passes_validate() {
        let tmp = tasks_dir();
        create_task(tmp.path(), "p0", "ready", "Fix: The Bug!", "body").unwrap();
        create_task(tmp.path(), "p4", "in-progress", "x", "body").unwrap();
        create_task(tmp.path(), "p2", "brainstorming", "a".repeat(200).as_str(), "body").unwrap();

        let r = crate::validate::validate(tmp.path());
        assert!(r.ok(), "validate failed: {:?}", r.errors);
    }

    #[test]
    fn rejects_missing_tasks_dir() {
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("does-not-exist");
        let r = create_task(&missing, "p2", "ready", "s", "body");
        assert!(matches!(r, Err(Error::NotFound(_))));
    }

    #[test]
    fn slug_is_normalized() {
        let tmp = tasks_dir();
        let r = create_task(tmp.path(), "p2", "ready", "Fix The Bug!", "body").unwrap();
        assert!(r.filename.contains("--fix-the-bug.md"));
    }

    #[test]
    fn sequential_creates_yield_monotonic_ids() {
        let tmp = tasks_dir();
        let a = create_task(tmp.path(), "p2", "ready", "a", "body").unwrap();
        let b = create_task(tmp.path(), "p2", "ready", "b", "body").unwrap();
        assert_ne!(a.id, b.id);
        assert_eq!(a.id[..2], b.id[..2]);
        let a_seq: u32 = a.id[2..].parse().unwrap();
        let b_seq: u32 = b.id[2..].parse().unwrap();
        assert_eq!(b_seq, a_seq + 1);
    }

    #[test]
    fn oexcl_collision_triggers_retry() {
        let tmp = tasks_dir();
        let squatter_id = next_id(tmp.path());
        let squatter = format_filename(&squatter_id, "p2", "ready", "squatter");
        std::fs::write(tmp.path().join(&squatter), "squat").unwrap();

        let r = create_task(tmp.path(), "p2", "ready", "winner", "body").unwrap();
        assert_ne!(r.id, squatter_id);
        assert!(r.filename.contains("--winner.md"));
    }
}
