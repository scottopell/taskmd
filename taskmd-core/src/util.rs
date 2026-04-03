/// Validate a date string has the format YYYY-MM-DD (no calendar correctness check).
pub fn is_valid_date(s: &str) -> bool {
    if s.len() != 10 {
        return false;
    }
    let b = s.as_bytes();
    b[4] == b'-'
        && b[7] == b'-'
        && b[..4].iter().all(|c| c.is_ascii_digit())
        && b[5..7].iter().all(|c| c.is_ascii_digit())
        && b[8..10].iter().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid() {
        assert!(is_valid_date("2026-03-30"));
        assert!(is_valid_date("2000-01-01"));
    }

    #[test]
    fn rejects_invalid() {
        assert!(!is_valid_date("26-03-30"));
        assert!(!is_valid_date("2026/03/30"));
        assert!(!is_valid_date("not-a-date"));
        assert!(!is_valid_date(""));
    }
}
