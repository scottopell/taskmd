use std::str::FromStr;
use std::sync::LazyLock;

use regex::Regex;

use crate::constants::{Priority, Status, VALID_PRIORITIES, VALID_STATUSES};

/// Maximum length (in bytes) of the slug component of a task filename.
pub const MAX_SLUG_LEN: usize = 40;

/// Result of parsing a task filename into its constituent fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFilename {
    pub id: String,
    pub priority: Priority,
    pub status: Status,
    pub slug: String,
}

/// The canonical regex pattern for a valid task filename, built at startup from
/// the `VALID_STATUSES` and `VALID_PRIORITIES` constants so the two can never
/// drift apart.
///
/// Exported so consumers (Python, tests) compile it themselves rather than
/// maintaining a separate copy.
pub static FILENAME_PATTERN: LazyLock<String> = LazyLock::new(|| {
    let statuses = VALID_STATUSES.join("|");
    let priorities = VALID_PRIORITIES.join("|");
    // Three ID formats (tried in order):
    //   \d{5}              — new all-numeric DDNNN (e.g. 34042)
    //   [A-HJ-NP-Z]{2}\d{3} — old alpha-prefix AANNN (e.g. YF042), letters-only
    //                         prefix so it doesn't overlap with \d{5}
    //   \d{4}              — legacy 4-digit (e.g. 0042)
    format!(r"^(\d{{5}}|[A-HJ-NP-Z]{{2}}\d{{3}}|\d{{4}})-({priorities})-({statuses})--(.+)\.md$")
});

static FILENAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&FILENAME_PATTERN).expect("FILENAME_PATTERN is valid regex"));

/// Parse a task filename into its constituent fields.
/// Returns `None` if the name doesn't match the expected pattern.
pub fn parse_filename(name: &str) -> Option<ParsedFilename> {
    let caps = FILENAME_RE.captures(name)?;
    // The regex is built from `VALID_PRIORITIES` / `VALID_STATUSES`, so the
    // priority/status captures are guaranteed to match a known variant in
    // practice. If the impossible happens (e.g. someone changed the const
    // arrays without updating the enums), return `None` rather than panic.
    let priority = Priority::from_str(&caps[2]).ok()?;
    let status = Status::from_str(&caps[3]).ok()?;
    Some(ParsedFilename {
        id: caps[1].to_string(),
        priority,
        status,
        slug: caps[4].to_string(),
    })
}

/// Build the canonical filename for a task. Double-dash separates status from slug.
pub fn format_filename(id: &str, priority: Priority, status: Status, slug: &str) -> String {
    format!("{id}-{}-{}--{slug}.md", priority.as_str(), status.as_str())
}

/// Convert a human-readable title to a valid taskmd slug.
///
/// Rules: lowercase → replace non-alphanumeric runs with a single hyphen →
/// strip leading/trailing hyphens → truncate at [`MAX_SLUG_LEN`] characters.
///
/// Returns `"untitled"` if the input contains no alphanumeric characters
/// (e.g. empty string, whitespace, all punctuation). Callers that want to
/// reject such input must validate before calling — `create_task` and
/// `update_task` both do this and surface `Error::InvalidSlug`.
pub fn derive_slug(title: &str) -> String {
    let lower = title.to_lowercase();
    let mut slug = String::with_capacity(lower.len().min(MAX_SLUG_LEN + 10));
    let mut last_was_sep = true; // start true to suppress a leading hyphen

    for c in lower.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
            last_was_sep = false;
        } else if !last_was_sep && !slug.is_empty() {
            slug.push('-');
            last_was_sep = true;
        }
    }

    if slug.ends_with('-') {
        slug.pop();
    }

    if slug.len() > MAX_SLUG_LEN {
        slug.truncate(MAX_SLUG_LEN);
        if slug.ends_with('-') {
            slug.pop();
        }
    }

    if slug.is_empty() {
        slug.push_str("untitled");
    }

    slug
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_contains_all_valid_statuses() {
        for status in VALID_STATUSES {
            let name = format!("34042-p2-{status}--slug.md");
            assert!(
                parse_filename(&name).is_some(),
                "pattern missing status '{status}'"
            );
        }
    }

    #[test]
    fn pattern_contains_all_valid_priorities() {
        for priority in VALID_PRIORITIES {
            let name = format!("34042-{priority}-ready--slug.md");
            assert!(
                parse_filename(&name).is_some(),
                "pattern missing priority '{priority}'"
            );
        }
    }

    #[test]
    fn parse_numeric_format() {
        let r = parse_filename("34042-p2-ready--fix-the-bug.md").unwrap();
        assert_eq!(r.id, "34042");
        assert_eq!(r.priority, Priority::P2);
        assert_eq!(r.status, Status::Ready);
        assert_eq!(r.slug, "fix-the-bug");
    }

    #[test]
    fn parse_alpha_prefix_format() {
        let r = parse_filename("YF042-p2-ready--fix-the-bug.md").unwrap();
        assert_eq!(r.id, "YF042");
        assert_eq!(r.priority, Priority::P2);
        assert_eq!(r.status, Status::Ready);
        assert_eq!(r.slug, "fix-the-bug");
    }

    #[test]
    fn parse_legacy_format() {
        let r = parse_filename("0042-p1-in-progress--refactor.md").unwrap();
        assert_eq!(r.id, "0042");
        assert_eq!(r.priority, Priority::P1);
        assert_eq!(r.status, Status::InProgress);
        assert_eq!(r.slug, "refactor");
    }

    #[test]
    fn parse_rejects_bad_name() {
        assert!(parse_filename("not-a-task.md").is_none());
        assert!(parse_filename("34042-p5-ready--slug.md").is_none()); // p5 invalid
        assert!(parse_filename("34042-p2-pending--slug.md").is_none()); // unknown status
    }

    #[test]
    fn format_roundtrip_numeric() {
        let name = "34042-p2-in-progress--my-slug.md";
        let p = parse_filename(name).unwrap();
        assert_eq!(format_filename(&p.id, p.priority, p.status, &p.slug), name);
    }

    #[test]
    fn format_roundtrip_alpha() {
        let name = "YF042-p2-in-progress--my-slug.md";
        let p = parse_filename(name).unwrap();
        assert_eq!(format_filename(&p.id, p.priority, p.status, &p.slug), name);
    }

    #[test]
    fn derive_slug_basic() {
        assert_eq!(derive_slug("Fix the Bug"), "fix-the-bug");
    }

    #[test]
    fn derive_slug_special_chars() {
        assert_eq!(derive_slug("Add OAuth2 support!"), "add-oauth2-support");
    }

    #[test]
    fn derive_slug_truncates_at_max() {
        let long = "a".repeat(MAX_SLUG_LEN + 10);
        assert!(derive_slug(&long).len() <= MAX_SLUG_LEN);
    }

    #[test]
    fn derive_slug_no_trailing_hyphen() {
        assert!(!derive_slug("hello world ").ends_with('-'));
        assert!(!derive_slug(&("a".repeat(MAX_SLUG_LEN) + "-")).ends_with('-'));
    }
}
