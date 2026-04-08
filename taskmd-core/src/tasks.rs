use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::filename::{format_filename, parse_filename};
use crate::frontmatter::{parse_frontmatter_file, FRONTMATTER_OPEN};
use crate::util::normalize_line_endings;

/// A parsed task file (filename + frontmatter combined).
#[derive(Debug, Clone)]
pub struct TaskFile {
    pub path: PathBuf,
    pub id: String,
    pub priority: String,
    pub status: String,
    pub slug: String,
    pub fields: HashMap<String, String>,
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

/// Parse a task file from a path.  Returns `None` if the filename doesn't match.
pub fn parse_task_file(path: &Path) -> Option<TaskFile> {
    let name = path.file_name()?.to_str()?;
    let (id, priority, status, slug) = parse_filename(name)?;
    let fields = parse_frontmatter_file(path).unwrap_or_default();

    Some(TaskFile {
        path: path.to_path_buf(),
        id,
        priority,
        status,
        slug,
        fields,
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

/// Find a single task by its ID.  Returns `None` if not found.
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

/// Update the `status` field in a frontmatter block in-place.
///
/// Replaces the `status: <old>` line inside the `---` delimiters.
/// Returns the content unchanged if no `status:` line is found.
pub fn update_status_in_content(content: &str, new_status: &str) -> String {
    let content = normalize_line_endings(content);

    if !content.starts_with(FRONTMATTER_OPEN) {
        return content.to_string();
    }

    let body_start = FRONTMATTER_OPEN.len();
    let end = match content[body_start..].find("\n---\n") {
        Some(pos) => body_start + pos,
        None => return content.to_string(),
    };

    let body = &content[body_start..end]; // no surrounding newlines
    let rest = &content[end..]; // starts with "\n---\n"

    let new_body: String = body
        .lines()
        .map(|line| {
            if let Some(colon) = line.find(':') {
                if line[..colon].trim() == "status" {
                    return format!("status: {new_status}");
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("---\n{new_body}{rest}")
}

/// Change a task file's status: update frontmatter then rename the file.
///
/// Returns `(old_filename, new_filename)` on success.
pub fn rename_status(
    tasks_dir: &Path,
    id: &str,
    new_status: &str,
) -> Result<(String, String), Error> {
    let task = find_task_by_id(tasks_dir, id)
        .ok_or_else(|| Error::NotFound(format!("task {id} not found in {}", tasks_dir.display())))?;

    let old_name = task
        .path
        .file_name()
        .expect("task path has filename")
        .to_string_lossy()
        .to_string();
    let new_name = format_filename(&task.id, &task.priority, new_status, &task.slug);
    let new_path = tasks_dir.join(&new_name);

    if new_path.exists() && new_path != task.path {
        return Err(Error::Conflict(format!(
            "cannot rename {old_name} to {new_name}: target already exists"
        )));
    }

    // Update frontmatter first
    let content = std::fs::read_to_string(&task.path)?;
    let updated = update_status_in_content(&content, new_status);
    std::fs::write(&task.path, updated)?;

    // Rename
    std::fs::rename(&task.path, &new_path)?;

    Ok((old_name, new_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_status_replaces_status_line() {
        let content = "---\nstatus: ready\npriority: p2\n---\n\nBody\n";
        let updated = update_status_in_content(content, "done");
        assert!(updated.contains("status: done"));
        assert!(!updated.contains("status: ready"));
        assert!(updated.contains("priority: p2"));
        assert!(updated.contains("Body"));
    }

    #[test]
    fn update_status_no_frontmatter() {
        let content = "no frontmatter here";
        let updated = update_status_in_content(content, "done");
        assert_eq!(updated, content);
    }
}
