use std::collections::HashMap;
use std::path::Path;

use crate::constants::{VALID_FIELDS, VALID_PRIORITIES, VALID_STATUSES};
use crate::filename::{format_filename, parse_filename};
use crate::frontmatter::{has_valid_frontmatter, parse_frontmatter_str, FRONTMATTER_OPEN};
use crate::tasks::task_files;
use crate::util::{is_valid_date, normalize_line_endings};

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
        return result; // empty directory is valid
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

        let content = match std::fs::read_to_string(path) {
            Ok(c) => normalize_line_endings(&c).into_owned(),
            Err(e) => {
                result.errors.push(format!("{name}: cannot read: {e}"));
                continue;
            }
        };

        if !content.starts_with(FRONTMATTER_OPEN) {
            result
                .errors
                .push(format!("{name}: missing YAML frontmatter (must start with ---)"));
            continue;
        }

        if !has_valid_frontmatter(&content) {
            result
                .errors
                .push(format!("{name}: malformed YAML frontmatter (no closing ---)"));
            continue;
        }

        let fields = parse_frontmatter_str(&content);

        // status
        match fields.get("status").map(|s| s.as_str()) {
            None => result
                .errors
                .push(format!("{name}: missing 'status' field")),
            Some(s) if !VALID_STATUSES.contains(&s) => result.errors.push(format!(
                "{name}: invalid status '{s}' (valid: {})",
                VALID_STATUSES.join(", ")
            )),
            _ => {}
        }

        // priority
        match fields.get("priority").map(|s| s.as_str()) {
            None => result
                .errors
                .push(format!("{name}: missing 'priority' field")),
            Some(p) if !VALID_PRIORITIES.contains(&p) => result.errors.push(format!(
                "{name}: invalid priority '{p}' (valid: {})",
                VALID_PRIORITIES.join(", ")
            )),
            _ => {}
        }

        // created
        match fields.get("created").map(|s| s.as_str()) {
            None => result
                .errors
                .push(format!("{name}: missing 'created' field")),
            Some(d) if !is_valid_date(d) => result.errors.push(format!(
                "{name}: invalid 'created' date format (expected YYYY-MM-DD)"
            )),
            _ => {}
        }

        // artifact
        match fields.get("artifact").map(|s| s.as_str()) {
            None => result.errors.push(format!(
                "{name}: missing 'artifact' field (what file or system change does this task produce?)"
            )),
            Some("") => result.errors.push(format!(
                "{name}: 'artifact' field is empty (must name a concrete output, e.g. a file path, config change, or commit)"
            )),
            _ => {}
        }

        // unknown fields
        let mut unknown: Vec<&str> = fields
            .keys()
            .map(|k| k.as_str())
            .filter(|k| !VALID_FIELDS.contains(k))
            .collect();
        if !unknown.is_empty() {
            unknown.sort_unstable();
            result.errors.push(format!(
                "{name}: unknown field(s): {} (valid: {})",
                unknown.join(", "),
                VALID_FIELDS.join(", ")
            ));
        }

        // filename vs frontmatter consistency
        if let Some((id, _, _, slug)) = parse_filename(&name) {
            if let (Some(status), Some(priority)) =
                (fields.get("status"), fields.get("priority"))
            {
                let expected = format_filename(&id, priority, status, &slug);
                if name != expected {
                    result.errors.push(format!(
                        "{name}: filename doesn't match frontmatter, expected: {expected}"
                    ));
                }
            }
            id_map.entry(id).or_default().push(name.clone());
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
