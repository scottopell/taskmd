//! Atomic create-and-claim for new task files.
//!
//! `create_task` is the content-first counterpart to `next_id`: it allocates
//! the next ID, formats the filename, synthesizes frontmatter from the
//! supplied metadata, and writes the file in one step. The write uses O_EXCL
//! (`create_new`) so two concurrent callers in the same partition cannot both
//! claim the same ID — on collision we recompute `next_id` and retry.
//!
//! This exists primarily as ergonomics for agents that otherwise follow
//! on-disk ID patterns by example and skip `taskmd next` entirely.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::constants::{VALID_PRIORITIES, VALID_STATUSES};
use crate::date::today;
use crate::error::Error;
use crate::filename::{derive_slug, format_filename};
use crate::ids::next_id;

/// Upper bound on how many times we'll re-call `next_id` after O_EXCL collisions.
/// In practice this is only hit during an active race; 50 is far past anything
/// a real workload will produce.
const MAX_CREATE_RETRIES: u32 = 50;

/// Default body used when no body is supplied on stdin — matches the shape
/// of `_TEMPLATE.md` so downstream validation passes without hand-editing.
const DEFAULT_BODY: &str = "\
# Task Title

## Summary

Brief description of what needs to be done.

## Context

Why this task exists, any relevant background.

## Done When

- [ ] Criterion 1
- [ ] Criterion 2

## Notes

Any additional information.
";

/// Metadata returned to the caller after a successful write.
#[derive(Debug, Clone)]
pub struct CreatedTask {
    pub id: String,
    pub path: PathBuf,
    pub filename: String,
}

/// Allocate an ID, synthesize frontmatter, and atomically write a new task file.
///
/// `body` may be empty — a default skeleton is used. `body` must not itself
/// contain a frontmatter block; frontmatter is synthesized from the
/// `priority`/`status`/`artifact` arguments plus today's date.
pub fn create_task(
    tasks_dir: &Path,
    priority: &str,
    status: &str,
    slug: &str,
    artifact: &str,
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
    if artifact.trim().is_empty() {
        return Err(Error::InvalidValue(
            "artifact cannot be empty — name the file/change this task produces".into(),
        ));
    }
    // Artifact is the only user-supplied value injected into the synthesized
    // frontmatter verbatim, so it must stay on one line. A literal `\n---\n`
    // inside the value would otherwise close the frontmatter block early and
    // the resulting file would fail `taskmd validate`.
    if artifact.contains('\n') || artifact.contains('\r') {
        return Err(Error::InvalidValue(
            "artifact must be a single line (no newline or carriage return)".into(),
        ));
    }

    // Normalize slug through derive_slug; it's idempotent for already-slugified
    // input and a safety net for agents that pass titles or dirty strings.
    let slug = derive_slug(slug);
    if slug == "untitled" && slug.is_empty() {
        return Err(Error::InvalidValue("slug cannot be empty".into()));
    }

    if !tasks_dir.exists() {
        return Err(Error::NotFound(format!(
            "tasks directory does not exist: {} (run 'taskmd init' first)",
            tasks_dir.display()
        )));
    }

    let body_trimmed = if body.trim().is_empty() {
        DEFAULT_BODY.trim_end_matches('\n').to_string()
    } else {
        // Reject a frontmatter-in-body mistake early with a useful message.
        if body.trim_start().starts_with("---\n") || body.trim_start().starts_with("---\r\n") {
            return Err(Error::InvalidValue(
                "body appears to contain its own frontmatter; pass only the markdown body — \
                 frontmatter is synthesized from --priority/--status/--artifact"
                    .into(),
            ));
        }
        body.trim_end_matches('\n').to_string()
    };

    let created = today();

    for _ in 0..MAX_CREATE_RETRIES {
        let id = next_id(tasks_dir);
        let filename = format_filename(&id, priority, status, &slug);
        let path = tasks_dir.join(&filename);

        let content = format!(
            "---\ncreated: {created}\npriority: {priority}\nstatus: {status}\nartifact: {artifact}\n---\n\n{body_trimmed}\n"
        );

        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut f) => {
                f.write_all(content.as_bytes())?;
                return Ok(CreatedTask { id, path, filename });
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // Concurrent claimer beat us to this ID; recompute and retry.
                continue;
            }
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
        // create_task requires the dir to exist
        std::fs::create_dir_all(tmp.path()).ok();
        tmp
    }

    #[test]
    fn creates_task_file_with_frontmatter_and_body() {
        let tmp = tasks_dir();
        let r = create_task(
            tmp.path(),
            "p2",
            "ready",
            "fix-the-bug",
            "src/foo.rs",
            "My custom body.",
        )
        .unwrap();

        assert!(r.path.exists());
        assert!(r.filename.ends_with("-p2-ready--fix-the-bug.md"));
        let content = std::fs::read_to_string(&r.path).unwrap();
        assert!(content.starts_with("---\ncreated: "));
        assert!(content.contains("priority: p2"));
        assert!(content.contains("status: ready"));
        assert!(content.contains("artifact: src/foo.rs"));
        assert!(content.contains("My custom body."));
    }

    #[test]
    fn uses_default_body_when_empty() {
        let tmp = tasks_dir();
        let r = create_task(tmp.path(), "p2", "ready", "no-body", "src/x.rs", "").unwrap();
        let content = std::fs::read_to_string(&r.path).unwrap();
        assert!(content.contains("## Summary"));
        assert!(content.contains("## Done When"));
    }

    #[test]
    fn rejects_invalid_priority() {
        let tmp = tasks_dir();
        let r = create_task(tmp.path(), "p9", "ready", "s", "a", "");
        assert!(matches!(r, Err(Error::InvalidValue(_))));
    }

    #[test]
    fn rejects_invalid_status() {
        let tmp = tasks_dir();
        let r = create_task(tmp.path(), "p2", "pending", "s", "a", "");
        assert!(matches!(r, Err(Error::InvalidValue(_))));
    }

    #[test]
    fn rejects_empty_artifact() {
        let tmp = tasks_dir();
        let r = create_task(tmp.path(), "p2", "ready", "s", "   ", "");
        assert!(matches!(r, Err(Error::InvalidValue(_))));
    }

    #[test]
    fn rejects_newline_in_artifact() {
        let tmp = tasks_dir();
        // A literal `\n---\n` in artifact would close the frontmatter early
        // and let 'new' silently produce a file that 'validate' then rejects.
        let r = create_task(tmp.path(), "p2", "ready", "s", "src/x.rs\n---\nevil", "");
        assert!(matches!(r, Err(Error::InvalidValue(_))));

        // Plain \n is also rejected (would silently truncate at parse time).
        let r = create_task(tmp.path(), "p2", "ready", "s", "line1\nline2", "");
        assert!(matches!(r, Err(Error::InvalidValue(_))));

        // \r alone is also rejected.
        let r = create_task(tmp.path(), "p2", "ready", "s", "line1\rline2", "");
        assert!(matches!(r, Err(Error::InvalidValue(_))));
    }

    /// Regression: every input `create_task` accepts must produce a file that
    /// `taskmd validate` considers clean. This is the contract the user asked
    /// about explicitly.
    #[test]
    fn created_file_always_passes_validate() {
        let tmp = tasks_dir();
        // Exercise a mix of valid-but-unusual inputs
        create_task(tmp.path(), "p0", "ready", "Fix: The Bug!", "src/foo.rs", "").unwrap();
        create_task(tmp.path(), "p4", "in-progress", "x", "path/with colons:and-stuff", "body").unwrap();
        create_task(tmp.path(), "p2", "brainstorming", "a".repeat(200).as_str(), "src/y.rs", "").unwrap();

        let r = crate::validate::validate(tmp.path());
        assert!(r.ok(), "validate failed after create_task: {:?}", r.errors);
    }

    #[test]
    fn rejects_body_with_frontmatter() {
        let tmp = tasks_dir();
        let body = "---\nstatus: ready\n---\n\nhi";
        let r = create_task(tmp.path(), "p2", "ready", "s", "a", body);
        assert!(matches!(r, Err(Error::InvalidValue(_))));
    }

    #[test]
    fn rejects_missing_tasks_dir() {
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("does-not-exist");
        let r = create_task(&missing, "p2", "ready", "s", "a", "");
        assert!(matches!(r, Err(Error::NotFound(_))));
    }

    #[test]
    fn slug_is_normalized() {
        let tmp = tasks_dir();
        let r = create_task(tmp.path(), "p2", "ready", "Fix The Bug!", "src/x.rs", "").unwrap();
        assert!(r.filename.contains("--fix-the-bug.md"));
    }

    #[test]
    fn sequential_creates_yield_monotonic_ids() {
        let tmp = tasks_dir();
        let a = create_task(tmp.path(), "p2", "ready", "a", "src/a.rs", "").unwrap();
        let b = create_task(tmp.path(), "p2", "ready", "b", "src/b.rs", "").unwrap();
        assert_ne!(a.id, b.id);
        // Both IDs share the same 2-digit prefix and b's sequence > a's
        assert_eq!(a.id[..2], b.id[..2]);
        let a_seq: u32 = a.id[2..].parse().unwrap();
        let b_seq: u32 = b.id[2..].parse().unwrap();
        assert_eq!(b_seq, a_seq + 1);
    }

    #[test]
    fn oexcl_collision_triggers_retry() {
        // Simulate a squatter by pre-creating the file at the ID next_id will
        // return first. create_task should skip it and land on the next one.
        let tmp = tasks_dir();
        let squatter_id = next_id(tmp.path());
        let squatter = format_filename(&squatter_id, "p2", "ready", "squatter");
        std::fs::write(tmp.path().join(&squatter), "squat").unwrap();

        let r = create_task(tmp.path(), "p2", "ready", "winner", "src/x.rs", "").unwrap();
        assert_ne!(r.id, squatter_id);
        assert!(r.filename.contains("--winner.md"));
    }
}
