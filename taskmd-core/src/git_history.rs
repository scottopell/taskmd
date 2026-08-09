//! Read task filenames from locally known Git history.
//!
//! Production traversal uses gitoxide (`gix`) exclusively. The Git CLI
//! implementation is retained only as a differential-test oracle.

use std::collections::HashSet;
use std::path::Path;

/// Return every filename that has appeared directly inside `tasks_dir` in a
/// commit reachable from local refs, remote-tracking refs, or reflogs.
///
/// Any discovery or object-database failure produces an empty result so callers
/// retain taskmd's filesystem-only fallback outside usable Git repositories.
pub(crate) fn task_filenames(tasks_dir: &Path) -> Vec<String> {
    try_task_filenames(tasks_dir).unwrap_or_default()
}

fn try_task_filenames(tasks_dir: &Path) -> Option<Vec<String>> {
    let mut repo = gix::discover(tasks_dir).ok()?;
    repo.object_cache_size_if_unset(4 * 1024 * 1024);

    let workdir = repo.workdir()?.canonicalize().ok()?;
    let tasks_dir = tasks_dir.canonicalize().ok()?;
    let relative_tasks_dir = tasks_dir.strip_prefix(workdir).ok()?;

    let mut starts = HashSet::new();
    let mut reflog_names = Vec::new();

    let reference_platform = repo.references().ok()?;
    let references = reference_platform.all().ok()?;
    for reference in references.flatten() {
        reflog_names.push(reference.name().to_owned());
        if let Some(id) = reference.try_id() {
            add_commit_start(&repo, id.detach(), &mut starts);
        }
    }

    if let Ok(head_id) = repo.head_id() {
        add_commit_start(&repo, head_id.detach(), &mut starts);
    }

    let mut reflog_buf = Vec::new();
    for name in &reflog_names {
        add_reflog_starts(&repo, name.as_ref(), &mut reflog_buf, &mut starts);
    }
    let head: gix::refs::FullName = "HEAD".try_into().ok()?;
    add_reflog_starts(&repo, head.as_ref(), &mut reflog_buf, &mut starts);

    if starts.is_empty() {
        return Some(Vec::new());
    }

    let walk = repo.rev_walk(starts).all().ok()?;
    let mut seen_task_trees = HashSet::new();
    let mut filenames = HashSet::new();

    for info in walk.flatten() {
        let Ok(commit) = info.object() else {
            continue;
        };
        let Ok(root) = commit.tree() else {
            continue;
        };

        let task_tree = if relative_tasks_dir.as_os_str().is_empty() {
            root
        } else {
            let Ok(Some(entry)) = root.lookup_entry_by_path(relative_tasks_dir) else {
                continue;
            };
            if !entry.mode().is_tree() {
                continue;
            }
            let Ok(object) = entry.object() else {
                continue;
            };
            object.into_tree()
        };

        if !seen_task_trees.insert(task_tree.id().detach()) {
            continue;
        }

        for entry in task_tree.iter().flatten() {
            if let Ok(name) = std::str::from_utf8(entry.filename().as_ref()) {
                filenames.insert(name.to_owned());
            }
        }
    }

    let mut filenames: Vec<_> = filenames.into_iter().collect();
    filenames.sort();
    Some(filenames)
}

fn add_commit_start(
    repo: &gix::Repository,
    id: gix::ObjectId,
    starts: &mut HashSet<gix::ObjectId>,
) {
    if id.is_null() {
        return;
    }
    if let Ok(object) = repo.find_object(id) {
        if let Ok(commit) = object.peel_to_commit() {
            starts.insert(commit.id().detach());
        }
    }
}

fn add_reflog_starts(
    repo: &gix::Repository,
    name: &gix::refs::FullNameRef,
    buf: &mut Vec<u8>,
    starts: &mut HashSet<gix::ObjectId>,
) {
    let Ok(Some(lines)) = repo.refs.reflog_iter(name, buf) else {
        return;
    };
    for line in lines.flatten() {
        add_commit_start(repo, line.previous_oid(), starts);
        add_commit_start(repo, line.new_oid(), starts);
    }
}

#[cfg(test)]
pub(crate) fn task_filenames_via_git_cli(tasks_dir: &Path) -> Vec<String> {
    use std::process::Command;

    let Ok(output) = Command::new("git")
        .args([
            "log",
            "--all",
            "--reflog",
            "--name-only",
            "--format=",
            "--",
            ".",
        ])
        .current_dir(tasks_dir)
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let mut filenames: Vec<_> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| Path::new(line).file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    filenames.sort();
    filenames
}
