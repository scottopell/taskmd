use std::borrow::Cow;

/// Normalise CRLF line endings to LF.
///
/// Returns a borrowed reference when no `\r\n` is present (zero-cost),
/// or an owned copy with `\r\n` replaced otherwise.
pub fn normalize_line_endings(s: &str) -> Cow<'_, str> {
    if s.contains("\r\n") {
        Cow::Owned(s.replace("\r\n", "\n"))
    } else {
        Cow::Borrowed(s)
    }
}
