use std::collections::HashMap;
use std::path::Path;

use crate::filename::parse_filename;
use crate::tasks::task_files;

pub struct ValidationResult {
    pub errors: Vec<String>,
    pub file_count: usize,
}

impl ValidationResult {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn validate(tasks_dir: &Path) -> ValidationResult {
    let mut result = ValidationResult {
        errors: vec![],
        file_count: 0,
    };

    if !tasks_dir.exists() {
        return result; // empty/missing directory is valid
    }

    let files = match task_files(tasks_dir) {
        Ok(f) => f,
        Err(e) => {
            result.errors.push(format!("cannot read directory: {e}"));
            return result;
        }
    };

    result.file_count = files.len();

    let mut id_map: HashMap<String, Vec<String>> = HashMap::new();

    for path in &files {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        match parse_filename(&name) {
            Some((id, _, _, _)) => {
                id_map.entry(id).or_default().push(name);
            }
            None => {
                result.errors.push(format!(
                    "{name}: filename doesn't match pattern DDNNN-pX-status--slug.md"
                ));
            }
        }
    }

    // Duplicate IDs (sorted for deterministic output)
    let mut sorted_ids: Vec<&String> = id_map.keys().collect();
    sorted_ids.sort();
    for id in sorted_ids {
        let names = &id_map[id];
        if names.len() > 1 {
            let mut sorted_names = names.clone();
            sorted_names.sort();
            result.errors.push(format!(
                "duplicate task id {id}: {} — run 'taskmd fix' to auto-renumber",
                sorted_names.join(", ")
            ));
        }
    }

    result
}
