//! Property-based tests for taskmd-core.
//!
//! Mirrors the Python Hypothesis test suite (tests/test_properties.py).
//! Each test is tagged with the Python property number it corresponds to.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use proptest::prelude::*;
use tempfile::TempDir;

use taskmd_core::constants::{VALID_PRIORITIES, VALID_STATUSES};
use taskmd_core::filename::{derive_slug, format_filename, parse_filename, MAX_SLUG_LEN};
use taskmd_core::fix::fix;
use taskmd_core::ids::next_id;
use taskmd_core::tasks::{parse_task_file, task_files};
use taskmd_core::validate::validate;

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

fn arb_priority() -> impl Strategy<Value = String> {
    prop::sample::select(VALID_PRIORITIES).prop_map(|s| s.to_string())
}

fn arb_status() -> impl Strategy<Value = String> {
    prop::sample::select(VALID_STATUSES).prop_map(|s| s.to_string())
}

/// Kebab-case slug: 1-5 words of [a-z][a-z0-9]{0,8} joined by hyphens.
fn arb_slug() -> impl Strategy<Value = String> {
    prop::collection::vec("[a-z][a-z0-9]{0,8}", 1..=5)
        .prop_map(|parts| parts.join("-"))
        .prop_filter("slug must not contain --", |s| !s.contains("--"))
}

/// 5-digit numeric task ID: 2-digit prefix + 3-digit sequence (001-990).
fn arb_task_id() -> impl Strategy<Value = String> {
    (10..100u32, 1..=990u32).prop_map(|(pfx, seq)| format!("{pfx:02}{seq:03}"))
}

/// Full valid task params tuple.
fn arb_task_params() -> impl Strategy<Value = (String, String, String, String)> {
    (arb_task_id(), arb_priority(), arb_status(), arb_slug())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn write_task(dir: &Path, id: &str, priority: &str, status: &str, slug: &str) {
    let filename = format_filename(id, priority, status, slug);
    let content = format!(
        "---\ncreated: 2026-01-01\npriority: {priority}\nstatus: {status}\nartifact: src/{slug}.py\n---\n\n# Task {id}\n"
    );
    fs::write(dir.join(&filename), content).unwrap();
}

/// Create a temp dir with N unique valid task files. Returns (dir, ids used).
fn make_task_dir(
    params: &[(String, String, String, String)],
) -> (TempDir, Vec<String>) {
    let tmp = TempDir::new().unwrap();
    let mut ids = vec![];
    for (id, pri, sta, slug) in params {
        write_task(tmp.path(), id, pri, sta, slug);
        ids.push(id.clone());
    }
    (tmp, ids)
}

// ---------------------------------------------------------------------------
// P1: Filename roundtrip
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn filename_roundtrip((id, pri, sta, slug) in arb_task_params()) {
        let filename = format_filename(&id, &pri, &sta, &slug);
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(&filename);
        let content = format!(
            "---\ncreated: 2026-01-01\npriority: {pri}\nstatus: {sta}\n---\n"
        );
        fs::write(&path, content).unwrap();
        let task = parse_task_file(&path).unwrap();
        prop_assert_eq!(&task.id, &id);
        prop_assert_eq!(&task.priority, &pri);
        prop_assert_eq!(&task.status, &sta);
        prop_assert_eq!(&task.slug, &slug);
    }
}

// ---------------------------------------------------------------------------
// P2: Parse-regenerate roundtrip
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn parse_regenerate_roundtrip((id, pri, sta, slug) in arb_task_params()) {
        let original = format_filename(&id, &pri, &sta, &slug);
        let (parsed_id, parsed_pri, parsed_sta, parsed_slug) =
            parse_filename(&original).unwrap();
        let regenerated =
            format_filename(&parsed_id, &parsed_pri, &parsed_sta, &parsed_slug);
        prop_assert_eq!(original, regenerated);
    }
}

// ---------------------------------------------------------------------------
// P3: Slug preservation (multi-hyphen)
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn slug_preservation(
        id in arb_task_id(),
        pri in arb_priority(),
        sta in arb_status(),
        slug in prop::collection::vec("[a-z][a-z0-9]{0,8}", 2..=5)
            .prop_map(|parts| parts.join("-"))
            .prop_filter("no double dash", |s| !s.contains("--"))
    ) {
        let tmp = TempDir::new().unwrap();
        let filename = format_filename(&id, &pri, &sta, &slug);
        let path = tmp.path().join(&filename);
        let content = format!(
            "---\ncreated: 2026-01-01\npriority: {pri}\nstatus: {sta}\n---\n"
        );
        fs::write(&path, content).unwrap();
        let task = parse_task_file(&path).unwrap();
        prop_assert_eq!(task.slug, slug);
    }
}

// ---------------------------------------------------------------------------
// P4: Parsed ID format (DDNNN - 5 digits)
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn parsed_id_is_five_digits((id, pri, sta, slug) in arb_task_params()) {
        let filename = format_filename(&id, &pri, &sta, &slug);
        let (parsed_id, _, _, _) = parse_filename(&filename).unwrap();
        prop_assert_eq!(parsed_id.len(), 5);
        prop_assert!(parsed_id.chars().all(|c| c.is_ascii_digit()));
    }
}

// ---------------------------------------------------------------------------
// P7: Filename starts with 5-digit ID
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn filename_starts_with_five_digit_id((id, pri, sta, slug) in arb_task_params()) {
        let filename = format_filename(&id, &pri, &sta, &slug);
        let first_five: String = filename.chars().take(5).collect();
        prop_assert!(first_five.chars().all(|c| c.is_ascii_digit()));
        prop_assert_eq!(&filename.chars().nth(5).unwrap(), &'-');
    }
}

// ---------------------------------------------------------------------------
// P8: Exactly one double-dash separator
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn filename_has_exactly_one_double_dash((id, pri, sta, slug) in arb_task_params()) {
        let filename = format_filename(&id, &pri, &sta, &slug);
        prop_assert_eq!(filename.matches("--").count(), 1);
    }
}

// ---------------------------------------------------------------------------
// P9: Non-conforming filenames return None
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn non_conforming_filenames_return_none(name in "[a-zA-Z0-9 _.-]{1,60}") {
        // Add .md if missing
        let name = if name.ends_with(".md") { name } else { format!("{name}.md") };
        if parse_filename(&name).is_some() {
            // Hypothesis generated a valid filename -- skip
            return Ok(());
        }
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(&name);
        fs::write(&path, "---\nstatus: ready\n---\n").unwrap();
        prop_assert!(parse_task_file(&path).is_none());
    }
}

// ---------------------------------------------------------------------------
// P10: Fix idempotency
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    #[test]
    fn fix_idempotency(
        params in prop::collection::vec(arb_task_params(), 3)
            .prop_filter("unique ids", |v| {
                let ids: HashSet<_> = v.iter().map(|(id, _, _, _)| id.clone()).collect();
                ids.len() == v.len()
            })
    ) {
        let (tmp, _) = make_task_dir(&params);
        fix(tmp.path());
        let result2 = fix(tmp.path());
        prop_assert_eq!(result2.patched, 0);
        prop_assert_eq!(result2.renamed, 0);
        prop_assert!(result2.ok());
    }
}

// ---------------------------------------------------------------------------
// P11: Fix implies validate
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    #[test]
    fn fix_implies_validate(
        params in prop::collection::vec(arb_task_params(), 3)
            .prop_filter("unique ids", |v| {
                let ids: HashSet<_> = v.iter().map(|(id, _, _, _)| id.clone()).collect();
                ids.len() == v.len()
            })
    ) {
        let (tmp, _) = make_task_dir(&params);
        let fix_result = fix(tmp.path());
        if fix_result.ok() {
            let val_result = validate(tmp.path());
            prop_assert!(
                val_result.ok(),
                "validate failed after fix: {:?}", val_result.errors
            );
        }
    }
}

// ---------------------------------------------------------------------------
// P12: Fix preserves file count
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    #[test]
    fn fix_preserves_file_count(
        params in prop::collection::vec(arb_task_params(), 3)
            .prop_filter("unique ids", |v| {
                let ids: HashSet<_> = v.iter().map(|(id, _, _, _)| id.clone()).collect();
                ids.len() == v.len()
            })
    ) {
        let (tmp, _) = make_task_dir(&params);
        let before = task_files(tmp.path()).unwrap().len();
        fix(tmp.path());
        let after = task_files(tmp.path()).unwrap().len();
        prop_assert_eq!(before, after);
    }
}

// ---------------------------------------------------------------------------
// P15: next_id format (always 5 digits)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    #[test]
    fn next_id_is_five_digits(
        params in prop::collection::vec(arb_task_params(), 0..=3)
            .prop_filter("unique ids", |v| {
                let ids: HashSet<_> = v.iter().map(|(id, _, _, _)| id.clone()).collect();
                ids.len() == v.len()
            })
    ) {
        let (tmp, _) = make_task_dir(&params);
        let id = next_id(tmp.path());
        prop_assert_eq!(id.len(), 5, "next_id returned {:?}, expected 5 chars", id);
        prop_assert!(id.chars().all(|c| c.is_ascii_digit()));
    }
}

// ---------------------------------------------------------------------------
// P17: Template and ancillary files are transparent
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn template_and_ancillary_transparent((id, pri, sta, slug) in arb_task_params()) {
        let tmp = TempDir::new().unwrap();
        write_task(tmp.path(), &id, &pri, &sta, &slug);

        // Add template
        fs::write(
            tmp.path().join("_TEMPLATE.md"),
            "---\ncreated: YYYY\npriority: p2\nstatus: ready\n---\n",
        ).unwrap();

        // Add ancillary file
        let task_stem = format_filename(&id, &pri, &sta, &slug);
        let ancillary_name = task_stem.replace(".md", ".qaplan.md");
        fs::write(
            tmp.path().join(&ancillary_name),
            "ancillary content\n",
        ).unwrap();

        let val = validate(tmp.path());
        prop_assert!(val.ok(), "unexpected errors: {:?}", val.errors);
        prop_assert_eq!(val.file_count, 1);

        let fix_result = fix(tmp.path());
        prop_assert!(fix_result.ok());
    }
}

// ---------------------------------------------------------------------------
// P20: Duplicate IDs always detected
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn duplicate_ids_detected(
        id in arb_task_id(),
        pri1 in arb_priority(),
        sta1 in arb_status(),
        slug1 in arb_slug(),
        pri2 in arb_priority(),
        sta2 in arb_status(),
        slug2 in arb_slug(),
    ) {
        let f1 = format_filename(&id, &pri1, &sta1, &slug1);
        let f2 = format_filename(&id, &pri2, &sta2, &slug2);
        prop_assume!(f1 != f2);

        let tmp = TempDir::new().unwrap();
        write_task(tmp.path(), &id, &pri1, &sta1, &slug1);
        write_task(tmp.path(), &id, &pri2, &sta2, &slug2);

        let result = validate(tmp.path());
        prop_assert!(!result.ok());
        prop_assert!(
            result.errors.iter().any(|e| e.contains("duplicate task id")),
            "expected duplicate ID error, got: {:?}", result.errors
        );
    }
}

// ---------------------------------------------------------------------------
// P22: Validate file count matches actual
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    #[test]
    fn validate_file_count_matches_actual(
        params in prop::collection::vec(arb_task_params(), 1..=4)
            .prop_filter("unique ids", |v| {
                let ids: HashSet<_> = v.iter().map(|(id, _, _, _)| id.clone()).collect();
                ids.len() == v.len()
            })
    ) {
        let (tmp, _) = make_task_dir(&params);
        let actual = task_files(tmp.path()).unwrap().len();
        let result = validate(tmp.path());
        prop_assert_eq!(result.file_count, actual);
    }
}

// ---------------------------------------------------------------------------
// P23: Validate errors reference originating filename
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn validate_errors_reference_filename(
        (id, pri, sta, slug) in arb_task_params()
    ) {
        // Create file with mismatched status in frontmatter
        let other_status = VALID_STATUSES.iter()
            .find(|&&s| s != sta)
            .unwrap();
        let tmp = TempDir::new().unwrap();
        let filename = format_filename(&id, &pri, &sta, &slug);
        let content = format!(
            "---\ncreated: 2026-01-01\npriority: {pri}\nstatus: {other_status}\nartifact: src/x.py\n---\n"
        );
        fs::write(tmp.path().join(&filename), content).unwrap();

        let result = validate(tmp.path());
        let mismatch_errors: Vec<_> = result.errors.iter()
            .filter(|e| e.contains("doesn't match frontmatter"))
            .collect();
        prop_assert!(!mismatch_errors.is_empty());
        for err in &mismatch_errors {
            prop_assert!(
                err.contains(&filename),
                "error {err:?} doesn't reference filename {filename:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// P25: One error per filename/frontmatter mismatch
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn one_error_per_mismatch(
        (id, pri, sta, slug) in arb_task_params()
    ) {
        let other_status = VALID_STATUSES.iter()
            .find(|&&s| s != sta)
            .unwrap();
        let tmp = TempDir::new().unwrap();
        let filename = format_filename(&id, &pri, &sta, &slug);
        let content = format!(
            "---\ncreated: 2026-01-01\npriority: {pri}\nstatus: {other_status}\nartifact: src/x.py\n---\n"
        );
        fs::write(tmp.path().join(&filename), content).unwrap();

        let result = validate(tmp.path());
        let mismatch_count = result.errors.iter()
            .filter(|e| e.contains("doesn't match frontmatter") && e.contains(&filename))
            .count();
        prop_assert_eq!(mismatch_count, 1);
    }
}

// ---------------------------------------------------------------------------
// derive_slug properties
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn derive_slug_never_exceeds_max_len(title in ".{0,100}") {
        let slug = derive_slug(&title);
        prop_assert!(slug.len() <= MAX_SLUG_LEN);
    }

    #[test]
    fn derive_slug_no_trailing_hyphen(title in ".{1,80}") {
        let slug = derive_slug(&title);
        if !slug.is_empty() {
            prop_assert!(!slug.ends_with('-'));
        }
    }

    #[test]
    fn derive_slug_no_leading_hyphen(title in ".{1,80}") {
        let slug = derive_slug(&title);
        if !slug.is_empty() {
            prop_assert!(!slug.starts_with('-'));
        }
    }

    #[test]
    fn derive_slug_only_valid_chars(title in ".{0,80}") {
        let slug = derive_slug(&title);
        prop_assert!(slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    }
}
