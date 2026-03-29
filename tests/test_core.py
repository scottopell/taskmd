"""Tests for taskmd core library.

Exercises validate, fix, next_number, and parse_task_file against
real task files on disk. Uses tmp_path fixtures — no global state.
"""
import os
from pathlib import Path

import pytest

from taskmd.core import (
    VALID_FIELDS,
    VALID_PRIORITIES,
    VALID_STATUSES,
    ValidationResult,
    fix,
    get_expected_filename,
    init,
    next_number,
    parse_task_file,
    validate,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def make_task(tasks_dir: Path, number: int, priority: str, status: str, slug: str, **extra_fields) -> Path:
    """Create a valid task file on disk."""
    fields = {"created": "2026-03-04", "priority": priority, "status": status, "artifact": f"src/{slug}.py"}
    fields.update(extra_fields)
    fm = "\n".join(f"{k}: {v}" for k, v in fields.items())
    filename = get_expected_filename(number, priority, status, slug)
    path = tasks_dir / filename
    path.write_text(f"---\n{fm}\n---\n\n# Task {number}\n\nSummary here.\n")
    return path


def make_template(tasks_dir: Path) -> Path:
    path = tasks_dir / "_TEMPLATE.md"
    path.write_text("---\ncreated: YYYY-MM-DD\npriority: p2\nstatus: ready\n---\n\n# Template\n")
    return path


# ---------------------------------------------------------------------------
# parse_task_file
# ---------------------------------------------------------------------------

class TestParseTaskFile:
    def test_valid_file(self, tmp_path):
        p = make_task(tmp_path, 42, "p2", "ready", "fix-bug")
        task = parse_task_file(p)
        assert task is not None
        assert task.number == 42
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
        p = tmp_path / "0042-p2-ready-fix-bug.md"  # single dash before slug
        p.write_text("---\ncreated: 2026-03-04\npriority: p2\nstatus: ready\n---\n")
        assert parse_task_file(p) is None

    def test_4digit_file(self, tmp_path):
        p = make_task(tmp_path, 1234, "p1", "done", "big-feature")
        task = parse_task_file(p)
        assert task is not None
        assert task.number == 1234

    def test_all_statuses_parse(self, tmp_path):
        for i, status in enumerate(sorted(VALID_STATUSES), start=1):
            p = make_task(tmp_path, i, "p2", status, f"test-{status}")
            task = parse_task_file(p)
            assert task is not None, f"Failed to parse status: {status}"
            assert task.status == status

    def test_frontmatter_with_colon_in_value(self, tmp_path):
        p = make_task(tmp_path, 1, "p2", "ready", "test")
        content = p.read_text()
        content = content.replace("---\n\n#", 'title: "QA Report: Streaming"\n---\n\n#')
        p.write_text(content)
        # Re-read — partition should handle the colon
        task = parse_task_file(p)
        assert task is not None


# ---------------------------------------------------------------------------
# get_expected_filename
# ---------------------------------------------------------------------------

class TestGetExpectedFilename:
    def test_basic(self):
        assert get_expected_filename(42, "p2", "ready", "fix-bug") == "0042-p2-ready--fix-bug.md"

    def test_4digit(self):
        assert get_expected_filename(1234, "p0", "done", "big") == "1234-p0-done--big.md"

    def test_zero_padded(self):
        assert get_expected_filename(1, "p4", "brainstorming", "idea") == "0001-p4-brainstorming--idea.md"


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
        make_template(tmp_path)
        make_task(tmp_path, 1, "p2", "ready", "first")
        make_task(tmp_path, 2, "p1", "done", "second")
        result = validate(tmp_path)
        assert result.ok
        assert result.file_count == 2

    def test_missing_frontmatter(self, tmp_path):
        p = tmp_path / "0001-p2-ready--no-fm.md"
        p.write_text("# No frontmatter\n")
        result = validate(tmp_path)
        assert not result.ok
        assert any("missing YAML frontmatter" in e for e in result.errors)

    def test_missing_status(self, tmp_path):
        p = tmp_path / "0001-p2-ready--test.md"
        p.write_text("---\ncreated: 2026-03-04\npriority: p2\n---\n")
        result = validate(tmp_path)
        assert not result.ok
        assert any("missing 'status'" in e for e in result.errors)

    def test_missing_priority(self, tmp_path):
        p = tmp_path / "0001-p2-ready--test.md"
        p.write_text("---\ncreated: 2026-03-04\nstatus: ready\n---\n")
        result = validate(tmp_path)
        assert not result.ok
        assert any("missing 'priority'" in e for e in result.errors)

    def test_missing_created(self, tmp_path):
        p = tmp_path / "0001-p2-ready--test.md"
        p.write_text("---\npriority: p2\nstatus: ready\n---\n")
        result = validate(tmp_path)
        assert not result.ok
        assert any("missing 'created'" in e for e in result.errors)

    def test_invalid_status(self, tmp_path):
        p = tmp_path / "0001-p2-ready--test.md"
        p.write_text("---\ncreated: 2026-03-04\npriority: p2\nstatus: pending\n---\n")
        result = validate(tmp_path)
        assert not result.ok
        assert any("invalid status" in e for e in result.errors)

    def test_filename_mismatch(self, tmp_path):
        # Frontmatter says done, filename says ready
        p = tmp_path / "0001-p2-ready--test.md"
        p.write_text("---\ncreated: 2026-03-04\npriority: p2\nstatus: done\n---\n")
        result = validate(tmp_path)
        assert not result.ok
        assert any("doesn't match frontmatter" in e for e in result.errors)

    def test_duplicate_numbers(self, tmp_path):
        make_task(tmp_path, 1, "p2", "ready", "first")
        make_task(tmp_path, 1, "p1", "done", "second")
        result = validate(tmp_path)
        assert not result.ok
        assert any("duplicate task number" in e for e in result.errors)

    def test_template_skipped(self, tmp_path):
        make_template(tmp_path)
        result = validate(tmp_path)
        assert result.ok
        assert result.file_count == 0  # template not counted

    def test_ancillary_skipped(self, tmp_path):
        make_task(tmp_path, 1, "p2", "ready", "test")
        # Create ancillary files
        (tmp_path / "0001-p2-ready--test.qaplan.md").write_text("---\ncreated: 2026-03-04\n---\n")
        (tmp_path / "0001-p2-ready--test.qareport.md").write_text("---\ncreated: 2026-03-04\n---\n")
        result = validate(tmp_path)
        assert result.ok
        assert result.file_count == 1  # only the main task


# ---------------------------------------------------------------------------
# fix
# ---------------------------------------------------------------------------

class TestFix:
    def test_inject_missing_created(self, tmp_path):
        p = tmp_path / "0001-p2-ready--test.md"
        p.write_text("---\npriority: p2\nstatus: ready\n---\n\n# Test\n")
        result = fix(tmp_path)
        assert result.patched == 1
        content = p.read_text()
        assert "created:" in content

    def test_replace_malformed_created(self, tmp_path):
        p = tmp_path / "0001-p2-ready--test.md"
        p.write_text("---\ncreated: YYYY-MM-DD\npriority: p2\nstatus: ready\n---\n")
        fix(tmp_path)
        content = p.read_text()
        assert "YYYY-MM-DD" not in content
        assert "created:" in content
        # Run again — should be idempotent
        result2 = fix(tmp_path)
        assert result2.patched == 0

    def test_rename_to_match_frontmatter(self, tmp_path):
        p = tmp_path / "0001-p2-ready--old-name.md"
        p.write_text("---\ncreated: 2026-03-04\npriority: p1\nstatus: done\n---\n")
        result = fix(tmp_path)
        assert result.renamed == 1
        assert (tmp_path / "0001-p1-done--old-name.md").exists()
        assert not p.exists()

    def test_rename_conflict(self, tmp_path):
        make_task(tmp_path, 1, "p1", "done", "target")
        p = tmp_path / "0001-p2-ready--target.md"
        p.write_text("---\ncreated: 2026-03-04\npriority: p1\nstatus: done\n---\n")
        result = fix(tmp_path)
        assert any("cannot rename" in e for e in result.errors)

    def test_ancillary_skipped_by_fix(self, tmp_path):
        make_task(tmp_path, 1, "p2", "ready", "test")
        (tmp_path / "0001-p2-ready--test.qaplan.md").write_text("garbage")
        result = fix(tmp_path)
        assert result.ok  # no errors from the qaplan file


# ---------------------------------------------------------------------------
# next_number
# ---------------------------------------------------------------------------

class TestNextNumber:
    def test_empty_dir(self, tmp_path):
        assert next_number(tmp_path) == 1

    def test_nonexistent_dir(self, tmp_path):
        assert next_number(tmp_path / "nope") == 1

    def test_with_tasks(self, tmp_path):
        make_task(tmp_path, 5, "p2", "ready", "a")
        make_task(tmp_path, 10, "p1", "done", "b")
        assert next_number(tmp_path) == 11

    def test_with_gaps(self, tmp_path):
        make_task(tmp_path, 1, "p2", "ready", "a")
        make_task(tmp_path, 100, "p2", "ready", "b")
        assert next_number(tmp_path) == 101  # max + 1, not fill gaps


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
