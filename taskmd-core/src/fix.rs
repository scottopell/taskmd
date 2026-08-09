use std::path::{Path, PathBuf};

use crate::filename::{format_filename, parse_filename};
use crate::ids::{needs_migration, next_id, parse_id_parts, prefix_for};
use crate::tasks::{parse_task_file, task_files};
use crate::util::normalize_line_endings;

/// Maximum sequence number that fits in the 3-digit NNN suffix.
/// Files with a sequence above this cannot be migrated automatically.
const MAX_SEQ: u32 = 999;

/// How `fix` should treat task files that still carry legacy YAML frontmatter.
///
/// Frontmatter is no longer part of the task format, but pre-1.0 files in
/// existing repos still have it. Stripping it is destructive (the YAML block
/// is removed from the file body), so the user must opt in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrateMode {
    /// Default: refuse to run if any file has frontmatter, returning an error
    /// that names the files and points the user at `--migrate` / `--no-migrate`.
    Prompt,
    /// Strip frontmatter from every file that has it before doing the rest
    /// of the fix work. Destructive — caller is responsible for committing
    /// first.
    Migrate,
    /// Skip the frontmatter check entirely. Files keep whatever frontmatter
    /// they have; `fix` just does ID migration and dup renumber.
    Skip,
}

/// Strip a leading YAML frontmatter block from `content`, returning the
/// remaining body. Returns `None` if the content does not start with a
/// well-formed frontmatter block (`---\n...\n---\n`).
///
/// The returned body has any leading newlines after the closing `---`
/// trimmed, so a typical file with a blank line between the closing `---`
/// and the H1 will simply start at the H1 after stripping.
fn strip_frontmatter(content: &str) -> Option<String> {
    let normalized = normalize_line_endings(content);
    let s: &str = &normalized;
    let open = "---\n";
    if !s.starts_with(open) {
        return None;
    }
    let close = "\n---\n";
    let after_open = open.len();
    let close_at = s[after_open..].find(close)?;
    let body_start = after_open + close_at + close.len();
    let body = s[body_start..].trim_start_matches('\n');
    Some(body.to_string())
}

/// True if `content` starts with a well-formed YAML frontmatter block.
pub fn has_frontmatter(content: &str) -> bool {
    strip_frontmatter(content).is_some()
}

/// Compute the human-readable fix summary from the change counters.
pub fn fix_summary(
    renamed: usize,
    migrated: usize,
    renumbered: usize,
    frontmatter_stripped: usize,
) -> String {
    if renamed == 0 && migrated == 0 && renumbered == 0 && frontmatter_stripped == 0 {
        return "All files already correct".to_string();
    }
    let mut parts: Vec<String> = vec![];
    if frontmatter_stripped > 0 {
        parts.push(format!(
            "stripped frontmatter from {frontmatter_stripped} file(s)"
        ));
    }
    if renamed > 0 {
        parts.push(format!("renamed {renamed} file(s)"));
    }
    if migrated > 0 {
        parts.push(format!("migrated {migrated} file(s) to numeric ID format"));
    }
    if renumbered > 0 {
        parts.push(format!("renumbered {renumbered} duplicate ID(s)"));
    }
    let joined = parts.join(", ");
    let mut chars = joined.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[derive(Debug)]
pub struct FixResult {
    pub renamed: usize,
    pub migrated: usize,
    /// Per-file rename details: `(old_filename, new_filename)`.
    pub renames: Vec<(String, String)>,
    /// Per-file renumber details: `(old_id, new_id, old_filename, new_filename)`.
    ///
    /// These are files that shared a duplicate task ID with another file; the
    /// "winner" keeps the original ID (picked via `tiebreaker_key`), every
    /// other duplicate gets a fresh ID via `next_id`. Cross-references to
    /// `old_id` elsewhere in the repo are intentionally NOT rewritten — this
    /// list is the hand-off so a human can grep and patch.
    pub renumbered: Vec<(String, String, String, String)>,
    /// Filenames whose YAML frontmatter was stripped (only set when
    /// `MigrateMode::Migrate` is passed).
    pub frontmatter_stripped: Vec<String>,
    /// Filenames detected as having frontmatter when `MigrateMode::Prompt`
    /// is in effect. Pairs with a single error in `errors` to let callers
    /// surface a list to the user.
    pub frontmatter_pending: Vec<String>,
    pub errors: Vec<String>,
}

impl FixResult {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn summary(&self) -> String {
        fix_summary(
            self.renamed,
            self.migrated,
            self.renumbered.len(),
            self.frontmatter_stripped.len(),
        )
    }
}

/// Tiebreaker key for picking the "winner" among files sharing a duplicate ID.
///
/// Ordering (ascending = winner):
///   1. Earliest git-first-seen commit date (follows renames via `git log --follow`).
///   2. Earliest filesystem mtime (nanosecond precision).
///   3. Lexicographic filename (deterministic across platforms).
fn tiebreaker_key(path: &Path) -> (Option<i64>, Option<i128>, String) {
    let git = git_first_seen_unix(path);
    let mtime = mtime_unix(path);
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    (git, mtime, name)
}

fn sort_by_tiebreaker(paths: &mut [PathBuf]) {
    paths.sort_by(|a, b| {
        let (ga, ma, na) = tiebreaker_key(a);
        let (gb, mb, nb) = tiebreaker_key(b);
        let cmp_git = match (ga, gb) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        };
        if cmp_git != std::cmp::Ordering::Equal {
            return cmp_git;
        }
        let cmp_mtime = match (ma, mb) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        };
        if cmp_mtime != std::cmp::Ordering::Equal {
            return cmp_mtime;
        }
        na.cmp(&nb)
    });
}

fn git_first_seen_unix(path: &Path) -> Option<i64> {
    let parent = path.parent()?;
    let output = std::process::Command::new("git")
        .args(["log", "--follow", "--diff-filter=A", "--format=%at"])
        .arg(path)
        .current_dir(parent)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let last = stdout.trim().lines().next_back()?.trim().to_string();
    if last.is_empty() {
        return None;
    }
    last.parse::<i64>().ok()
}

fn mtime_unix(path: &Path) -> Option<i128> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    let d = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(d.as_nanos() as i128)
}

/// REQ-TM-005: Auto-fix task files: optionally strip legacy frontmatter,
/// migrate legacy IDs, and renumber files that share a duplicate ID.
pub fn fix(tasks_dir: &Path, migrate_mode: MigrateMode) -> FixResult {
    let mut result = FixResult {
        renamed: 0,
        migrated: 0,
        renames: vec![],
        renumbered: vec![],
        frontmatter_stripped: vec![],
        frontmatter_pending: vec![],
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

    // Frontmatter migration runs before everything else so the rest of fix
    // operates on already-migrated content.
    match migrate_mode {
        MigrateMode::Skip => {}
        MigrateMode::Prompt => {
            for path in &files {
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                let content = match std::fs::read_to_string(path) {
                    Ok(c) => c,
                    Err(e) => {
                        // Don't silently skip — the prompt-mode guarantee is
                        // "no file slips past with frontmatter still in it",
                        // so an unreadable file must surface as an error.
                        result.errors.push(format!(
                            "{name}: cannot read for frontmatter check: {e}"
                        ));
                        continue;
                    }
                };
                if has_frontmatter(&content) {
                    result.frontmatter_pending.push(name);
                }
            }
            if !result.frontmatter_pending.is_empty() {
                let n = result.frontmatter_pending.len();
                result.errors.push(format!(
                    "{n} task file(s) have legacy YAML frontmatter that must be \
                     removed. Run 'taskmd fix --migrate' to strip it (destructive; \
                     commit first), or 'taskmd fix --no-migrate' to skip the check"
                ));
                return result;
            }
            // If any file was unreadable, bail before doing further work —
            // the user needs to resolve the IO error first.
            if !result.errors.is_empty() {
                return result;
            }
        }
        MigrateMode::Migrate => {
            for path in &files {
                let content = match std::fs::read_to_string(path) {
                    Ok(c) => c,
                    Err(e) => {
                        let name = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned();
                        result.errors.push(format!("{name}: cannot read: {e}"));
                        continue;
                    }
                };
                if let Some(new_body) = strip_frontmatter(&content) {
                    if let Err(e) = std::fs::write(path, &new_body) {
                        let name = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned();
                        result.errors.push(format!("{name}: cannot write: {e}"));
                        continue;
                    }
                    result.frontmatter_stripped.push(
                        path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                    );
                }
            }
        }
    }

    let prefix = prefix_for(tasks_dir);

    // Track sequences already claimed (by existing correct-prefix files and
    // by files migrated earlier in this loop) to avoid collisions.
    let mut used_seqs: std::collections::HashSet<u32> = std::collections::HashSet::new();
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
        }

        let expected = format_filename(&task_id, task.priority, task.status, &task.slug);

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

            if task_id != task.id {
                result.migrated += 1;
            }
            result.renames.push((name, expected));
            result.renamed += 1;
        }
    }

    renumber_duplicates(tasks_dir, &mut result);

    result
}

/// Detect files sharing the same parsed task ID and renumber the "losers".
fn renumber_duplicates(tasks_dir: &Path, result: &mut FixResult) {
    let files = match task_files(tasks_dir) {
        Ok(f) => f,
        Err(e) => {
            result
                .errors
                .push(format!("renumber: cannot re-read directory: {e}"));
            return;
        }
    };

    let mut by_id: std::collections::HashMap<String, Vec<PathBuf>> =
        std::collections::HashMap::new();
    for path in &files {
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(s) => s,
            None => continue,
        };
        if let Some(parsed) = parse_filename(name) {
            by_id.entry(parsed.id).or_default().push(path.clone());
        }
    }

    let mut ids: Vec<String> = by_id.keys().cloned().collect();
    ids.sort();

    for id in ids {
        let mut group = by_id.remove(&id).unwrap();
        if group.len() < 2 {
            continue;
        }

        sort_by_tiebreaker(&mut group);
        let losers: Vec<PathBuf> = group.into_iter().skip(1).collect();

        for loser_path in losers {
            let task = match parse_task_file(&loser_path) {
                Some(t) => t,
                None => {
                    let name = loser_path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    result
                        .errors
                        .push(format!("{name}: cannot re-parse for renumber"));
                    continue;
                }
            };
            let old_name = task
                .path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();
            let old_id = task.id.clone();

            let new_id = next_id(tasks_dir);
            let (_, new_seq) = parse_id_parts(&new_id);
            if new_seq > MAX_SEQ {
                result.errors.push(format!(
                    "{old_name}: cannot renumber — prefix space appears exhausted \
                     (next_id returned '{new_id}')"
                ));
                continue;
            }

            let new_filename = format_filename(&new_id, task.priority, task.status, &task.slug);
            let new_path = tasks_dir.join(&new_filename);
            if new_path.exists() {
                result.errors.push(format!(
                    "{old_name}: cannot renumber to {new_filename}, file exists"
                ));
                continue;
            }
            if let Err(e) = std::fs::rename(&task.path, &new_path) {
                result
                    .errors
                    .push(format!("{old_name}: cannot renumber: {e}"));
                continue;
            }

            result
                .renumbered
                .push((old_id, new_id, old_name, new_filename));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_task(dir: &Path, id: &str, priority: &str, status: &str, slug: &str) -> PathBuf {
        let filename = format!("{id}-{priority}-{status}--{slug}.md");
        let path = dir.join(&filename);
        fs::write(&path, format!("# {slug}\n")).unwrap();
        path
    }

    #[test]
    fn no_duplicates_is_a_noop() {
        let tmp = TempDir::new().unwrap();
        let prefix = prefix_for(tmp.path());
        write_task(tmp.path(), &format!("{prefix}001"), "p2", "ready", "a");
        write_task(tmp.path(), &format!("{prefix}002"), "p2", "ready", "b");
        let r = fix(tmp.path(), MigrateMode::Skip);
        assert!(r.ok(), "{:?}", r.errors);
        assert_eq!(r.renumbered.len(), 0);
    }

    #[test]
    fn two_duplicates_one_renumbered() {
        let tmp = TempDir::new().unwrap();
        let prefix = prefix_for(tmp.path());
        let id = format!("{prefix}001");
        let winner = write_task(tmp.path(), &id, "p2", "ready", "winner");
        std::thread::sleep(std::time::Duration::from_millis(50));
        let loser = write_task(tmp.path(), &id, "p1", "done", "loser");

        let r = fix(tmp.path(), MigrateMode::Skip);
        assert_eq!(r.renumbered.len(), 1, "{:?}", r);
        let (old_id, new_id, old_name, new_name) = &r.renumbered[0];
        assert_eq!(old_id, &id);
        assert_ne!(new_id, &id);
        assert!(old_name.contains("loser"));
        assert!(new_name.contains("loser"));

        assert!(winner.exists());
        assert!(!loser.exists());
        assert!(tmp.path().join(new_name).exists());
    }

    #[test]
    fn three_duplicates_two_renumbered() {
        let tmp = TempDir::new().unwrap();
        let prefix = prefix_for(tmp.path());
        let id = format!("{prefix}001");
        write_task(tmp.path(), &id, "p2", "ready", "a-first");
        std::thread::sleep(std::time::Duration::from_millis(50));
        write_task(tmp.path(), &id, "p2", "ready", "b-second");
        std::thread::sleep(std::time::Duration::from_millis(50));
        write_task(tmp.path(), &id, "p2", "ready", "c-third");

        let r = fix(tmp.path(), MigrateMode::Skip);
        assert!(r.ok(), "{:?}", r.errors);
        assert_eq!(r.renumbered.len(), 2);

        let new_ids: std::collections::HashSet<_> =
            r.renumbered.iter().map(|(_, n, _, _)| n.clone()).collect();
        assert_eq!(new_ids.len(), 2);
        for new_id in &new_ids {
            assert_ne!(new_id, &id);
        }
    }

    #[test]
    fn duplicates_across_priorities_and_statuses() {
        let tmp = TempDir::new().unwrap();
        let prefix = prefix_for(tmp.path());
        let id = format!("{prefix}042");
        write_task(tmp.path(), &id, "p2", "ready", "alpha");
        std::thread::sleep(std::time::Duration::from_millis(50));
        write_task(tmp.path(), &id, "p0", "done", "beta");

        let r = fix(tmp.path(), MigrateMode::Skip);
        assert_eq!(r.renumbered.len(), 1);
        let (_, _, _, new_name) = &r.renumbered[0];
        assert!(new_name.contains("-p0-done--beta.md"), "got: {new_name}");
    }

    #[test]
    fn tiebreaker_mtime_selects_earliest() {
        let tmp = TempDir::new().unwrap();
        let prefix = prefix_for(tmp.path());
        let id = format!("{prefix}100");
        let zebra = write_task(tmp.path(), &id, "p2", "ready", "zebra");
        std::thread::sleep(std::time::Duration::from_millis(50));
        let alpha = write_task(tmp.path(), &id, "p2", "ready", "alpha");

        let r = fix(tmp.path(), MigrateMode::Skip);
        assert_eq!(r.renumbered.len(), 1);
        let (_, _, old_name, _) = &r.renumbered[0];
        assert!(old_name.contains("alpha"));
        assert!(zebra.exists());
        assert!(!alpha.exists());
    }

    #[test]
    fn tiebreaker_lexicographic_for_nonexistent_paths() {
        let a = PathBuf::from("/nonexistent/34001-p2-ready--alpha.md");
        let b = PathBuf::from("/nonexistent/34001-p2-ready--bravo.md");
        let mut paths = vec![b.clone(), a.clone()];
        sort_by_tiebreaker(&mut paths);
        assert_eq!(paths[0], a);
    }

    #[test]
    fn renumbered_losers_are_not_counted_as_renames() {
        let tmp = TempDir::new().unwrap();
        let prefix = prefix_for(tmp.path());
        let id = format!("{prefix}001");
        write_task(tmp.path(), &id, "p2", "ready", "first");
        std::thread::sleep(std::time::Duration::from_millis(50));
        write_task(tmp.path(), &id, "p2", "ready", "second");

        let r = fix(tmp.path(), MigrateMode::Skip);
        assert_eq!(r.renamed, 0);
        assert_eq!(r.renames.len(), 0);
        assert_eq!(r.renumbered.len(), 1);
    }

    #[test]
    fn fix_is_idempotent_after_renumber() {
        let tmp = TempDir::new().unwrap();
        let prefix = prefix_for(tmp.path());
        let id = format!("{prefix}001");
        write_task(tmp.path(), &id, "p2", "ready", "a");
        std::thread::sleep(std::time::Duration::from_millis(50));
        write_task(tmp.path(), &id, "p2", "ready", "b");

        let r1 = fix(tmp.path(), MigrateMode::Skip);
        assert_eq!(r1.renumbered.len(), 1);

        let r2 = fix(tmp.path(), MigrateMode::Skip);
        assert_eq!(r2.renumbered.len(), 0);
        assert_eq!(r2.renamed, 0);
    }

    #[test]
    fn duplicates_post_legacy_migration() {
        let tmp = TempDir::new().unwrap();
        let prefix = prefix_for(tmp.path());
        write_task(tmp.path(), &format!("{prefix}042"), "p2", "ready", "existing");
        fs::write(
            tmp.path().join("0042-p2-ready--legacy.md"),
            "# legacy\n",
        )
        .unwrap();

        let r = fix(tmp.path(), MigrateMode::Skip);
        assert!(r.ok(), "{:?}", r.errors);
        assert!(tmp.path().join(format!("{prefix}042-p2-ready--existing.md")).exists());
        assert!(!tmp.path().join("0042-p2-ready--legacy.md").exists());
    }

    #[test]
    fn summary_reports_renumber_count() {
        assert_eq!(fix_summary(0, 0, 2, 0), "Renumbered 2 duplicate ID(s)");
        assert_eq!(
            fix_summary(1, 0, 1, 0),
            "Renamed 1 file(s), renumbered 1 duplicate ID(s)"
        );
        assert_eq!(fix_summary(0, 0, 0, 0), "All files already correct");
        assert_eq!(
            fix_summary(0, 0, 0, 2),
            "Stripped frontmatter from 2 file(s)"
        );
    }

    // -- Frontmatter migration --------------------------------------------

    fn write_with_frontmatter(dir: &Path, id: &str, slug: &str) -> PathBuf {
        let filename = format!("{id}-p2-ready--{slug}.md");
        let content = format!(
            "---\ncreated: 2026-01-01\npriority: p2\nstatus: ready\nartifact: x\n---\n\n# {slug}\n\nbody\n"
        );
        let path = dir.join(&filename);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn prompt_mode_fails_when_frontmatter_present() {
        let tmp = TempDir::new().unwrap();
        let prefix = prefix_for(tmp.path());
        write_with_frontmatter(tmp.path(), &format!("{prefix}001"), "alpha");

        let r = fix(tmp.path(), MigrateMode::Prompt);
        assert!(!r.ok());
        assert_eq!(r.frontmatter_pending.len(), 1);
        assert!(r.frontmatter_pending[0].contains("alpha"));
        assert!(r.errors[0].contains("--migrate"));
        assert!(r.errors[0].contains("--no-migrate"));
    }

    #[test]
    fn prompt_mode_passes_when_no_frontmatter() {
        let tmp = TempDir::new().unwrap();
        let prefix = prefix_for(tmp.path());
        write_task(tmp.path(), &format!("{prefix}001"), "p2", "ready", "alpha");

        let r = fix(tmp.path(), MigrateMode::Prompt);
        assert!(r.ok(), "{:?}", r.errors);
        assert_eq!(r.frontmatter_pending.len(), 0);
    }

    #[test]
    fn migrate_mode_strips_frontmatter() {
        let tmp = TempDir::new().unwrap();
        let prefix = prefix_for(tmp.path());
        let path = write_with_frontmatter(tmp.path(), &format!("{prefix}001"), "alpha");

        let r = fix(tmp.path(), MigrateMode::Migrate);
        assert!(r.ok(), "{:?}", r.errors);
        assert_eq!(r.frontmatter_stripped.len(), 1);

        let new_content = fs::read_to_string(&path).unwrap();
        assert!(!new_content.starts_with("---"));
        assert!(new_content.starts_with("# alpha"));
        assert!(new_content.contains("body"));
    }

    #[test]
    fn migrate_mode_idempotent() {
        let tmp = TempDir::new().unwrap();
        let prefix = prefix_for(tmp.path());
        write_with_frontmatter(tmp.path(), &format!("{prefix}001"), "alpha");

        fix(tmp.path(), MigrateMode::Migrate);
        let r2 = fix(tmp.path(), MigrateMode::Migrate);
        assert_eq!(r2.frontmatter_stripped.len(), 0);
        assert!(r2.ok());
    }

    #[test]
    fn skip_mode_leaves_frontmatter_alone() {
        let tmp = TempDir::new().unwrap();
        let prefix = prefix_for(tmp.path());
        let path = write_with_frontmatter(tmp.path(), &format!("{prefix}001"), "alpha");

        let r = fix(tmp.path(), MigrateMode::Skip);
        assert!(r.ok());
        assert_eq!(r.frontmatter_stripped.len(), 0);
        assert_eq!(r.frontmatter_pending.len(), 0);

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("---"));
    }

    #[test]
    fn strip_frontmatter_returns_none_for_no_frontmatter() {
        assert!(strip_frontmatter("# Hello\n").is_none());
        assert!(strip_frontmatter("").is_none());
        assert!(strip_frontmatter("---\nopen but no close").is_none());
    }

    #[test]
    fn strip_frontmatter_handles_well_formed_block() {
        let s = "---\nfoo: bar\n---\n\n# Title\n\nbody\n";
        let stripped = strip_frontmatter(s).unwrap();
        assert_eq!(stripped, "# Title\n\nbody\n");
    }
}
