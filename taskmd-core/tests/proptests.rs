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
use taskmd_core::fix::{fix, fix_summary};
use taskmd_core::frontmatter::parse_frontmatter_str;
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
// P26: Fix preserves valid numeric IDs (issue #6)
//
// A task whose ID is already in the 5-digit DDNNN format must keep that exact
// ID after `fix`, regardless of whether the prefix matches the current
// directory. The prefix encodes where the task was *created*; `fix` must not
// overwrite it with the local directory's prefix.
// ---------------------------------------------------------------------------

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

        // Record the IDs present before fix
        let ids_before: Vec<String> = task_files(tmp.path())
            .unwrap()
            .iter()
            .filter_map(|p| parse_task_file(p))
            .map(|t| t.id.clone())
            .collect();

        fix(tmp.path());

        // Collect IDs after fix
        let ids_after: Vec<String> = task_files(tmp.path())
            .unwrap()
            .iter()
            .filter_map(|p| {
                let name = p.file_name()?.to_string_lossy().to_string();
                let (id, _, _, _) = parse_filename(&name)?;
                Some(id)
            })
            .collect();

        // Every original ID must appear unchanged after fix
        let after_set: HashSet<_> = ids_after.iter().collect();
        for id in &ids_before {
            prop_assert!(
                after_set.contains(id),
                "fix changed task ID {} — IDs after fix: {:?}", id, ids_after
            );
        }
    }
}

// ---------------------------------------------------------------------------
// P27: Fix idempotency across directories (issue #6)
//
// Running fix twice in the same directory must be a no-op the second time:
// zero renames, zero patches, zero migrations. This catches cases where fix
// generates unstable filenames.
// ---------------------------------------------------------------------------

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

        // First fix — may do work
        fix(tmp.path());

        // Second fix — must be a complete no-op
        let r2 = fix(tmp.path());
        prop_assert_eq!(r2.patched, 0, "second fix patched files");
        prop_assert_eq!(r2.renamed, 0, "second fix renamed files");
        prop_assert_eq!(r2.migrated, 0, "second fix migrated files");
        prop_assert!(r2.ok(), "second fix had errors: {:?}", r2.errors);
    }
}

// ---------------------------------------------------------------------------
// P28: Fix never modifies body content (bug 1 -- CREATED_RE outside frontmatter)
//
// The body (everything after the closing `---`) must be byte-identical before
// and after `fix`. The bug: when frontmatter lacks `created:` but the body
// has a line starting with `created:`, fix replaces the body line instead of
// injecting into frontmatter.
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn fix_never_modifies_body(
        id in arb_task_id(),
        pri in arb_priority(),
        sta in arb_status(),
        slug in arb_slug(),
        body_prefix in "[a-zA-Z ]{0,20}",
        body_suffix in "[a-zA-Z ]{0,20}",
    ) {
        let tmp = TempDir::new().unwrap();
        let filename = format_filename(&id, &pri, &sta, &slug);

        // Frontmatter deliberately OMITS `created:`. Body contains "created:".
        let body = format!("{body_prefix}created: yesterday by the team{body_suffix}");
        let content = format!(
            "---\npriority: {pri}\nstatus: {sta}\nartifact: src/{slug}.py\n---\n\n{body}\n"
        );
        fs::write(tmp.path().join(&filename), &content).unwrap();

        // Extract body before fix
        let body_before = content
            .match_indices("\n---\n")
            .next()
            .map(|(pos, _)| &content[pos + 5..])
            .unwrap_or("")
            .to_string();

        fix(tmp.path());

        // After fix the file may have been renamed -- find the surviving file
        let files = task_files(tmp.path()).unwrap();
        prop_assert_eq!(files.len(), 1);
        let content_after = fs::read_to_string(&files[0]).unwrap();
        let body_after = content_after
            .match_indices("\n---\n")
            .next()
            .map(|(pos, _)| &content_after[pos + 5..])
            .unwrap_or("")
            .to_string();

        prop_assert_eq!(
            body_before, body_after,
            "fix modified body content"
        );
    }
}

// ---------------------------------------------------------------------------
// P29: fix_summary "all correct" iff all counts are zero (bug 2)
//
// fix_summary is a pure function. It must report "All files already correct"
// if and only if every counter (patched, renamed, migrated, renumbered) is
// zero.
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn fix_summary_all_correct_iff_all_zero(
        patched in 0..10usize,
        renamed in 0..10usize,
        migrated in 0..10usize,
        renumbered in 0..10usize,
    ) {
        let summary = fix_summary(patched, renamed, migrated, renumbered);
        let all_zero = patched == 0 && renamed == 0 && migrated == 0 && renumbered == 0;
        prop_assert_eq!(
            summary == "All files already correct",
            all_zero,
            "fix_summary({}, {}, {}, {}) = {:?}",
            patched, renamed, migrated, renumbered, summary
        );
    }
}

// ---------------------------------------------------------------------------
// P30: fix migrated count never exceeds renamed count (bug 3)
//
// Every migration changes the task ID, which changes the filename, so a
// migration always implies a rename. result.migrated must be <= result.renamed.
// ---------------------------------------------------------------------------

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
        let result = fix(tmp.path());
        prop_assert!(
            result.migrated <= result.renamed,
            "migrated ({}) > renamed ({})",
            result.migrated, result.renamed
        );
    }
}

// ---------------------------------------------------------------------------
// P31: next_id never collides with existing files (bugs 4+5)
//
// For any set of existing tasks (with any mix of prefixes), next_id must
// return an ID that does not match any existing task's ID. This is the
// user-facing invariant and catches both the global-max-sequence waste and
// the overflow-into-occupied-space problems.
// ---------------------------------------------------------------------------

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

        // Collect all existing IDs
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

// ---------------------------------------------------------------------------
// P32: derive_slug never returns empty string (bug 7)
//
// An empty slug produces an unparseable filename via format_filename. Any
// title with at least one ASCII alphanumeric character must yield a non-empty
// slug. Titles with no ASCII alphanumeric content should get a fallback.
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn derive_slug_never_empty(title in ".{1,80}") {
        let slug = derive_slug(&title);
        // If the title has any ASCII alphanumeric char, the slug must be non-empty.
        // If it doesn't, the slug should still be non-empty (fallback).
        prop_assert!(
            !slug.is_empty(),
            "derive_slug({:?}) returned empty string", title
        );
    }
}

// ---------------------------------------------------------------------------
// P33: CRLF frontmatter parses identically to LF (bug 8)
//
// parse_frontmatter_str must produce the same fields whether the input uses
// LF or CRLF line endings. This catches the hardcoded "\n" delimiters.
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn crlf_frontmatter_parses_same_as_lf(
        pri in arb_priority(),
        sta in arb_status(),
        slug in arb_slug(),
    ) {
        let lf_content = format!(
            "---\ncreated: 2026-01-01\npriority: {pri}\nstatus: {sta}\nartifact: src/{slug}.py\n---\n\nBody\n"
        );
        let crlf_content = lf_content.replace('\n', "\r\n");

        let lf_fields = parse_frontmatter_str(&lf_content);
        let crlf_fields = parse_frontmatter_str(&crlf_content);

        prop_assert_eq!(
            lf_fields, crlf_fields,
            "CRLF parse differs from LF parse"
        );
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
