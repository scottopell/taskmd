use sha2::{Digest, Sha256};
use std::path::Path;

/// Maximum sequence number that fits in the 3-digit NNN suffix.
const MAX_SEQ: u32 = 999;

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

/// Hash machine identity and tasks directory path together into a value 0-99.
///
/// Machine identity is `TASKMD_MACHINE_ID` if set, else the hostname. Path is
/// the canonical (symlink-resolved) tasks directory. The two inputs are
/// separated by a NUL byte so distinct (machine, path) pairs can't collide
/// through concatenation ambiguity.
fn prefix_digits(machine_id_override: Option<&str>, tasks_dir: &Path) -> usize {
    let mut hasher = Sha256::new();
    match machine_id_override {
        Some(val) => hasher.update(val.as_bytes()),
        None => {
            let hostname = gethostname::gethostname();
            hasher.update(hostname.to_string_lossy().as_bytes());
        }
    }
    hasher.update(b"\0");
    let resolved = resolve_path(tasks_dir);
    hasher.update(resolved.to_string_lossy().as_bytes());
    let h = hasher.finalize();
    let v = ((h[0] as u16) << 8 | h[1] as u16) as usize;
    v % 100
}

/// Derive a deterministic 2-digit numeric prefix for a tasks directory.
///
/// Hashes `(machine_identity, canonical_path)` together and takes the result
/// modulo 100. Machine identity is `TASKMD_MACHINE_ID` if set, otherwise the
/// hostname. Different worktrees on the same machine and different machines
/// both produce different prefixes, partitioning the ID space so concurrent
/// `taskmd next` calls rarely collide. With 100 buckets, the birthday-50%
/// point is ~12 concurrent worktrees.
pub fn prefix_for(tasks_dir: &Path) -> String {
    let v = prefix_digits(
        std::env::var("TASKMD_MACHINE_ID").ok().as_deref(),
        tasks_dir,
    );
    format!("{v:02}")
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
        if let Some(parsed) = crate::filename::parse_filename(&name) {
            let (pfx, seq) = parse_id_parts(&parsed.id);
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
/// computing the next sequence number. If the local prefix is full
/// (seq > 999), the prefix is bumped by 1 (mod 100) and successive
/// prefix buckets are scanned until one with a free sequence is found.
///
/// # Panics
///
/// If every one of the 100 prefix buckets is exhausted (i.e. the tasks
/// directory holds 99 900 tasks). This is a six-figure scale event and
/// has not been observed in practice.
pub fn next_id(tasks_dir: &Path) -> String {
    let prefix = prefix_for(tasks_dir);

    if !tasks_dir.exists() {
        return format!("{prefix}001");
    }

    let local_seqs = used_sequences(tasks_dir, &prefix);
    let max_seq = local_seqs.iter().copied().max().unwrap_or(0);
    let next = max_seq + 1;
    if next <= MAX_SEQ {
        return format!("{prefix}{next:03}");
    }

    let prefix_num: u32 = prefix.parse().expect("prefix_for returns 2-digit numeric");
    for offset in 1..100 {
        let candidate_prefix = format!("{:02}", (prefix_num + offset) % 100);
        let seqs = used_sequences(tasks_dir, &candidate_prefix);
        for seq in 1..=MAX_SEQ {
            if !seqs.contains(&seq) {
                return format!("{candidate_prefix}{seq:03}");
            }
        }
    }
    panic!("all 100 prefix buckets exhausted (each holds 999 tasks); tasks directory holds ~99 900 entries");
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
    fn prefix_digits_override_changes_result() {
        let tmp = tempfile::tempdir().unwrap();
        let base = prefix_digits(None, tmp.path());
        // Different machine identities should usually produce different prefixes.
        // We test that the override actually feeds the hash: at least one of
        // several candidates differs from the hostname-derived prefix.
        let mut any_different = false;
        for candidate in ["alpha", "beta", "gamma", "delta", "epsilon"] {
            if prefix_digits(Some(candidate), tmp.path()) != base {
                any_different = true;
                break;
            }
        }
        assert!(
            any_different,
            "machine_id_override should influence the hash"
        );
    }

    #[test]
    fn prefix_digits_changes_with_path() {
        // The original collision bug: identical machine identity + identical
        // path produced identical prefixes. Verify that varying *only* the path
        // (machine identity fixed) produces a different prefix for at least
        // one candidate pair. Try several siblings to absorb the 1-in-100
        // chance any individual pair happens to collide.
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("base");
        std::fs::create_dir(&base).unwrap();
        let base_prefix = prefix_digits(Some("m"), &base);

        let mut any_different = false;
        for name in ["a", "b", "c", "d", "e", "f", "g", "h"] {
            let other = tmp.path().join(name);
            std::fs::create_dir(&other).unwrap();
            if prefix_digits(Some("m"), &other) != base_prefix {
                any_different = true;
                break;
            }
        }
        assert!(
            any_different,
            "path input should influence the hash — varying path with fixed \
             machine identity should produce different prefixes"
        );
    }

    #[test]
    fn prefix_digits_same_path_different_machines_differ() {
        // Cross-machine case: identical path, different machine identities
        // should produce different prefixes. This is the multi-host scenario
        // the prefix is meant to disambiguate.
        let tmp = tempfile::tempdir().unwrap();
        let base = prefix_digits(Some("host-a"), tmp.path());
        let mut any_different = false;
        for candidate in ["host-b", "host-c", "host-d", "host-e", "host-f"] {
            if prefix_digits(Some(candidate), tmp.path()) != base {
                any_different = true;
                break;
            }
        }
        assert!(
            any_different,
            "machine identity should influence the hash — varying machine \
             with fixed path should produce different prefixes"
        );
    }

    #[test]
    fn prefix_digits_in_range() {
        let tmp = tempfile::tempdir().unwrap();
        let v = prefix_digits(None, tmp.path());
        assert!(v < 100);
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
    fn next_id_overflow_bumps_prefix() {
        // When local bucket fills (seq > 999), prefix increments by 1 (mod 100).
        let prefix_num: u32 = 34;
        let overflow = format!("{:02}", (prefix_num + 1) % 100);
        assert_eq!(overflow, "35");
    }

    #[test]
    fn next_id_overflow_wraps_at_99() {
        let prefix_num: u32 = 99;
        let overflow = format!("{:02}", (prefix_num + 1) % 100);
        assert_eq!(overflow, "00");
    }

    /// Helper: write a minimal task file with a given ID into a directory.
    fn write_task_file(dir: &Path, id: &str) {
        let filename = format!("{id}-p2-ready--test.md");
        std::fs::write(dir.join(filename), "# task body\n").unwrap();
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
        let prefix_num: u32 = local_prefix.parse().unwrap();
        let overflow_prefix = format!("{:02}", (prefix_num + 1) % 100);

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
