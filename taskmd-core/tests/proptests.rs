//! Property-based tests for taskmd-core.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use proptest::prelude::*;
use tempfile::TempDir;

use taskmd_core::constants::{VALID_PRIORITIES, VALID_STATUSES};
use taskmd_core::filename::{derive_slug, format_filename, parse_filename, MAX_SLUG_LEN};
use taskmd_core::fix::{fix, fix_summary, MigrateMode};
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

fn arb_slug() -> impl Strategy<Value = String> {
    prop::collection::vec("[a-z][a-z0-9]{0,8}", 1..=5)
        .prop_map(|parts| parts.join("-"))
        .prop_filter("slug must not contain --", |s| !s.contains("--"))
}

fn arb_task_id() -> impl Strategy<Value = String> {
    (10..100u32, 1..=990u32).prop_map(|(pfx, seq)| format!("{pfx:02}{seq:03}"))
}

fn arb_task_params() -> impl Strategy<Value = (String, String, String, String)> {
    (arb_task_id(), arb_priority(), arb_status(), arb_slug())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn write_task(dir: &Path, id: &str, priority: &str, status: &str, slug: &str) {
    let filename = format_filename(id, priority, status, slug);
    fs::write(dir.join(&filename), format!("# Task {id}\n")).unwrap();
}

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
// Filename round-trip / structural invariants
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn filename_roundtrip((id, pri, sta, slug) in arb_task_params()) {
        let filename = format_filename(&id, &pri, &sta, &slug);
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(&filename);
        fs::write(&path, "body\n").unwrap();
        let task = parse_task_file(&path).unwrap();
        prop_assert_eq!(&task.id, &id);
        prop_assert_eq!(&task.priority, &pri);
        prop_assert_eq!(&task.status, &sta);
        prop_assert_eq!(&task.slug, &slug);
    }
}

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
        fs::write(&path, "body\n").unwrap();
        let task = parse_task_file(&path).unwrap();
        prop_assert_eq!(task.slug, slug);
    }
}

proptest! {
    #[test]
    fn parsed_id_is_five_digits((id, pri, sta, slug) in arb_task_params()) {
        let filename = format_filename(&id, &pri, &sta, &slug);
        let (parsed_id, _, _, _) = parse_filename(&filename).unwrap();
        prop_assert_eq!(parsed_id.len(), 5);
        prop_assert!(parsed_id.chars().all(|c| c.is_ascii_digit()));
    }
}

proptest! {
    #[test]
    fn filename_starts_with_five_digit_id((id, pri, sta, slug) in arb_task_params()) {
        let filename = format_filename(&id, &pri, &sta, &slug);
        let first_five: String = filename.chars().take(5).collect();
        prop_assert!(first_five.chars().all(|c| c.is_ascii_digit()));
        prop_assert_eq!(&filename.chars().nth(5).unwrap(), &'-');
    }
}

proptest! {
    #[test]
    fn filename_has_exactly_one_double_dash((id, pri, sta, slug) in arb_task_params()) {
        let filename = format_filename(&id, &pri, &sta, &slug);
        prop_assert_eq!(filename.matches("--").count(), 1);
    }
}

proptest! {
    #[test]
    fn non_conforming_filenames_return_none(name in "[a-zA-Z0-9 _.-]{1,60}") {
        let name = if name.ends_with(".md") { name } else { format!("{name}.md") };
        if parse_filename(&name).is_some() {
            return Ok(());
        }
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(&name);
        fs::write(&path, "body\n").unwrap();
        prop_assert!(parse_task_file(&path).is_none());
    }
}

// ---------------------------------------------------------------------------
// Fix idempotency / preservation
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
        fix(tmp.path(), MigrateMode::Skip);
        let result2 = fix(tmp.path(), MigrateMode::Skip);
        prop_assert_eq!(result2.renamed, 0);
        prop_assert!(result2.ok());
    }
}

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
        let fix_result = fix(tmp.path(), MigrateMode::Skip);
        if fix_result.ok() {
            let val_result = validate(tmp.path());
            prop_assert!(
                val_result.ok(),
                "validate failed after fix: {:?}", val_result.errors
            );
        }
    }
}

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
        fix(tmp.path(), MigrateMode::Skip);
        let after = task_files(tmp.path()).unwrap().len();
        prop_assert_eq!(before, after);
    }
}

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

proptest! {
    #[test]
    fn template_and_ancillary_transparent((id, pri, sta, slug) in arb_task_params()) {
        let tmp = TempDir::new().unwrap();
        write_task(tmp.path(), &id, &pri, &sta, &slug);

        fs::write(tmp.path().join("_TEMPLATE.md"), "# Template\n").unwrap();

        let task_stem = format_filename(&id, &pri, &sta, &slug);
        let ancillary_name = task_stem.replace(".md", ".qaplan.md");
        fs::write(tmp.path().join(&ancillary_name), "ancillary content\n").unwrap();

        let val = validate(tmp.path());
        prop_assert!(val.ok(), "unexpected errors: {:?}", val.errors);
        prop_assert_eq!(val.file_count, 1);

        let fix_result = fix(tmp.path(), MigrateMode::Skip);
        prop_assert!(fix_result.ok());
    }
}

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

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn fix_preserves_valid_numeric_ids(
        params in prop::collection::vec(arb_task_params(), 1..=4)
            .prop_filter("unique ids", |v| {
                let ids: HashSet<_> = v.iter().map(|(id, _, _, _)| id.clone()).collect();
                ids.len() == v.len()
            })
    ) {
        let (tmp, _original_ids) = make_task_dir(&params);

        let ids_before: Vec<String> = task_files(tmp.path())
            .unwrap()
            .iter()
            .filter_map(|p| parse_task_file(p))
            .map(|t| t.id.clone())
            .collect();

        fix(tmp.path(), MigrateMode::Skip);

        let ids_after: Vec<String> = task_files(tmp.path())
            .unwrap()
            .iter()
            .filter_map(|p| {
                let name = p.file_name()?.to_string_lossy().to_string();
                let (id, _, _, _) = parse_filename(&name)?;
                Some(id)
            })
            .collect();

        let after_set: HashSet<_> = ids_after.iter().collect();
        for id in &ids_before {
            prop_assert!(
                after_set.contains(id),
                "fix changed task ID {} — IDs after fix: {:?}", id, ids_after
            );
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn fix_idempotency_no_migrations(
        params in prop::collection::vec(arb_task_params(), 1..=4)
            .prop_filter("unique ids", |v| {
                let ids: HashSet<_> = v.iter().map(|(id, _, _, _)| id.clone()).collect();
                ids.len() == v.len()
            })
    ) {
        let (tmp, _) = make_task_dir(&params);

        fix(tmp.path(), MigrateMode::Skip);

        let r2 = fix(tmp.path(), MigrateMode::Skip);
        prop_assert_eq!(r2.renamed, 0, "second fix renamed files");
        prop_assert_eq!(r2.migrated, 0, "second fix migrated files");
        prop_assert!(r2.ok(), "second fix had errors: {:?}", r2.errors);
    }
}

proptest! {
    #[test]
    fn fix_summary_all_correct_iff_all_zero(
        renamed in 0..10usize,
        migrated in 0..10usize,
        renumbered in 0..10usize,
        stripped in 0..10usize,
    ) {
        let summary = fix_summary(renamed, migrated, renumbered, stripped);
        let all_zero = renamed == 0 && migrated == 0 && renumbered == 0 && stripped == 0;
        prop_assert_eq!(
            summary == "All files already correct",
            all_zero,
            "fix_summary({}, {}, {}, {}) = {:?}",
            renamed, migrated, renumbered, stripped, summary
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn fix_migrated_le_renamed(
        params in prop::collection::vec(arb_task_params(), 1..=4)
            .prop_filter("unique ids", |v| {
                let ids: HashSet<_> = v.iter().map(|(id, _, _, _)| id.clone()).collect();
                ids.len() == v.len()
            })
    ) {
        let (tmp, _) = make_task_dir(&params);
        let result = fix(tmp.path(), MigrateMode::Skip);
        prop_assert!(
            result.migrated <= result.renamed,
            "migrated ({}) > renamed ({})",
            result.migrated, result.renamed
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn next_id_never_collides(
        params in prop::collection::vec(arb_task_params(), 1..=6)
            .prop_filter("unique ids", |v| {
                let ids: HashSet<_> = v.iter().map(|(id, _, _, _)| id.clone()).collect();
                ids.len() == v.len()
            })
    ) {
        let (tmp, _) = make_task_dir(&params);
        let new_id = next_id(tmp.path());

        let existing_ids: HashSet<String> = task_files(tmp.path())
            .unwrap()
            .iter()
            .filter_map(|p| {
                let name = p.file_name()?.to_string_lossy().to_string();
                let (id, _, _, _) = parse_filename(&name)?;
                Some(id)
            })
            .collect();

        prop_assert!(
            !existing_ids.contains(&new_id),
            "next_id returned {} which already exists: {:?}", new_id, existing_ids
        );
    }
}

proptest! {
    #[test]
    fn derive_slug_never_empty(title in ".{1,80}") {
        let slug = derive_slug(&title);
        prop_assert!(
            !slug.is_empty(),
            "derive_slug({:?}) returned empty string", title
        );
    }
}

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
