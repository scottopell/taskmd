use chrono::Local;
use std::path::Path;

/// Infer the creation date for a file: git log → file mtime → today.
///
/// All dates are in the local timezone to match `git log` output and
/// Python's `date.fromtimestamp()` behaviour.
pub fn infer_created_date(path: &Path) -> String {
    if let Some(d) = git_date(path) {
        return d;
    }
    if let Some(d) = mtime_date(path) {
        return d;
    }
    today()
}

fn git_date(path: &Path) -> Option<String> {
    let parent = path.parent()?;
    let output = std::process::Command::new("git")
        .args(["log", "--follow", "--diff-filter=A", "--format=%as"])
        .arg(path)
        .current_dir(parent)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    let last = stdout.trim().lines().next_back()?.to_string();
    if last.is_empty() { None } else { Some(last) }
}

fn mtime_date(path: &Path) -> Option<String> {
    use chrono::DateTime;
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    let secs = modified
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    // Convert UTC Unix timestamp → local date (matches Python's date.fromtimestamp)
    let dt = DateTime::from_timestamp(secs, 0)?.with_timezone(&Local);
    Some(dt.format("%Y-%m-%d").to_string())
}

pub fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn today_is_formatted_correctly() {
        let d = today();
        assert_eq!(d.len(), 10);
        assert_eq!(&d[4..5], "-");
        assert_eq!(&d[7..8], "-");
    }
}
