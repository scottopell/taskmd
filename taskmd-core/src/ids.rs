use sha2::{Digest, Sha256};
use std::path::Path;

/// Derive a deterministic 2-digit numeric prefix for a tasks directory.
///
/// - D1: `TASKMD_MACHINE_ID` env var (single digit 0-9) if set, else
///   `sha256(hostname) mod 10`.
/// - D2: `sha256(canonical_path) mod 10`.
///
/// Different machines produce different D1; different worktrees on the same
/// machine produce different D2. Together they partition the ID space so
/// concurrent `taskmd next` across machines/worktrees won't collide.
pub fn prefix_for(tasks_dir: &Path) -> String {
    let d1 = match std::env::var("TASKMD_MACHINE_ID") {
        Ok(val) if val.len() == 1 && val.as_bytes()[0].is_ascii_digit() => {
            (val.as_bytes()[0] - b'0') as usize
        }
        _ => {
            let hostname = gethostname::gethostname();
            let hostname_str = hostname.to_string_lossy();
            let h = Sha256::digest(hostname_str.as_bytes());
            h[0] as usize % 10
        }
    };

    let canonical = tasks_dir
        .canonicalize()
        .unwrap_or_else(|_| tasks_dir.to_path_buf());
    let path_str = canonical.to_string_lossy();
    let h = Sha256::digest(path_str.as_bytes());
    let d2 = h[0] as usize % 10;

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
/// number, and increments by 1. After migration, all files share the same
/// prefix, so counting all sequences avoids gaps and collisions.
pub fn next_id(tasks_dir: &Path) -> String {
    let prefix = prefix_for(tasks_dir);

    if !tasks_dir.exists() {
        return format!("{prefix}001");
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

    format!("{prefix}{:03}", max_seq + 1)
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
    fn prefix_machine_id_env_var() {
        let tmp = tempfile::tempdir().unwrap();

        // Set TASKMD_MACHINE_ID=7
        std::env::set_var("TASKMD_MACHINE_ID", "7");
        let prefix = prefix_for(tmp.path());
        assert!(prefix.starts_with('7'), "expected prefix to start with '7', got '{prefix}'");

        // Clean up
        std::env::remove_var("TASKMD_MACHINE_ID");
    }

    #[test]
    fn prefix_machine_id_env_var_invalid_ignored() {
        let tmp = tempfile::tempdir().unwrap();

        // Invalid values should fall back to hostname hash
        std::env::set_var("TASKMD_MACHINE_ID", "ab");
        let p1 = prefix_for(tmp.path());

        std::env::remove_var("TASKMD_MACHINE_ID");
        let p2 = prefix_for(tmp.path());

        // Both should be the same (env var ignored -> hostname hash)
        assert_eq!(p1, p2);
    }
}
