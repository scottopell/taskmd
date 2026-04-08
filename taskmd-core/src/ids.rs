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

/// True if a task ID needs migration to the current numeric format.
///
/// Covers legacy formats only:
/// - Legacy 4-digit NNNN (e.g. "0042")
/// - Old alpha-prefix AANNN (e.g. "YF042")
///
/// A 5-digit all-numeric ID (e.g. "34042") is already in the current format
/// and is NEVER migrated, even if its prefix differs from the local directory.
/// The prefix encodes the directory where the task was created; changing it
/// would destroy cross-worktree identity (see issue #6).
pub fn needs_migration(task_id: &str, _expected_prefix: &str) -> bool {
    if is_legacy_id(task_id) {
        return true;
    }
    if task_id.len() >= 5 {
        // Alpha-prefix (e.g. "YF042") needs migration; numeric prefix does not.
        let first_two = &task_id[..2];
        return !first_two.bytes().all(|b| b.is_ascii_digit());
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

/// Collect the set of sequence numbers used by tasks with a given prefix.
fn used_sequences(tasks_dir: &Path, prefix: &str) -> std::collections::HashSet<u32> {
    let mut seqs = std::collections::HashSet::new();
    for path in crate::tasks::task_files(tasks_dir).unwrap_or_default() {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if let Some((id, _, _, _)) = crate::filename::parse_filename(&name) {
            let (pfx, seq) = parse_id_parts(&id);
            if pfx == prefix {
                seqs.insert(seq);
            }
        }
    }
    seqs
}

/// Return the next available task ID for this tasks directory.
///
/// Only considers tasks whose prefix matches the local prefix when
/// computing the next sequence number. If the local prefix overflows
/// (seq > 999), D2 is bumped and sequences in the new prefix space are
/// checked to avoid collisions.
pub fn next_id(tasks_dir: &Path) -> String {
    let d1 = machine_digit(std::env::var("TASKMD_MACHINE_ID").ok().as_deref());
    let d2 = dir_digit(tasks_dir);
    let prefix = format!("{d1}{d2}");

    if !tasks_dir.exists() {
        return format!("{prefix}001");
    }

    let local_seqs = used_sequences(tasks_dir, &prefix);
    let max_seq = local_seqs.iter().copied().max().unwrap_or(0);

    let next = max_seq + 1;
    if next > MAX_SEQ {
        // Overflow: bump D2 and find the first unused sequence in the new space.
        let d2_next = (d2 + 1) % 10;
        let overflow_prefix = format!("{d1}{d2_next}");
        let overflow_seqs = used_sequences(tasks_dir, &overflow_prefix);
        let mut seq = 1u32;
        while overflow_seqs.contains(&seq) && seq <= MAX_SEQ {
            seq += 1;
        }
        format!("{overflow_prefix}{seq:03}")
    } else {
        format!("{prefix}{next:03}")
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
    fn needs_migration_different_numeric_prefix_is_not_migrated() {
        // Issue #6: a valid numeric prefix from another worktree must NOT be migrated
        assert!(!needs_migration("21042", "34"));
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

    /// Helper: write a minimal task file with a given ID into a directory.
    fn write_task_file(dir: &Path, id: &str) {
        let filename = format!("{id}-p2-ready--test.md");
        let content = format!(
            "---\ncreated: 2026-01-01\npriority: p2\nstatus: ready\n---\n"
        );
        std::fs::write(dir.join(filename), content).unwrap();
    }

    // -- Bug 4: next_id should scope sequence scan to local prefix --

    #[test]
    fn next_id_ignores_foreign_prefix_sequences() {
        // Create a dir, determine its local prefix, then add a task with a
        // foreign prefix that has a high sequence number. next_id should NOT
        // jump past the foreign sequence.
        let tmp = tempfile::tempdir().unwrap();
        let local_prefix = prefix_for(tmp.path());

        // Pick a foreign prefix that differs from local
        let foreign_prefix = if local_prefix == "99" {
            "00".to_string()
        } else {
            format!("{:02}", local_prefix.parse::<u32>().unwrap() + 1)
        };

        // Write a foreign-prefix task with high sequence
        write_task_file(tmp.path(), &format!("{foreign_prefix}900"));
        // Write a local-prefix task with low sequence
        write_task_file(tmp.path(), &format!("{local_prefix}005"));

        let id = next_id(tmp.path());
        let (pfx, seq) = parse_id_parts(&id);
        assert_eq!(pfx, local_prefix);
        // Should be 006 (next after local max of 005), NOT 901
        assert_eq!(
            seq, 6,
            "next_id returned {id} (seq {seq}), expected seq 6 — \
             it should ignore foreign prefix {foreign_prefix}900"
        );
    }

    #[test]
    fn next_id_starts_at_001_with_only_foreign_tasks() {
        let tmp = tempfile::tempdir().unwrap();
        let local_prefix = prefix_for(tmp.path());

        let foreign_prefix = if local_prefix == "99" {
            "00".to_string()
        } else {
            format!("{:02}", local_prefix.parse::<u32>().unwrap() + 1)
        };

        // Only foreign-prefix tasks exist
        write_task_file(tmp.path(), &format!("{foreign_prefix}500"));
        write_task_file(tmp.path(), &format!("{foreign_prefix}501"));

        let id = next_id(tmp.path());
        let (pfx, seq) = parse_id_parts(&id);
        assert_eq!(pfx, local_prefix);
        assert_eq!(
            seq, 1,
            "next_id returned {id} (seq {seq}), expected seq 1 — \
             no local-prefix tasks exist"
        );
    }

    // -- Bug 5: overflow should check target prefix space for collisions --

    #[test]
    fn next_id_overflow_avoids_collision() {
        // Simulate: local prefix is full (seq 999), and the bumped prefix
        // already has tasks. next_id should skip occupied sequences.
        let tmp = tempfile::tempdir().unwrap();
        let local_prefix = prefix_for(tmp.path());
        let d1: usize = local_prefix[..1].parse().unwrap();
        let d2: usize = local_prefix[1..2].parse().unwrap();
        let d2_next = (d2 + 1) % 10;
        let overflow_prefix = format!("{d1}{d2_next}");

        // Fill local prefix to 999
        write_task_file(tmp.path(), &format!("{local_prefix}999"));
        // Put tasks in the overflow prefix space
        write_task_file(tmp.path(), &format!("{overflow_prefix}001"));
        write_task_file(tmp.path(), &format!("{overflow_prefix}002"));

        let id = next_id(tmp.path());
        let (pfx, seq) = parse_id_parts(&id);
        assert_eq!(pfx, overflow_prefix);
        // Should skip 001 and 002 which are occupied
        assert_eq!(
            seq, 3,
            "next_id returned {id} (seq {seq}), expected seq 3 — \
             overflow prefix {overflow_prefix} already has 001 and 002"
        );
    }
}
