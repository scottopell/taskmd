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

/// Initialise a tasks directory with a `_TEMPLATE.md` file.
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

    let template_path = tasks_dir.join("_TEMPLATE.md");
    if let Err(e) = std::fs::write(&template_path, TEMPLATE_CONTENT) {
        result.error = Some(format!("cannot write template: {e}"));
        return result;
    }

    result.created.push(template_path.to_string_lossy().into_owned());

    result
}
