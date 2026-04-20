"""Tests for taskmd core library.

Exercises validate, fix, next_id, and parse_task_file against
real task files on disk. Uses tmp_path fixtures -- no global state.
"""
from pathlib import Path

import pytest

from taskmd.core import (
    VALID_FIELDS,
    VALID_PRIORITIES,
    VALID_STATUSES,
    ValidationResult,
    _is_legacy_id,
    _needs_migration,
    _parse_id_parts,
    _prefix_for,
    create_task,
    fix,
    get_expected_filename,
    init,
    next_id,
    parse_task_file,
    validate,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def make_task(tasks_dir: Path, task_id: str, priority: str, status: str, slug: str, **extra_fields) -> Path:
    """Create a valid task file on disk."""
    fields = {"created": "2026-03-04", "priority": priority, "status": status, "artifact": f"src/{slug}.py"}
    fields.update(extra_fields)
    fm = "\n".join(f"{k}: {v}" for k, v in fields.items())
    filename = get_expected_filename(task_id, priority, status, slug)
    path = tasks_dir / filename
    path.write_text(f"---\n{fm}\n---\n\n# Task {task_id}\n\nSummary here.\n")
    return path


def make_legacy_task(tasks_dir: Path, number: int, priority: str, status: str, slug: str, **extra_fields) -> Path:
    """Create a legacy 4-digit format task file on disk."""
    fields = {"created": "2026-03-04", "priority": priority, "status": status, "artifact": f"src/{slug}.py"}
    fields.update(extra_fields)
    fm = "\n".join(f"{k}: {v}" for k, v in fields.items())
    filename = f"{number:04d}-{priority}-{status}--{slug}.md"
    path = tasks_dir / filename
    path.write_text(f"---\n{fm}\n---\n\n# Task {number}\n\nSummary here.\n")
    return path


def make_template(tasks_dir: Path) -> Path:
    path = tasks_dir / "_TEMPLATE.md"
    path.write_text("---\ncreated: YYYY-MM-DD\npriority: p2\nstatus: ready\n---\n\n# Template\n")
    return path


# ---------------------------------------------------------------------------
# ID helpers
# ---------------------------------------------------------------------------

class TestIdHelpers:
    def test_prefix_deterministic(self, tmp_path):
        assert _prefix_for(tmp_path) == _prefix_for(tmp_path)

    def test_prefix_is_two_digits(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        assert len(prefix) == 2
        assert all(c.isdigit() for c in prefix)

    def test_prefix_stable_before_and_after_dir_creation(self, tmp_path):
        """Prefix for a non-existent dir matches prefix after creation."""
        tasks = tmp_path / "tasks"
        before = _prefix_for(tasks)
        tasks.mkdir()
        after = _prefix_for(tasks)
        assert before == after

    def test_parse_id_parts_numeric(self):
        assert _parse_id_parts("34042") == ("34", 42)
        assert _parse_id_parts("00001") == ("00", 1)
        assert _parse_id_parts("99999") == ("99", 999)

    def test_parse_id_parts_alpha(self):
        """Old alpha-prefix IDs still parse correctly."""
        assert _parse_id_parts("AB042") == ("AB", 42)
        assert _parse_id_parts("ZZ999") == ("ZZ", 999)

    def test_parse_id_parts_legacy(self):
        assert _parse_id_parts("0042") == ("", 42)
        assert _parse_id_parts("9999") == ("", 9999)

    def test_is_legacy_id(self):
        assert _is_legacy_id("0042")
        assert _is_legacy_id("9999")
        assert not _is_legacy_id("AB042")
        assert not _is_legacy_id("34042")
        assert not _is_legacy_id("42")

    def test_needs_migration(self):
        # Legacy 4-digit always needs migration
        assert _needs_migration("0042", "34")
        # Alpha prefix always needs migration
        assert _needs_migration("YF042", "34")
        # A valid numeric prefix from another worktree must NOT be migrated —
        # the prefix encodes where the task was created; rewriting it would
        # destroy cross-worktree identity (see issue #6 and the Rust test
        # `needs_migration_different_numeric_prefix_is_not_migrated`).
        assert not _needs_migration("21042", "34")
        # Correct prefix does not need migration
        assert not _needs_migration("34042", "34")


# ---------------------------------------------------------------------------
# parse_task_file
# ---------------------------------------------------------------------------

class TestParseTaskFile:
    def test_valid_file(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        tid = f"{prefix}042"
        p = make_task(tmp_path, tid, "p2", "ready", "fix-bug")
        task = parse_task_file(p)
        assert task is not None
        assert task.id == tid
        assert task.priority == "p2"
        assert task.status == "ready"
        assert task.slug == "fix-bug"
        assert task.fields["created"] == "2026-03-04"

    def test_invalid_filename(self, tmp_path):
        p = tmp_path / "not-a-task.md"
        p.write_text("---\nstatus: ready\n---\n")
        assert parse_task_file(p) is None

    def test_old_3digit_format_rejected(self, tmp_path):
        p = tmp_path / "042-p2-ready--fix-bug.md"
        p.write_text("---\ncreated: 2026-03-04\npriority: p2\nstatus: ready\n---\n")
        assert parse_task_file(p) is None  # 3 digits not accepted

    def test_single_dash_rejected(self, tmp_path):
        p = tmp_path / "34042-p2-ready-fix-bug.md"  # single dash before slug
        p.write_text("---\ncreated: 2026-03-04\npriority: p2\nstatus: ready\n---\n")
        assert parse_task_file(p) is None

    def test_legacy_4digit_file(self, tmp_path):
        p = make_legacy_task(tmp_path, 42, "p1", "done", "big-feature")
        task = parse_task_file(p)
        assert task is not None
        assert task.id == "0042"

    def test_alpha_prefix_file(self, tmp_path):
        """Old alpha-prefix AANNN files still parse."""
        p = make_task(tmp_path, "AB123", "p1", "done", "big-feature")
        task = parse_task_file(p)
        assert task is not None
        assert task.id == "AB123"

    def test_numeric_prefix_file(self, tmp_path):
        p = make_task(tmp_path, "34042", "p1", "done", "big-feature")
        task = parse_task_file(p)
        assert task is not None
        assert task.id == "34042"

    def test_all_statuses_parse(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        for i, status in enumerate(sorted(VALID_STATUSES), start=1):
            p = make_task(tmp_path, f"{prefix}{i:03d}", "p2", status, f"test-{status}")
            task = parse_task_file(p)
            assert task is not None, f"Failed to parse status: {status}"
            assert task.status == status

    def test_frontmatter_with_colon_in_value(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        p = make_task(tmp_path, f"{prefix}001", "p2", "ready", "test")
        content = p.read_text()
        content = content.replace("---\n\n#", 'title: "QA Report: Streaming"\n---\n\n#')
        p.write_text(content)
        task = parse_task_file(p)
        assert task is not None


# ---------------------------------------------------------------------------
# get_expected_filename
# ---------------------------------------------------------------------------

class TestGetExpectedFilename:
    def test_numeric_format(self):
        assert get_expected_filename("34042", "p2", "ready", "fix-bug") == "34042-p2-ready--fix-bug.md"

    def test_different_prefix(self):
        assert get_expected_filename("21001", "p0", "done", "big") == "21001-p0-done--big.md"

    def test_legacy_format(self):
        assert get_expected_filename("0001", "p4", "brainstorming", "idea") == "0001-p4-brainstorming--idea.md"


# ---------------------------------------------------------------------------
# validate
# ---------------------------------------------------------------------------

class TestValidate:
    def test_empty_dir(self, tmp_path):
        result = validate(tmp_path)
        assert result.ok
        assert result.file_count == 0

    def test_nonexistent_dir(self, tmp_path):
        result = validate(tmp_path / "nope")
        assert result.ok

    def test_valid_tasks(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        make_template(tmp_path)
        make_task(tmp_path, f"{prefix}001", "p2", "ready", "first")
        make_task(tmp_path, f"{prefix}002", "p1", "done", "second")
        result = validate(tmp_path)
        assert result.ok
        assert result.file_count == 2

    def test_missing_frontmatter(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        p = tmp_path / f"{prefix}001-p2-ready--no-fm.md"
        p.write_text("# No frontmatter\n")
        result = validate(tmp_path)
        assert not result.ok
        assert any("missing YAML frontmatter" in e for e in result.errors)

    def test_missing_status(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        p = tmp_path / f"{prefix}001-p2-ready--test.md"
        p.write_text("---\ncreated: 2026-03-04\npriority: p2\n---\n")
        result = validate(tmp_path)
        assert not result.ok
        assert any("missing 'status'" in e for e in result.errors)

    def test_missing_priority(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        p = tmp_path / f"{prefix}001-p2-ready--test.md"
        p.write_text("---\ncreated: 2026-03-04\nstatus: ready\n---\n")
        result = validate(tmp_path)
        assert not result.ok
        assert any("missing 'priority'" in e for e in result.errors)

    def test_missing_created(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        p = tmp_path / f"{prefix}001-p2-ready--test.md"
        p.write_text("---\npriority: p2\nstatus: ready\n---\n")
        result = validate(tmp_path)
        assert not result.ok
        assert any("missing 'created'" in e for e in result.errors)

    def test_invalid_status(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        p = tmp_path / f"{prefix}001-p2-ready--test.md"
        p.write_text("---\ncreated: 2026-03-04\npriority: p2\nstatus: pending\n---\n")
        result = validate(tmp_path)
        assert not result.ok
        assert any("invalid status" in e for e in result.errors)

    def test_filename_mismatch(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        # Frontmatter says done, filename says ready
        p = tmp_path / f"{prefix}001-p2-ready--test.md"
        p.write_text("---\ncreated: 2026-03-04\npriority: p2\nstatus: done\n---\n")
        result = validate(tmp_path)
        assert not result.ok
        assert any("doesn't match frontmatter" in e for e in result.errors)

    def test_duplicate_ids(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        make_task(tmp_path, f"{prefix}001", "p2", "ready", "first")
        make_task(tmp_path, f"{prefix}001", "p1", "done", "second")
        result = validate(tmp_path)
        assert not result.ok
        assert any("duplicate task id" in e for e in result.errors)

    def test_template_skipped(self, tmp_path):
        make_template(tmp_path)
        result = validate(tmp_path)
        assert result.ok
        assert result.file_count == 0  # template not counted

    def test_ancillary_skipped(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        make_task(tmp_path, f"{prefix}001", "p2", "ready", "test")
        # Create ancillary files
        (tmp_path / f"{prefix}001-p2-ready--test.qaplan.md").write_text("---\ncreated: 2026-03-04\n---\n")
        (tmp_path / f"{prefix}001-p2-ready--test.qareport.md").write_text("---\ncreated: 2026-03-04\n---\n")
        result = validate(tmp_path)
        assert result.ok
        assert result.file_count == 1  # only the main task

    def test_legacy_format_still_validates(self, tmp_path):
        make_legacy_task(tmp_path, 1, "p2", "ready", "test")
        result = validate(tmp_path)
        assert result.ok
        assert result.file_count == 1


# ---------------------------------------------------------------------------
# fix
# ---------------------------------------------------------------------------

class TestFix:
    def test_inject_missing_created(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        p = tmp_path / f"{prefix}001-p2-ready--test.md"
        p.write_text("---\npriority: p2\nstatus: ready\n---\n\n# Test\n")
        result = fix(tmp_path)
        assert result.patched == 1
        content = (tmp_path / f"{prefix}001-p2-ready--test.md").read_text()
        assert "created:" in content

    def test_replace_malformed_created(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        p = tmp_path / f"{prefix}001-p2-ready--test.md"
        p.write_text("---\ncreated: YYYY-MM-DD\npriority: p2\nstatus: ready\n---\n")
        fix(tmp_path)
        content = (tmp_path / f"{prefix}001-p2-ready--test.md").read_text()
        assert "YYYY-MM-DD" not in content
        assert "created:" in content
        # Run again -- should be idempotent
        result2 = fix(tmp_path)
        assert result2.patched == 0

    def test_rename_to_match_frontmatter(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        p = tmp_path / f"{prefix}001-p2-ready--old-name.md"
        p.write_text("---\ncreated: 2026-03-04\npriority: p1\nstatus: done\n---\n")
        result = fix(tmp_path)
        assert result.renamed == 1
        assert (tmp_path / f"{prefix}001-p1-done--old-name.md").exists()
        assert not p.exists()

    def test_rename_conflict(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        make_task(tmp_path, f"{prefix}001", "p1", "done", "target")
        p = tmp_path / f"{prefix}001-p2-ready--target.md"
        p.write_text("---\ncreated: 2026-03-04\npriority: p1\nstatus: done\n---\n")
        result = fix(tmp_path)
        assert any("cannot rename" in e for e in result.errors)

    def test_ancillary_skipped_by_fix(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        make_task(tmp_path, f"{prefix}001", "p2", "ready", "test")
        (tmp_path / f"{prefix}001-p2-ready--test.qaplan.md").write_text("garbage")
        result = fix(tmp_path)
        assert result.ok  # no errors from the qaplan file

    def test_migrate_legacy_to_numeric(self, tmp_path):
        make_legacy_task(tmp_path, 42, "p2", "ready", "old-task")
        prefix = _prefix_for(tmp_path)
        result = fix(tmp_path)
        assert result.migrated == 1
        assert result.renamed == 1
        expected = f"{prefix}042-p2-ready--old-task.md"
        assert (tmp_path / expected).exists()
        assert not (tmp_path / "0042-p2-ready--old-task.md").exists()

    def test_migrate_alpha_prefix_to_numeric(self, tmp_path):
        """Alpha-prefix AANNN files are migrated to numeric DDNNN."""
        # Create a file with alpha prefix directly (bypassing make_task helper)
        p = tmp_path / "YF042-p2-ready--alpha-task.md"
        p.write_text("---\ncreated: 2026-03-04\npriority: p2\nstatus: ready\nartifact: src/alpha-task.py\n---\n\n# Task YF042\n")
        prefix = _prefix_for(tmp_path)
        result = fix(tmp_path)
        assert result.migrated == 1
        expected = f"{prefix}042-p2-ready--alpha-task.md"
        assert (tmp_path / expected).exists()
        assert not p.exists()

    def test_migrate_legacy_over_999_errors(self, tmp_path):
        make_legacy_task(tmp_path, 1000, "p2", "ready", "big-number")
        result = fix(tmp_path)
        assert not result.ok
        assert any("exceeds 999" in e for e in result.errors)

    def test_migrate_multiple_legacy_files(self, tmp_path):
        make_legacy_task(tmp_path, 1, "p2", "ready", "first")
        make_legacy_task(tmp_path, 2, "p1", "done", "second")
        prefix = _prefix_for(tmp_path)
        result = fix(tmp_path)
        assert result.migrated == 2
        assert (tmp_path / f"{prefix}001-p2-ready--first.md").exists()
        assert (tmp_path / f"{prefix}002-p1-done--second.md").exists()

    def test_fix_idempotent_after_migration(self, tmp_path):
        make_legacy_task(tmp_path, 1, "p2", "ready", "test")
        fix(tmp_path)
        result2 = fix(tmp_path)
        assert result2.patched == 0
        assert result2.renamed == 0
        assert result2.migrated == 0

    def test_renumber_two_duplicates(self, tmp_path):
        """End-to-end: two files with the same ID → fix renumbers one and
        reports the mapping; validate is clean afterwards."""
        import time

        prefix = _prefix_for(tmp_path)
        tid = f"{prefix}001"
        # Tiebreaker resolves on mtime when neither file is in git: the older
        # mtime wins. Sleep between writes so the order is deterministic.
        make_task(tmp_path, tid, "p2", "ready", "alpha")
        time.sleep(0.05)
        make_task(tmp_path, tid, "p1", "done", "beta")

        result = fix(tmp_path)
        assert result.ok, result.errors
        assert len(result.renumbered) == 1
        old_id, new_id, old_name, new_name = result.renumbered[0]
        assert old_id == tid
        assert new_id != tid
        # Same priority/status/slug as the loser — renumber only touches the ID.
        assert "-p1-done--beta.md" in new_name
        # Filesystem reflects the change.
        assert not (tmp_path / old_name).exists()
        assert (tmp_path / new_name).exists()
        # Validate is clean afterwards.
        assert validate(tmp_path).ok

    def test_renumber_idempotent(self, tmp_path):
        """Running fix twice after a renumber is a no-op for the second run."""
        import time

        prefix = _prefix_for(tmp_path)
        tid = f"{prefix}007"
        make_task(tmp_path, tid, "p2", "ready", "first")
        time.sleep(0.05)
        make_task(tmp_path, tid, "p2", "ready", "second")
        r1 = fix(tmp_path)
        assert len(r1.renumbered) == 1
        r2 = fix(tmp_path)
        assert r2.renumbered == []
        assert r2.renamed == 0

    def test_renumber_summary_reports_count(self, tmp_path):
        """FixResult.summary() mentions the renumber count when non-zero."""
        import time

        prefix = _prefix_for(tmp_path)
        tid = f"{prefix}050"
        make_task(tmp_path, tid, "p2", "ready", "a")
        time.sleep(0.05)
        make_task(tmp_path, tid, "p2", "ready", "b")
        result = fix(tmp_path)
        assert "renumbered" in result.summary().lower()


class TestValidateMentionsFix:
    """Validate's duplicate-ID error points callers at `taskmd fix`."""

    def test_duplicate_error_suggests_fix(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        make_task(tmp_path, f"{prefix}001", "p2", "ready", "alpha")
        make_task(tmp_path, f"{prefix}001", "p1", "done", "beta")
        result = validate(tmp_path)
        assert not result.ok
        dup_errors = [e for e in result.errors if "duplicate task id" in e]
        assert dup_errors, "expected at least one duplicate-id error"
        # Every duplicate-id error now points at the remediation.
        assert all("taskmd fix" in e for e in dup_errors), dup_errors


# ---------------------------------------------------------------------------
# next_id
# ---------------------------------------------------------------------------

class TestNextId:
    def test_empty_dir(self, tmp_path):
        result = next_id(tmp_path)
        prefix = _prefix_for(tmp_path)
        assert result == prefix + "001"

    def test_nonexistent_dir(self, tmp_path):
        result = next_id(tmp_path / "nope")
        # Should still return a valid ID
        assert len(result) == 5
        assert result.endswith("001")

    def test_with_tasks(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        make_task(tmp_path, prefix + "005", "p2", "ready", "a")
        make_task(tmp_path, prefix + "010", "p1", "done", "b")
        assert next_id(tmp_path) == prefix + "011"

    def test_with_gaps(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        make_task(tmp_path, prefix + "001", "p2", "ready", "a")
        make_task(tmp_path, prefix + "100", "p2", "ready", "b")
        assert next_id(tmp_path) == prefix + "101"  # max + 1, not fill gaps

    def test_ignores_legacy_files_when_allocating(self, tmp_path):
        """next_id is scoped to the local prefix; legacy files (empty prefix)
        do not inflate the local sequence. Any migration-time collision is
        resolved by `fix`, which bumps the migrated seq within the local
        prefix space until a free slot is found."""
        prefix = _prefix_for(tmp_path)
        make_legacy_task(tmp_path, 50, "p2", "ready", "old")
        assert next_id(tmp_path) == prefix + "001"

    def test_ignores_foreign_prefix_sequences(self, tmp_path):
        """next_id scopes to the local prefix so allocations in foreign
        partitions (different machine or worktree) don't inflate the local
        counter. Mirrors the Rust test `next_id_ignores_foreign_prefix_sequences`."""
        prefix = _prefix_for(tmp_path)
        # Foreign alpha-prefix task — should not count toward local prefix
        make_task(tmp_path, "ZQ500", "p2", "ready", "other")
        assert next_id(tmp_path) == prefix + "001"

    def test_different_dirs_yield_different_ids(self, tmp_path):
        a = tmp_path / "a"
        b = tmp_path / "b"
        a.mkdir()
        b.mkdir()
        id_a = next_id(a)
        id_b = next_id(b)
        # Different dirs may have same or different prefix (mod 10 collisions possible)
        # but both should end with 001
        assert id_a.endswith("001")
        assert id_b.endswith("001")


# ---------------------------------------------------------------------------
# init
# ---------------------------------------------------------------------------

class TestInit:
    def test_creates_dir_and_template(self, tmp_path):
        tasks_dir = tmp_path / "tasks"
        result = init(tasks_dir)
        assert result.ok
        assert tasks_dir.is_dir()
        assert (tasks_dir / "_TEMPLATE.md").exists()
        assert len(result.created) == 2
        assert result.template_fields == sorted(VALID_FIELDS)

    def test_template_has_frontmatter(self, tmp_path):
        tasks_dir = tmp_path / "tasks"
        init(tasks_dir)
        content = (tasks_dir / "_TEMPLATE.md").read_text()
        assert content.startswith("---\n")
        assert "status:" in content
        assert "priority:" in content
        assert "created:" in content
        assert "artifact:" in content

    def test_fails_if_dir_exists(self, tmp_path):
        tasks_dir = tmp_path / "tasks"
        tasks_dir.mkdir()
        result = init(tasks_dir)
        assert not result.ok
        assert "already exists" in result.error

    def test_custom_path(self, tmp_path):
        tasks_dir = tmp_path / "my-tasks"
        result = init(tasks_dir)
        assert result.ok
        assert tasks_dir.is_dir()
        assert (tasks_dir / "_TEMPLATE.md").exists()

    def test_nested_path_creates_parents(self, tmp_path):
        tasks_dir = tmp_path / "deep" / "nested" / "tasks"
        result = init(tasks_dir)
        assert result.ok
        assert tasks_dir.is_dir()


# ---------------------------------------------------------------------------
# create_task  (the atomic new-task primitive — Python-level smoke tests;
# exhaustive behaviour is covered in taskmd-core/src/create.rs)
# ---------------------------------------------------------------------------

class TestCreateTask:
    def test_creates_file_with_frontmatter_and_body(self, tmp_path):
        result = create_task(
            tmp_path, slug="fix-login", artifact="src/auth.py", body="Fix the bug."
        )
        assert result.path.exists()
        assert result.filename.endswith("-p2-ready--fix-login.md")
        content = result.path.read_text(encoding="utf-8")
        assert "priority: p2" in content
        assert "status: ready" in content
        assert "artifact: src/auth.py" in content
        assert "Fix the bug." in content

    def test_custom_body_is_used(self, tmp_path):
        result = create_task(
            tmp_path, slug="x", artifact="src/x.py", body="Custom body line."
        )
        assert "Custom body line." in result.path.read_text(encoding="utf-8")

    def test_priority_and_status_overrides(self, tmp_path):
        result = create_task(
            tmp_path,
            slug="x",
            artifact="src/x.py",
            priority="p0",
            status="in-progress",
            body="body",
        )
        assert "-p0-in-progress--x.md" in result.filename

    def test_dirty_slug_is_normalized(self, tmp_path):
        result = create_task(
            tmp_path, slug="Add OAuth2!", artifact="src/x.py", body="body"
        )
        assert "--add-oauth2.md" in result.filename

    def test_sequential_creates_are_monotonic(self, tmp_path):
        a = create_task(tmp_path, slug="a", artifact="src/a.py", body="body")
        b = create_task(tmp_path, slug="b", artifact="src/b.py", body="body")
        assert int(a.id[2:]) + 1 == int(b.id[2:])

    def test_result_file_validates_clean(self, tmp_path):
        create_task(tmp_path, slug="clean", artifact="src/clean.py", body="body")
        assert validate(tmp_path).ok

    def test_missing_tasks_dir_raises(self, tmp_path):
        missing = tmp_path / "nope"
        with pytest.raises(RuntimeError):
            create_task(missing, slug="x", artifact="src/x.py", body="body")

    def test_invalid_priority_raises(self, tmp_path):
        with pytest.raises(RuntimeError):
            create_task(
                tmp_path, slug="x", artifact="src/x.py", priority="p9", body="body"
            )

    def test_body_with_frontmatter_raises(self, tmp_path):
        with pytest.raises(RuntimeError):
            create_task(
                tmp_path,
                slug="x",
                artifact="src/x.py",
                body="---\nstatus: ready\n---\nbody",
            )

    def test_empty_artifact_raises(self, tmp_path):
        with pytest.raises(RuntimeError):
            create_task(tmp_path, slug="x", artifact="   ", body="body")

    def test_empty_body_raises(self, tmp_path):
        for body in ("", "   ", "\n\n", "\t\n"):
            with pytest.raises(RuntimeError):
                create_task(tmp_path, slug="x", artifact="src/x.py", body=body)
