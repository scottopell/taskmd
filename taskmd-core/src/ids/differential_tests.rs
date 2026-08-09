//! End-to-end differential acceptance tests for Git history traversal.
//!
//! Every fixture is a concrete repository on disk. The assertion boundary is
//! the observable next task ID, compared between the production gix backend and
//! the Git CLI reference backend.

use super::{next_id, next_id_with_history, parse_id_parts, prefix_for};
use crate::constants::{Priority, Status};
use crate::filename::format_filename;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use tempfile::TempDir;

struct GitFixture {
    _tmp: TempDir,
    repo: PathBuf,
    tasks: PathBuf,
}

impl GitFixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        init_repo(&repo);
        let tasks = repo.join("tasks");
        std::fs::create_dir(&tasks).unwrap();
        std::fs::write(tasks.join("_TEMPLATE.md"), "# Template\n").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-m", "base"]);
        git(&repo, &["branch", "-M", "main"]);
        Self {
            _tmp: tmp,
            repo,
            tasks,
        }
    }

    fn task_path(&self, prefix: &str, sequence: u32, slug: &str) -> PathBuf {
        self.tasks.join(format_filename(
            &format!("{prefix}{sequence:03}"),
            Priority::P2,
            Status::Ready,
            slug,
        ))
    }

    fn write_local_task(&self, sequence: u32, slug: &str) -> PathBuf {
        let prefix = prefix_for(&self.tasks);
        let path = self.task_path(&prefix, sequence, slug);
        std::fs::write(&path, format!("# {slug}\n")).unwrap();
        path
    }

    fn commit_all(&self, message: &str) {
        git(&self.repo, &["add", "-A"]);
        git(&self.repo, &["commit", "-m", message]);
    }

    fn assert_next_sequence(&self, expected: u32) {
        assert_differential_next(&self.tasks, expected);
    }
}

fn init_repo(repo: &Path) {
    std::fs::create_dir_all(repo).unwrap();
    git(repo, &["init"]);
    git(repo, &["config", "user.name", "taskmd test"]);
    git(repo, &["config", "user.email", "taskmd@example.invalid"]);
    git(repo, &["config", "commit.gpgsign", "false"]);
}

fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap_or_else(|error| panic!("failed to run git {args:?}: {error}"));
    assert!(
        output.status.success(),
        "git {args:?} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn git_stdout(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap_or_else(|error| panic!("failed to run git {args:?}: {error}"));
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn assert_differential_next(tasks_dir: &Path, expected_sequence: u32) {
    let gix_id = next_id(tasks_dir);
    let cli_id = next_id_with_history(
        tasks_dir,
        crate::git_history::task_filenames_via_git_cli(tasks_dir),
    );
    assert_eq!(gix_id, cli_id, "gix and Git CLI outcomes diverged");

    let (_, actual_sequence) = parse_id_parts(&gix_id);
    assert_eq!(actual_sequence, expected_sequence, "unexpected next ID");
}

#[test]
fn sibling_local_branches_reserve_hidden_task() {
    let fixture = GitFixture::new();
    git(&fixture.repo, &["switch", "-c", "fix-one"]);
    let first = fixture.write_local_task(1, "first-fix");
    fixture.commit_all("first fix");

    git(&fixture.repo, &["switch", "main"]);
    git(&fixture.repo, &["switch", "-c", "fix-two"]);
    assert!(!first.exists(), "fixture must hide the sibling task");

    fixture.assert_next_sequence(2);
}

#[test]
fn remote_tracking_branch_reserves_task() {
    let fixture = GitFixture::new();
    git(&fixture.repo, &["switch", "-c", "remote-only"]);
    fixture.write_local_task(1, "remote-fix");
    fixture.commit_all("remote fix");
    let tip = git_stdout(&fixture.repo, &["rev-parse", "HEAD"]);

    git(&fixture.repo, &["switch", "main"]);
    git(
        &fixture.repo,
        &["update-ref", "refs/remotes/origin/remote-only", &tip],
    );
    git(&fixture.repo, &["branch", "-D", "remote-only"]);
    git(
        &fixture.repo,
        &["reflog", "expire", "--expire=all", "--all"],
    );

    fixture.assert_next_sequence(2);
}

#[test]
fn reflog_only_commit_reserves_task() {
    let fixture = GitFixture::new();
    git(&fixture.repo, &["switch", "-c", "ephemeral"]);
    fixture.write_local_task(1, "ephemeral-fix");
    fixture.commit_all("ephemeral fix");

    git(&fixture.repo, &["switch", "main"]);
    git(&fixture.repo, &["branch", "-D", "ephemeral"]);

    fixture.assert_next_sequence(2);
}

#[test]
fn renamed_then_deleted_task_remains_reserved() {
    let fixture = GitFixture::new();
    let original = fixture.write_local_task(1, "original");
    fixture.commit_all("add task");

    let renamed = fixture.write_local_task(1, "renamed");
    std::fs::remove_file(original).unwrap();
    fixture.commit_all("rename task");
    std::fs::remove_file(renamed).unwrap();
    fixture.commit_all("delete task");

    fixture.assert_next_sequence(2);
}

#[test]
fn shallow_clone_uses_only_available_history() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let clone = tmp.path().join("clone");
    let clone_tasks = clone.join("tasks");
    let clone_prefix = prefix_for(&clone_tasks);

    init_repo(&source);
    let source_tasks = source.join("tasks");
    std::fs::create_dir(&source_tasks).unwrap();
    std::fs::write(source_tasks.join("_TEMPLATE.md"), "# Template\n").unwrap();
    let historical = source_tasks.join(format_filename(
        &format!("{clone_prefix}001"),
        Priority::P2,
        Status::Ready,
        "old-task",
    ));
    std::fs::write(&historical, "# old task\n").unwrap();
    git(&source, &["add", "."]);
    git(&source, &["commit", "-m", "historical task"]);
    std::fs::remove_file(historical).unwrap();
    git(&source, &["add", "-A"]);
    git(&source, &["commit", "-m", "delete historical task"]);

    let source_url = format!("file://{}", source.display());
    git(
        tmp.path(),
        &[
            "clone",
            "--depth",
            "1",
            &source_url,
            clone.to_str().unwrap(),
        ],
    );

    assert_differential_next(&clone_tasks, 1);
}

#[test]
fn non_git_directory_falls_back_to_visible_tasks() {
    let tmp = tempfile::tempdir().unwrap();
    let tasks = tmp.path().join("tasks");
    std::fs::create_dir(&tasks).unwrap();
    let prefix = prefix_for(&tasks);
    let filename = format_filename(
        &format!("{prefix}005"),
        Priority::P2,
        Status::Ready,
        "visible",
    );
    std::fs::write(tasks.join(filename), "# visible\n").unwrap();

    assert_differential_next(&tasks, 6);
}

#[test]
fn foreign_and_malformed_filenames_do_not_advance_local_sequence() {
    let fixture = GitFixture::new();
    fixture.write_local_task(5, "local");
    let local_prefix: u32 = prefix_for(&fixture.tasks).parse().unwrap();
    let foreign_prefix = format!("{:02}", (local_prefix + 1) % 100);
    let foreign = fixture.task_path(&foreign_prefix, 900, "foreign");
    std::fs::write(foreign, "# foreign\n").unwrap();
    std::fs::write(fixture.tasks.join("not-a-task.md"), "# malformed\n").unwrap();
    fixture.commit_all("mixed filenames");

    fixture.assert_next_sequence(6);
}

/// Manual, repeatable latency comparison on taskmd's own Git history.
#[test]
#[ignore = "measurement harness; run explicitly with --ignored --nocapture"]
fn measure_history_backends_on_taskmd_repository() {
    let tasks = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tasks");

    let gix_names = crate::git_history::task_filenames(&tasks);
    let cli_names = crate::git_history::task_filenames_via_git_cli(&tasks);
    assert_eq!(gix_names, cli_names);

    let gix = measure(25, || crate::git_history::task_filenames(&tasks));
    let cli = measure(25, || {
        crate::git_history::task_filenames_via_git_cli(&tasks)
    });
    eprintln!("history traversal median (25 runs): gix={gix:?}, git-cli={cli:?}");
}

fn measure<T>(runs: usize, mut operation: impl FnMut() -> T) -> Duration {
    let mut samples = Vec::with_capacity(runs);
    for _ in 0..runs {
        let started = Instant::now();
        std::hint::black_box(operation());
        samples.push(started.elapsed());
    }
    samples.sort_unstable();
    samples[samples.len() / 2]
}
