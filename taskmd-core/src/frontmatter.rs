use std::collections::HashMap;
use std::path::Path;

/// The opening delimiter of a YAML frontmatter block.
pub(crate) const FRONTMATTER_OPEN: &str = "---\n";

/// The closing delimiter (always preceded by a newline in well-formed files).
const FRONTMATTER_CLOSE: &str = "\n---\n";

/// Parse YAML frontmatter from a string into flat key-value pairs.
///
/// Splits on the first `:` so values containing colons are handled correctly,
/// matching the Python `line.partition(":")` behaviour.
///
/// Callers that read from disk should normalise CRLF before calling this
/// (see [`crate::util::normalize_line_endings`]).  As a safety net, this
/// function also normalises internally.
pub fn parse_frontmatter_str(content: &str) -> HashMap<String, String> {
    let content = crate::util::normalize_line_endings(content);

    let mut fields = HashMap::new();

    if !content.starts_with(FRONTMATTER_OPEN) {
        return fields;
    }

    let body_start = FRONTMATTER_OPEN.len();
    let end = match content[body_start..].find(FRONTMATTER_CLOSE) {
        Some(pos) => body_start + pos,
        None => return fields,
    };

    let body = content[body_start..end].trim();
    for line in body.lines() {
        if let Some(colon) = line.find(':') {
            let key = line[..colon].trim();
            let value = line[colon + 1..].trim();
            if !key.is_empty() {
                fields.insert(key.to_string(), value.to_string());
            }
        }
    }

    fields
}

/// Parse frontmatter from a file on disk.
pub fn parse_frontmatter_file(path: &Path) -> std::io::Result<HashMap<String, String>> {
    let content = std::fs::read_to_string(path)?;
    Ok(parse_frontmatter_str(&content))
}

/// Return true if `content` has syntactically valid frontmatter
/// (starts with `---\n` and contains a closing `\n---\n`).
/// Handles both LF and CRLF line endings.
pub fn has_valid_frontmatter(content: &str) -> bool {
    let content = crate::util::normalize_line_endings(content);
    content.starts_with(FRONTMATTER_OPEN)
        && content[FRONTMATTER_OPEN.len()..].contains(FRONTMATTER_CLOSE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_fields() {
        let content = "---\nstatus: ready\npriority: p2\n---\n\nBody\n";
        let fields = parse_frontmatter_str(content);
        assert_eq!(fields["status"], "ready");
        assert_eq!(fields["priority"], "p2");
    }

    #[test]
    fn handles_colon_in_value() {
        let content = "---\nartifact: path/to/file:line\n---\n";
        let fields = parse_frontmatter_str(content);
        assert_eq!(fields["artifact"], "path/to/file:line");
    }

    #[test]
    fn returns_empty_on_missing_frontmatter() {
        let fields = parse_frontmatter_str("no frontmatter here\n");
        assert!(fields.is_empty());
    }

    #[test]
    fn returns_empty_on_unclosed_frontmatter() {
        let fields = parse_frontmatter_str("---\nstatus: ready\n");
        assert!(fields.is_empty());
    }
}
