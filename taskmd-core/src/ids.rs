use sha2::{Digest, Sha256};
use std::path::Path;

pub const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKLMNPQRSTUVWXYZ"; // no I or O
pub const BASE: usize = 34;

/// Derive a deterministic 2-character prefix from a tasks directory realpath.
///
/// Uses SHA-256 of the canonical path so different worktrees produce different
/// prefixes and task IDs never collide.
pub fn prefix_for(tasks_dir: &Path) -> String {
    let canonical = tasks_dir
        .canonicalize()
        .unwrap_or_else(|_| tasks_dir.to_path_buf());
    let path_str = canonical.to_string_lossy();

    let hash = Sha256::digest(path_str.as_bytes());
    // Match Python: int.from_bytes(h[:2], "big") % (BASE * BASE)
    let val = (((hash[0] as usize) << 8) | (hash[1] as usize)) % (BASE * BASE);

    let c0 = ALPHABET[val / BASE] as char;
    let c1 = ALPHABET[val % BASE] as char;
    format!("{c0}{c1}")
}

/// True if the task ID uses the legacy 4-digit numeric format (e.g. "0042").
pub fn is_legacy_id(task_id: &str) -> bool {
    task_id.len() == 4 && task_id.bytes().all(|b| b.is_ascii_digit())
}

/// Decompose a task ID into (prefix, sequence_number).
///
/// New format "AB042" → ("AB", 42). Legacy "0042" → ("", 42).
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
/// Scans existing files (both legacy NNNN and new AANNN), takes the highest
/// sequence number for this prefix (or any legacy file), and increments by 1.
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
            let (pfx, seq) = parse_id_parts(&id);
            if pfx == prefix || pfx.is_empty() {
                max_seq = max_seq.max(seq);
            }
        }
    }

    format!("{prefix}{:03}", max_seq + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alphabet_has_no_i_or_o() {
        assert!(!ALPHABET.contains(&b'I'));
        assert!(!ALPHABET.contains(&b'O'));
        assert_eq!(ALPHABET.len(), BASE);
    }

    #[test]
    fn is_legacy_id_works() {
        assert!(is_legacy_id("0042"));
        assert!(is_legacy_id("0000"));
        assert!(!is_legacy_id("AB042"));
        assert!(!is_legacy_id("042"));
        assert!(!is_legacy_id("00042"));
    }

    #[test]
    fn parse_id_parts_new() {
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
    fn prefix_chars_are_in_alphabet() {
        let tmp = tempfile::tempdir().unwrap();
        let prefix = prefix_for(tmp.path());
        for c in prefix.chars() {
            assert!(ALPHABET.contains(&(c as u8)), "char {c} not in alphabet");
        }
    }
}
