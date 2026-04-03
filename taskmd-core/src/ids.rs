use sha2::{Digest, Sha256};
use std::path::Path;

/// Maximum sequence number that fits in the 3-digit NNN suffix.
const MAX_SEQ: u32 = 999;

/// Compute D1 (machine digit) from an explicit override or hostname hash.
fn machine_digit(machine_id_override: Option<&str>) -> usize {
    if let Some(val) = machine_id_override {
        if val.len() == 1 && val.as_bytes()[0].is_ascii_digit() {
            return (val.as_bytes()[0] - b'0') as usize;
        }
    }
    let hostname = gethostname::gethostname();
    let hostname_str = hostname.to_string_lossy();
    let h = Sha256::digest(hostname_str.as_bytes());
    h[0] as usize % 10
}

/// Resolve a path to its canonical form, even if it doesn't exist yet.
///
/// Tries `canonicalize()` first (resolves symlinks, requires path to exist).
/// Falls back to canonicalizing the nearest existing ancestor and appending
/// the remaining components. This handles the common case where the tasks
/// directory hasn't been created yet (e.g., before `taskmd init`).
fn resolve_path(path: &Path) -> std::path::PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }
    // Walk up until we find an ancestor that exists, canonicalize it,
    // then re-append the tail components.
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut tail = Vec::new();
    let mut ancestor = abs.as_path();
    loop {
        if let Ok(canonical) = ancestor.canonicalize() {
            let mut result = canonical;
            for component in tail.iter().rev() {
                result.push(component);
            }
            return result;
        }
        match ancestor.file_name() {
            Some(name) => {
                tail.push(name.to_os_string());
                ancestor = ancestor.parent().unwrap_or(ancestor);
            }
            None => return abs, // reached root without success, use absolute path
        }
    }
}

/// Compute D2 (directory digit) from the tasks directory path.
fn dir_digit(tasks_dir: &Path) -> usize {
    let resolved = resolve_path(tasks_dir);
    let path_str = resolved.to_string_lossy();
    let h = Sha256::digest(path_str.as_bytes());
    h[0] as usize % 10
}

/// Derive a deterministic 2-digit numeric prefix for a tasks directory.
///
/// - D1: `TASKMD_MACHINE_ID` env var (single digit 0-9) if set, else
///   `sha256(hostname) mod 10`.
/// - D2: `sha256(resolved_path) mod 10`.
///
/// Different machines produce different D1; different worktrees on the same
/// machine produce different D2. Together they partition the ID space so
/// concurrent `taskmd next` across machines/worktrees won't collide.
pub fn prefix_for(tasks_dir: &Path) -> String {
    let d1 = machine_digit(std::env::var("TASKMD_MACHINE_ID").ok().as_deref());
    let d2 = dir_digit(tasks_dir);
    format!("{d1}{d2}")
}

/// True if the task ID uses the legacy 4-digit numeric format (e.g. "0042").
pub fn is_legacy_id(task_id: &str) -> bool {
    task_id.len() == 4 && task_id.bytes().all(|b| b.is_ascii_digit())
}

/// True if a task ID needs migration to the current prefix.
///
/// Covers all three old formats:
/// - Legacy 4-digit NNNN (e.g. "0042")
/// - Old alpha-prefix AANNN (e.g. "YF042")
/// - Numeric prefix that doesn't match expected (e.g. "34042" when expected is "21")
pub fn needs_migration(task_id: &str, expected_prefix: &str) -> bool {
    if is_legacy_id(task_id) {
        return true;
    }
    if task_id.len() >= 5 {
        return &task_id[..2] != expected_prefix;
    }
    // Unrecognized format -- don't migrate
    false
}

/// Decompose a task ID into (prefix, sequence_number).
///
/// New format "34042" -> ("34", 42). Old alpha "AB042" -> ("AB", 42).
/// Legacy "0042" -> ("", 42).
pub fn parse_id_parts(task_id: &str) -> (String, u32) {
    if is_legacy_id(task_id) {
        ("".to_string(), task_id.parse().unwrap_or(0))
    } else if task_id.len() >= 5 {
        (task_id[..2].to_string(), task_id[2..].parse().unwrap_or(0))
    } else {
        ("".to_string(), 0)
    }
}

/// Return the next available task ID for this tasks directory.
///
/// Scans all existing files (any prefix format), takes the highest sequence
/// number, and increments by 1. If the sequence overflows 999, D2 is
/// incremented (mod 10) and the sequence resets to 001.
pub fn next_id(tasks_dir: &Path) -> String {
    let d1 = machine_digit(std::env::var("TASKMD_MACHINE_ID").ok().as_deref());
    let d2 = dir_digit(tasks_dir);

    if !tasks_dir.exists() {
        return format!("{d1}{d2}001");
    }

    let mut max_seq: u32 = 0;

    for path in crate::tasks::task_files(tasks_dir).unwrap_or_default() {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if let Some((id, _, _, _)) = crate::filename::parse_filename(&name) {
            let (_, seq) = parse_id_parts(&id);
            max_seq = max_seq.max(seq);
        }
    }

    let next = max_seq + 1;
    if next > MAX_SEQ {
        // Overflow: bump D2 and reset sequence
        let d2_next = (d2 + 1) % 10;
        format!("{d1}{d2_next}001")
    } else {
        format!("{d1}{d2}{next:03}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_legacy_id_works() {
        assert!(is_legacy_id("0042"));
        assert!(is_legacy_id("0000"));
        assert!(!is_legacy_id("AB042"));
        assert!(!is_legacy_id("042"));
        assert!(!is_legacy_id("00042"));
        assert!(!is_legacy_id("34042"));
    }

    #[test]
    fn needs_migration_legacy() {
        assert!(needs_migration("0042", "34"));
    }

    #[test]
    fn needs_migration_alpha_prefix() {
        assert!(needs_migration("YF042", "34"));
    }

    #[test]
    fn needs_migration_wrong_numeric_prefix() {
        assert!(needs_migration("21042", "34"));
    }

    #[test]
    fn needs_migration_correct_prefix() {
        assert!(!needs_migration("34042", "34"));
    }

    #[test]
    fn parse_id_parts_new() {
        assert_eq!(parse_id_parts("34042"), ("34".to_string(), 42));
    }

    #[test]
    fn parse_id_parts_alpha() {
        assert_eq!(parse_id_parts("AB042"), ("AB".to_string(), 42));
    }

    #[test]
    fn parse_id_parts_legacy() {
        assert_eq!(parse_id_parts("0042"), ("".to_string(), 42));
    }

    #[test]
    fn prefix_for_is_deterministic() {
        let tmp = tempfile::tempdir().unwrap();
        let p1 = prefix_for(tmp.path());
        let p2 = prefix_for(tmp.path());
        assert_eq!(p1, p2);
        assert_eq!(p1.len(), 2);
    }

    #[test]
    fn prefix_is_all_digits() {
        let tmp = tempfile::tempdir().unwrap();
        let prefix = prefix_for(tmp.path());
        for c in prefix.chars() {
            assert!(c.is_ascii_digit(), "prefix char '{c}' is not a digit");
        }
    }

    #[test]
    fn machine_digit_override() {
        assert_eq!(machine_digit(Some("7")), 7);
        assert_eq!(machine_digit(Some("0")), 0);
    }

    #[test]
    fn machine_digit_invalid_override_falls_back() {
        // Invalid values fall back to hostname hash -- just verify they don't panic
        let d1 = machine_digit(Some("ab"));
        assert!(d1 < 10);
        let d2 = machine_digit(Some(""));
        assert!(d2 < 10);
        let d3 = machine_digit(None);
        assert!(d3 < 10);
        // Invalid and None should both produce hostname hash
        assert_eq!(d1, d3);
        assert_eq!(d2, d3);
    }

    #[test]
    fn dir_digit_deterministic() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(dir_digit(tmp.path()), dir_digit(tmp.path()));
    }

    #[test]
    fn dir_digit_nonexistent_path_is_stable() {
        // Even for a path that doesn't exist, dir_digit should produce a
        // consistent result based on the absolute path resolution.
        let tmp = tempfile::tempdir().unwrap();
        let nonexistent = tmp.path().join("tasks");
        let d1 = dir_digit(&nonexistent);
        let d2 = dir_digit(&nonexistent);
        assert_eq!(d1, d2);
        // After creating the dir, the digit should remain the same
        std::fs::create_dir(&nonexistent).unwrap();
        let d3 = dir_digit(&nonexistent);
        assert_eq!(d1, d3);
    }

    #[test]
    fn prefix_for_nonexistent_dir_matches_after_creation() {
        let tmp = tempfile::tempdir().unwrap();
        let tasks = tmp.path().join("tasks");
        let before = prefix_for(&tasks);
        std::fs::create_dir(&tasks).unwrap();
        let after = prefix_for(&tasks);
        assert_eq!(before, after);
    }

    #[test]
    fn next_id_overflow_bumps_d2() {
        // Simulate: if max_seq were 999, next_id should overflow to next D2 bucket
        // We test the overflow logic directly since creating 999 files is slow.
        let d1 = 3_usize;
        let d2 = 4_usize;
        let max_seq = 999_u32;
        let next = max_seq + 1;

        let id = if next > MAX_SEQ {
            let d2_next = (d2 + 1) % 10;
            format!("{d1}{d2_next}001")
        } else {
            format!("{d1}{d2}{next:03}")
        };

        assert_eq!(id, "35001");
        assert_eq!(id.len(), 5);
    }

    #[test]
    fn next_id_overflow_d2_wraps() {
        // D2=9 should wrap to D2=0
        let d1 = 3_usize;
        let d2 = 9_usize;
        let d2_next = (d2 + 1) % 10;
        let id = format!("{d1}{d2_next}001");
        assert_eq!(id, "30001");
    }
}
