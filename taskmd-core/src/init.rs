use crate::constants::TEMPLATE_FILENAME;
use std::path::{Path, PathBuf};

const TEMPLATE_CONTENT: &str = "\
# Task Title

## Summary

Brief description of what needs to be done.

## Context

Why this task exists, any relevant background.

## Done When

- [ ] Criterion 1
- [ ] Criterion 2
- [ ] Criterion 3

## Notes

Any additional information, links, or considerations.
";

pub struct InitResult {
    pub tasks_dir: PathBuf,
    /// Paths created (directory and template file).
    pub created: Vec<String>,
    pub error: Option<String>,
}

impl InitResult {
    pub fn ok(&self) -> bool {
        self.error.is_none()
    }
}

/// REQ-TM-007: Initialise a tasks directory with a `_TEMPLATE.md` file.
///
/// Fails if `tasks_dir` already exists.
pub fn init(tasks_dir: &Path) -> InitResult {
    let mut result = InitResult {
        tasks_dir: tasks_dir.to_path_buf(),
        created: vec![],
        error: None,
    };

    if tasks_dir.exists() {
        result.error = Some(format!(
            "tasks directory already exists at {}",
            tasks_dir.display()
        ));
        return result;
    }

    if let Err(e) = std::fs::create_dir_all(tasks_dir) {
        result.error = Some(format!("cannot create directory: {e}"));
        return result;
    }

    result.created.push(format!("{}/", tasks_dir.display()));

    let template_path = tasks_dir.join(TEMPLATE_FILENAME);
    if let Err(e) = std::fs::write(&template_path, TEMPLATE_CONTENT) {
        result.error = Some(format!("cannot write template: {e}"));
        return result;
    }

    result.created.push(template_path.to_string_lossy().into_owned());

    result
}

pub struct EnsureResult {
    pub tasks_dir: PathBuf,
    /// Paths created on this call (empty if everything was already in place).
    pub created: Vec<String>,
    pub error: Option<String>,
}

impl EnsureResult {
    pub fn ok(&self) -> bool {
        self.error.is_none()
    }
}

/// Idempotent variant of `init`. Creates the tasks directory and the
/// `_TEMPLATE.md` file only if they're missing. Safe to call repeatedly.
pub fn ensure_initialized(tasks_dir: &Path) -> EnsureResult {
    let mut result = EnsureResult {
        tasks_dir: tasks_dir.to_path_buf(),
        created: vec![],
        error: None,
    };

    if !tasks_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(tasks_dir) {
            result.error = Some(format!("cannot create directory: {e}"));
            return result;
        }
        result.created.push(format!("{}/", tasks_dir.display()));
    }

    let template_path = tasks_dir.join(TEMPLATE_FILENAME);
    if !template_path.exists() {
        if let Err(e) = std::fs::write(&template_path, TEMPLATE_CONTENT) {
            result.error = Some(format!("cannot write template: {e}"));
            return result;
        }
        result
            .created
            .push(template_path.to_string_lossy().into_owned());
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn ensure_initialized_creates_missing_dir() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("nonexistent-tasks");
        let result = ensure_initialized(&target);
        assert!(result.ok());
        assert!(target.exists());
        assert!(target.join("_TEMPLATE.md").exists());
        assert_eq!(result.created.len(), 2);
    }

    #[test]
    fn ensure_initialized_idempotent() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("tasks");

        let first = ensure_initialized(&target);
        assert!(first.ok());
        assert_eq!(first.created.len(), 2);

        let second = ensure_initialized(&target);
        assert!(second.ok());
        assert_eq!(second.created.len(), 0);
    }

    #[test]
    fn ensure_initialized_fills_in_missing_template() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("tasks");
        std::fs::create_dir(&target).unwrap();
        // Directory exists, template does not.

        let result = ensure_initialized(&target);
        assert!(result.ok());
        assert!(target.join("_TEMPLATE.md").exists());
        assert_eq!(result.created.len(), 1);
    }

    #[test]
    fn ensure_initialized_leaves_existing_template_alone() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("tasks");
        std::fs::create_dir(&target).unwrap();
        let template_path = target.join("_TEMPLATE.md");
        let custom = "# My Custom Template\n";
        std::fs::write(&template_path, custom).unwrap();

        let result = ensure_initialized(&target);
        assert!(result.ok());
        assert_eq!(result.created.len(), 0);
        let preserved = std::fs::read_to_string(&template_path).unwrap();
        assert_eq!(preserved, custom);
    }
}
