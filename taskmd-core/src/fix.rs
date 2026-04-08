use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::date::infer_created_date;
use crate::filename::format_filename;
use crate::ids::{needs_migration, parse_id_parts, prefix_for};
use crate::util::{is_valid_date, normalize_line_endings};
use crate::tasks::{parse_task_file, task_files};

/// Maximum sequence number that fits in the 3-digit NNN suffix.
/// Files with a sequence above this cannot be migrated automatically.
const MAX_SEQ: u32 = 999;

// Matches "created: <anything>" at the start of a line (multiline mode).
static CREATED_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^created:.*$").expect("hardcoded regex is valid"));

/// Compute the human-readable fix summary from the three change counters.
///
/// This is the single canonical implementation; the Python `FixResult.summary()`
/// delegates here via the `_core.fix_summary` binding.
pub fn fix_summary(patched: usize, renamed: usize, migrated: usize) -> String {
    if patched == 0 && renamed == 0 && migrated == 0 {
        return "All files already correct".to_string();
    }
    let mut parts: Vec<String> = vec![];
    if patched > 0 {
        parts.push(format!("patched {patched} file(s)"));
    }
    if renamed > 0 {
        parts.push(format!("renamed {renamed} file(s)"));
    }
    if migrated > 0 {
        parts.push(format!("migrated {migrated} file(s) to numeric ID format"));
    }
    let joined = parts.join(", ");
    let mut chars = joined.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

pub struct FixResult {
    pub patched: usize,
    pub renamed: usize,
    pub migrated: usize,
    /// Per-file patch details: `(filename, inferred_date)`.
    pub patches: Vec<(String, String)>,
    /// Per-file rename details: `(old_filename, new_filename)`.
    pub renames: Vec<(String, String)>,
    pub errors: Vec<String>,
}

impl FixResult {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn summary(&self) -> String {
        fix_summary(self.patched, self.renamed, self.migrated)
    }
}

/// Auto-fix task files: inject missing `created`, rename to match frontmatter,
/// and migrate IDs with non-matching prefixes to the current numeric format.
pub fn fix(tasks_dir: &Path) -> FixResult {
    let mut result = FixResult {
        patched: 0,
        renamed: 0,
        migrated: 0,
        patches: vec![],
        renames: vec![],
        errors: vec![],
    };

    if !tasks_dir.exists() {
        return result;
    }

    let files = match task_files(tasks_dir) {
        Ok(f) => f,
        Err(e) => {
            result.errors.push(format!("cannot read directory: {e}"));
            return result;
        }
    };

    let prefix = prefix_for(tasks_dir);

    // Track sequences already claimed (by existing correct-prefix files and
    // by files migrated earlier in this loop) to avoid collisions.
    let mut used_seqs: std::collections::HashSet<u32> = std::collections::HashSet::new();

    // Pre-populate with sequences from files that already have the correct prefix.
    for path in &files {
        if let Some(task) = parse_task_file(path) {
            let (pfx, seq) = parse_id_parts(&task.id);
            if pfx == prefix {
                used_seqs.insert(seq);
            }
        }
    }

    for path in &files {
        let task = match parse_task_file(path) {
            Some(t) => t,
            None => {
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                result.errors.push(format!("{name}: could not parse file"));
                continue;
            }
        };

        let name = task
            .path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let mut fields = task.fields.clone();

        // ── Fix missing or malformed 'created' ───────────────────────────────
        let needs_patch = !fields.contains_key("created")
            || !is_valid_date(fields["created"].as_str());

        if needs_patch {
            let created = infer_created_date(&task.path);
            let mut content = match std::fs::read_to_string(&task.path) {
                Ok(c) => normalize_line_endings(&c).into_owned(),
                Err(e) => {
                    result.errors.push(format!("{name}: cannot read: {e}"));
                    continue;
                }
            };

            // Only match `created:` inside the frontmatter block (between
            // the opening `---\n` and the closing `\n---\n`), not in the body.
            let fm_end = content[4..].find("\n---\n").map(|p| p + 4);
            let has_created_in_fm = fm_end
                .map(|end| CREATED_RE.is_match(&content[..end]))
                .unwrap_or(false);

            if has_created_in_fm {
                let end = fm_end.unwrap();
                let replaced = CREATED_RE
                    .replacen(&content[..end], 1, format!("created: {created}").as_str())
                    .into_owned();
                content = format!("{replaced}{}", &content[end..]);
            } else {
                content = content.replacen("---\n", &format!("---\ncreated: {created}\n"), 1);
            }

            if let Err(e) = std::fs::write(&task.path, &content) {
                result.errors.push(format!("{name}: cannot write: {e}"));
                continue;
            }

            fields.insert("created".to_string(), created.clone());
            result.patches.push((name.clone(), created));
            result.patched += 1;
        }

        // ── Guard: need status + priority to proceed ─────────────────────────
        let (status, priority) = match (fields.get("status"), fields.get("priority")) {
            (Some(s), Some(p)) => (s.clone(), p.clone()),
            _ => {
                result.errors.push(format!(
                    "{name}: missing status or priority in frontmatter"
                ));
                continue;
            }
        };

        // ── Migrate any ID whose prefix doesn't match ──────────────────────
        let mut task_id = task.id.clone();
        if needs_migration(&task_id, &prefix) {
            let (_, mut seq) = parse_id_parts(&task_id);
            if seq > MAX_SEQ {
                result.errors.push(format!(
                    "{name}: task sequence {seq} exceeds {MAX_SEQ}, \
                     cannot migrate to 3-digit format"
                ));
                continue;
            }
            // Bump sequence if it collides with an already-claimed ID.
            while used_seqs.contains(&seq) {
                seq += 1;
            }
            if seq > MAX_SEQ {
                result.errors.push(format!(
                    "{name}: no available sequence after collision avoidance"
                ));
                continue;
            }
            used_seqs.insert(seq);
            task_id = format!("{prefix}{seq:03}");
            result.migrated += 1;
        }

        // ── Rename to match frontmatter ──────────────────────────────────────
        let expected = format_filename(&task_id, &priority, &status, &task.slug);

        if name != expected {
            let new_path = tasks_dir.join(&expected);
            if new_path.exists() {
                result
                    .errors
                    .push(format!("{name}: cannot rename to {expected}, file exists"));
                continue;
            }

            if let Err(e) = std::fs::rename(&task.path, &new_path) {
                result.errors.push(format!("{name}: cannot rename: {e}"));
                continue;
            }

            result.renames.push((name, expected));
            result.renamed += 1;
        }
    }

    result
}
