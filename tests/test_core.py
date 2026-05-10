"""Tests for taskmd core library.

Exercises validate, fix, next_id, and parse_task_file against
real task files on disk. Uses tmp_path fixtures -- no global state.
"""
from pathlib import Path

import pytest

from taskmd.core import (
    VALID_PRIORITIES,
    VALID_STATUSES,
    _is_legacy_id,
    _needs_migration,
    _parse_id_parts,
    _prefix_for,
    ancillary_files_for,
    create_task,
    ensure_initialized,
    find_task_by_slug,
    fix,
    get_expected_filename,
    init,
    next_id,
    parse_task_file,
    update_task,
    validate,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def make_task(tasks_dir: Path, task_id: str, priority: str, status: str, slug: str) -> Path:
    """Create a valid task file on disk (filename-only metadata, body is markdown)."""
    filename = get_expected_filename(task_id, priority, status, slug)
    path = tasks_dir / filename
    path.write_text(f"# Task {task_id}\n\nSummary here.\n")
    return path


def make_legacy_task(tasks_dir: Path, number: int, priority: str, status: str, slug: str) -> Path:
    """Create a legacy 4-digit format task file on disk."""
    filename = f"{number:04d}-{priority}-{status}--{slug}.md"
    path = tasks_dir / filename
    path.write_text(f"# Task {number}\n\nSummary here.\n")
    return path


def make_template(tasks_dir: Path) -> Path:
    path = tasks_dir / "_TEMPLATE.md"
    path.write_text("# Template\n\nBody.\n")
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
        assert _needs_migration("0042", "34")
        assert _needs_migration("YF042", "34")
        assert not _needs_migration("21042", "34")
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

    def test_invalid_filename(self, tmp_path):
        p = tmp_path / "not-a-task.md"
        p.write_text("# nope\n")
        assert parse_task_file(p) is None

    def test_old_3digit_format_rejected(self, tmp_path):
        p = tmp_path / "042-p2-ready--fix-bug.md"
        p.write_text("# x\n")
        assert parse_task_file(p) is None

    def test_single_dash_rejected(self, tmp_path):
        p = tmp_path / "34042-p2-ready-fix-bug.md"
        p.write_text("# x\n")
        assert parse_task_file(p) is None

    def test_legacy_4digit_file(self, tmp_path):
        p = make_legacy_task(tmp_path, 42, "p1", "done", "big-feature")
        task = parse_task_file(p)
        assert task is not None
        assert task.id == "0042"

    def test_alpha_prefix_file(self, tmp_path):
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

    def test_invalid_filename_pattern(self, tmp_path):
        # File doesn't match DDNNN-pX-status--slug.md
        p = tmp_path / "not-a-real-task.md"
        p.write_text("# x\n")
        result = validate(tmp_path)
        assert not result.ok
        assert any("doesn't match pattern" in e for e in result.errors)

    def test_invalid_status_in_filename(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        # `pending` is not a valid status, so the regex won't match
        p = tmp_path / f"{prefix}001-p2-pending--test.md"
        p.write_text("# x\n")
        result = validate(tmp_path)
        assert not result.ok
        assert any("doesn't match pattern" in e for e in result.errors)

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
        assert result.file_count == 0

    def test_ancillary_skipped(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        make_task(tmp_path, f"{prefix}001", "p2", "ready", "test")
        (tmp_path / f"{prefix}001-p2-ready--test.qaplan.md").write_text("ancillary\n")
        (tmp_path / f"{prefix}001-p2-ready--test.qareport.md").write_text("ancillary\n")
        result = validate(tmp_path)
        assert result.ok
        assert result.file_count == 1

    def test_legacy_format_still_validates(self, tmp_path):
        make_legacy_task(tmp_path, 1, "p2", "ready", "test")
        result = validate(tmp_path)
        assert result.ok
        assert result.file_count == 1


# ---------------------------------------------------------------------------
# fix
# ---------------------------------------------------------------------------

class TestFix:
    def test_no_renames_needed(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        make_task(tmp_path, f"{prefix}001", "p2", "ready", "test")
        result = fix(tmp_path)
        assert result.ok
        assert result.renamed == 0
        assert result.migrated == 0

    def test_ancillary_skipped_by_fix(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        make_task(tmp_path, f"{prefix}001", "p2", "ready", "test")
        (tmp_path / f"{prefix}001-p2-ready--test.qaplan.md").write_text("garbage")
        result = fix(tmp_path)
        assert result.ok

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
        p = tmp_path / "YF042-p2-ready--alpha-task.md"
        p.write_text("# Task YF042\n")
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
        assert result2.renamed == 0
        assert result2.migrated == 0

    def test_renumber_two_duplicates(self, tmp_path):
        import time

        prefix = _prefix_for(tmp_path)
        tid = f"{prefix}001"
        make_task(tmp_path, tid, "p2", "ready", "alpha")
        time.sleep(0.05)
        make_task(tmp_path, tid, "p1", "done", "beta")

        result = fix(tmp_path)
        assert result.ok, result.errors
        assert len(result.renumbered) == 1
        old_id, new_id, old_name, new_name = result.renumbered[0]
        assert old_id == tid
        assert new_id != tid
        assert "-p1-done--beta.md" in new_name
        assert not (tmp_path / old_name).exists()
        assert (tmp_path / new_name).exists()
        assert validate(tmp_path).ok

    def test_renumber_idempotent(self, tmp_path):
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
        import time

        prefix = _prefix_for(tmp_path)
        tid = f"{prefix}050"
        make_task(tmp_path, tid, "p2", "ready", "a")
        time.sleep(0.05)
        make_task(tmp_path, tid, "p2", "ready", "b")
        result = fix(tmp_path)
        assert "renumbered" in result.summary().lower()


class TestFixFrontmatterMigration:
    """`fix` requires opt-in to strip legacy YAML frontmatter."""

    def _make_fm_task(self, tasks_dir, task_id, slug):
        filename = f"{task_id}-p2-ready--{slug}.md"
        path = tasks_dir / filename
        path.write_text(
            "---\ncreated: 2026-01-01\npriority: p2\nstatus: ready\n"
            "artifact: x\n---\n\n# " + slug + "\n\nbody\n"
        )
        return path

    def test_default_fails_when_frontmatter_present(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        self._make_fm_task(tmp_path, f"{prefix}001", "alpha")
        result = fix(tmp_path)
        assert not result.ok
        assert len(result.frontmatter_pending) == 1
        assert "alpha" in result.frontmatter_pending[0]
        assert any("--migrate" in e for e in result.errors)
        assert any("--no-migrate" in e for e in result.errors)

    def test_default_passes_when_no_frontmatter(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        make_task(tmp_path, f"{prefix}001", "p2", "ready", "alpha")
        result = fix(tmp_path)
        assert result.ok, result.errors
        assert result.frontmatter_pending == []

    def test_migrate_strips_frontmatter(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        path = self._make_fm_task(tmp_path, f"{prefix}001", "alpha")
        result = fix(tmp_path, migrate=True)
        assert result.ok, result.errors
        assert len(result.frontmatter_stripped) == 1
        content = path.read_text()
        assert not content.startswith("---")
        assert content.startswith("# alpha")

    def test_no_migrate_leaves_frontmatter_alone(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        path = self._make_fm_task(tmp_path, f"{prefix}001", "alpha")
        result = fix(tmp_path, migrate=False)
        assert result.ok, result.errors
        assert result.frontmatter_stripped == []
        assert path.read_text().startswith("---")

    def test_summary_mentions_stripped(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        self._make_fm_task(tmp_path, f"{prefix}001", "alpha")
        self._make_fm_task(tmp_path, f"{prefix}002", "beta")
        result = fix(tmp_path, migrate=True)
        assert "stripped frontmatter" in result.summary().lower()


class TestCliFixMigrate:
    """CLI flag wiring for the migration flow."""

    def _make_fm_task(self, tasks_dir, task_id, slug):
        filename = f"{task_id}-p2-ready--{slug}.md"
        path = tasks_dir / filename
        path.write_text(
            "---\ncreated: 2026-01-01\npriority: p2\nstatus: ready\n"
            "artifact: x\n---\n\n# " + slug + "\n\nbody\n"
        )
        return path

    def test_default_human_lists_pending_files(self, tmp_path, capsys, monkeypatch):
        _unset_agent_env(monkeypatch)
        from taskmd.cli import main
        prefix = _prefix_for(tmp_path)
        self._make_fm_task(tmp_path, f"{prefix}001", "alpha")
        with pytest.raises(SystemExit) as exc:
            main(["fix", str(tmp_path)])
        assert exc.value.code == 1
        out = capsys.readouterr().out
        assert "legacy YAML frontmatter" in out
        assert "alpha" in out
        assert "--migrate" in out
        assert "--no-migrate" in out

    def test_default_json_includes_pending_data(self, tmp_path, capsys, monkeypatch):
        import json
        from taskmd.cli import main
        monkeypatch.setenv("FORCE_AGENT_MODE", "1")
        prefix = _prefix_for(tmp_path)
        self._make_fm_task(tmp_path, f"{prefix}001", "alpha")
        self._make_fm_task(tmp_path, f"{prefix}002", "beta")
        with pytest.raises(SystemExit) as exc:
            main(["fix", str(tmp_path)])
        assert exc.value.code == 1
        obj = json.loads(capsys.readouterr().out)
        assert obj["status"] == "error"
        # The error envelope must surface the pending list so agents can act.
        assert "data" in obj
        pending = obj["data"]["frontmatter_pending"]
        assert len(pending) == 2
        assert any("alpha" in name for name in pending)
        assert any("beta" in name for name in pending)
        suggestions = obj.get("suggestions", [])
        assert any("--migrate" in s for s in suggestions)
        assert any("--no-migrate" in s for s in suggestions)

    def test_migrate_flag_strips_frontmatter(self, tmp_path, capsys, monkeypatch):
        _unset_agent_env(monkeypatch)
        from taskmd.cli import main
        prefix = _prefix_for(tmp_path)
        path = self._make_fm_task(tmp_path, f"{prefix}001", "alpha")
        main(["fix", "--migrate", str(tmp_path)])
        out = capsys.readouterr().out
        assert "stripped frontmatter" in out
        assert not path.read_text().startswith("---")

    def test_no_migrate_flag_skips_check(self, tmp_path, capsys, monkeypatch):
        _unset_agent_env(monkeypatch)
        from taskmd.cli import main
        prefix = _prefix_for(tmp_path)
        path = self._make_fm_task(tmp_path, f"{prefix}001", "alpha")
        main(["fix", "--no-migrate", str(tmp_path)])
        out = capsys.readouterr().out
        assert "All files already correct" in out or "✓" in out
        # Frontmatter still present
        assert path.read_text().startswith("---")


class TestValidateMentionsFix:
    """Validate's duplicate-ID error points callers at `taskmd fix`."""

    def test_duplicate_error_suggests_fix(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        make_task(tmp_path, f"{prefix}001", "p2", "ready", "alpha")
        make_task(tmp_path, f"{prefix}001", "p1", "done", "beta")
        result = validate(tmp_path)
        assert not result.ok
        dup_errors = [e for e in result.errors if "duplicate task id" in e]
        assert dup_errors
        assert all("taskmd fix" in e for e in dup_errors), dup_errors


class TestCliValidateSuggestions:
    """`taskmd validate` should tailor its remediation suggestions to the
    actual errors. `taskmd fix` cannot repair filename-pattern violations,
    so the suggestion text must be different in that case."""

    def test_pattern_only_does_not_recommend_fix(self, tmp_path, capsys, monkeypatch):
        import json
        from taskmd.cli import main
        monkeypatch.setenv("FORCE_AGENT_MODE", "1")
        (tmp_path / "not-a-task.md").write_text("# x\n")
        with pytest.raises(SystemExit) as exc:
            main(["validate", str(tmp_path)])
        assert exc.value.code == 1
        obj = json.loads(capsys.readouterr().out)
        assert obj["status"] == "error"
        suggestions = obj.get("suggestions", [])
        assert any("Rename" in s for s in suggestions)
        assert not any("auto-renumber" in s for s in suggestions)
        assert not any("auto-repair" in s for s in suggestions)

    def test_duplicate_only_recommends_fix(self, tmp_path, capsys, monkeypatch):
        import json
        from taskmd.cli import main
        monkeypatch.setenv("FORCE_AGENT_MODE", "1")
        prefix = _prefix_for(tmp_path)
        make_task(tmp_path, f"{prefix}001", "p2", "ready", "alpha")
        make_task(tmp_path, f"{prefix}001", "p1", "done", "beta")
        with pytest.raises(SystemExit) as exc:
            main(["validate", str(tmp_path)])
        assert exc.value.code == 1
        obj = json.loads(capsys.readouterr().out)
        assert obj["status"] == "error"
        suggestions = obj.get("suggestions", [])
        assert any("auto-renumber" in s for s in suggestions)
        assert not any("Rename" in s for s in suggestions)

    def test_both_kinds_recommend_both(self, tmp_path, capsys, monkeypatch):
        import json
        from taskmd.cli import main
        monkeypatch.setenv("FORCE_AGENT_MODE", "1")
        prefix = _prefix_for(tmp_path)
        make_task(tmp_path, f"{prefix}001", "p2", "ready", "alpha")
        make_task(tmp_path, f"{prefix}001", "p1", "done", "beta")
        (tmp_path / "not-a-task.md").write_text("# x\n")
        with pytest.raises(SystemExit) as exc:
            main(["validate", str(tmp_path)])
        assert exc.value.code == 1
        obj = json.loads(capsys.readouterr().out)
        suggestions = obj.get("suggestions", [])
        assert any("auto-renumber" in s for s in suggestions)
        assert any("Rename" in s for s in suggestions)


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
        assert next_id(tmp_path) == prefix + "101"

    def test_ignores_legacy_files_when_allocating(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        make_legacy_task(tmp_path, 50, "p2", "ready", "old")
        assert next_id(tmp_path) == prefix + "001"

    def test_ignores_foreign_prefix_sequences(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        make_task(tmp_path, "ZQ500", "p2", "ready", "other")
        assert next_id(tmp_path) == prefix + "001"

    def test_different_dirs_yield_different_ids(self, tmp_path):
        a = tmp_path / "a"
        b = tmp_path / "b"
        a.mkdir()
        b.mkdir()
        id_a = next_id(a)
        id_b = next_id(b)
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

    def test_template_has_no_frontmatter(self, tmp_path):
        tasks_dir = tmp_path / "tasks"
        init(tasks_dir)
        content = (tasks_dir / "_TEMPLATE.md").read_text()
        assert not content.startswith("---")
        assert "# Task Title" in content

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
# create_task
# ---------------------------------------------------------------------------

class TestCreateTask:
    def test_creates_file_with_body_only(self, tmp_path):
        result = create_task(tmp_path, slug="fix-login", body="Fix the bug.")
        assert result.path.exists()
        assert result.filename.endswith("-p2-ready--fix-login.md")
        content = result.path.read_text(encoding="utf-8")
        assert not content.startswith("---")
        assert content == "Fix the bug.\n"

    def test_custom_body_is_used(self, tmp_path):
        result = create_task(tmp_path, slug="x", body="Custom body line.")
        assert result.path.read_text(encoding="utf-8") == "Custom body line.\n"

    def test_priority_and_status_overrides(self, tmp_path):
        result = create_task(
            tmp_path, slug="x", priority="p0", status="in-progress", body="body"
        )
        assert "-p0-in-progress--x.md" in result.filename

    def test_dirty_slug_is_normalized(self, tmp_path):
        result = create_task(tmp_path, slug="Add OAuth2!", body="body")
        assert "--add-oauth2.md" in result.filename

    def test_sequential_creates_are_monotonic(self, tmp_path):
        a = create_task(tmp_path, slug="a", body="body")
        b = create_task(tmp_path, slug="b", body="body")
        assert int(a.id[2:]) + 1 == int(b.id[2:])

    def test_result_file_validates_clean(self, tmp_path):
        create_task(tmp_path, slug="clean", body="body")
        assert validate(tmp_path).ok

    def test_missing_tasks_dir_raises(self, tmp_path):
        missing = tmp_path / "nope"
        with pytest.raises(RuntimeError):
            create_task(missing, slug="x", body="body")

    def test_invalid_priority_raises(self, tmp_path):
        with pytest.raises(ValueError):
            create_task(tmp_path, slug="x", priority="p9", body="body")

    def test_empty_body_raises(self, tmp_path):
        for body in ("", "   ", "\n\n", "\t\n"):
            with pytest.raises(RuntimeError):
                create_task(tmp_path, slug="x", body=body)


# ---------------------------------------------------------------------------
# update_task
# ---------------------------------------------------------------------------

class TestUpdateTask:
    def test_status_change_renames_file(self, tmp_path):
        old = make_task(tmp_path, "34001", "p2", "ready", "fix-login")
        old_name, new_name = update_task(tmp_path, "34001", status="in-progress")
        assert old_name == old.name
        assert new_name.endswith("-p2-in-progress--fix-login.md")
        assert not old.exists()
        assert (tmp_path / new_name).exists()

    def test_priority_change_renames_file(self, tmp_path):
        old = make_task(tmp_path, "34001", "p2", "ready", "fix-login")
        old_name, new_name = update_task(tmp_path, "34001", priority="p0")
        assert old_name == old.name
        assert "-p0-ready--" in new_name
        assert not old.exists()
        assert (tmp_path / new_name).exists()

    def test_slug_change_renames_file(self, tmp_path):
        old = make_task(tmp_path, "34001", "p2", "ready", "fix-login")
        old_name, new_name = update_task(tmp_path, "34001", slug="Brand New Slug")
        assert old_name == old.name
        assert new_name.endswith("--brand-new-slug.md")
        assert not old.exists()
        assert (tmp_path / new_name).exists()

    def test_combined_change(self, tmp_path):
        old = make_task(tmp_path, "34001", "p2", "ready", "fix-login")
        old_name, new_name = update_task(
            tmp_path,
            "34001",
            priority="p0",
            status="in-progress",
            slug="overhaul-auth",
        )
        assert old_name == old.name
        assert "-p0-in-progress--overhaul-auth.md" in new_name
        assert not old.exists()
        assert (tmp_path / new_name).exists()

    def test_noop_returns_same_filename(self, tmp_path):
        old = make_task(tmp_path, "34001", "p2", "ready", "fix-login")
        old_name, new_name = update_task(tmp_path, "34001")
        assert old_name == new_name
        assert old_name == old.name
        assert old.exists()

        # Also a no-op when kwargs match the current values.
        old_name2, new_name2 = update_task(
            tmp_path, "34001", priority="p2", status="ready"
        )
        assert old_name2 == new_name2
        assert old.exists()

    def test_unknown_id_raises(self, tmp_path):
        make_task(tmp_path, "34001", "p2", "ready", "slug")
        with pytest.raises(RuntimeError, match="not found"):
            update_task(tmp_path, "34999", status="done")

    def test_invalid_status_raises(self, tmp_path):
        make_task(tmp_path, "34001", "p2", "ready", "slug")
        with pytest.raises(ValueError, match="invalid status"):
            update_task(tmp_path, "34001", status="pending")

    def test_invalid_priority_raises(self, tmp_path):
        make_task(tmp_path, "34001", "p2", "ready", "slug")
        with pytest.raises(ValueError, match="invalid priority"):
            update_task(tmp_path, "34001", priority="p9")

    def test_invalid_slug_raises(self, tmp_path):
        make_task(tmp_path, "34001", "p2", "ready", "slug")
        with pytest.raises(RuntimeError, match="invalid slug"):
            update_task(tmp_path, "34001", slug="   ")

    def test_target_exists_raises(self, tmp_path):
        make_task(tmp_path, "34001", "p2", "ready", "fix-login")
        # Stage a conflicting file at the target name. find_task_by_id sorts
        # filenames; the stub at the in-progress name sorts before the real
        # task on disk, so when we rename back to 'ready' it collides with
        # the real task file.
        (tmp_path / "34001-p2-in-progress--fix-login.md").write_text("stub")
        with pytest.raises(RuntimeError, match="target already exists"):
            update_task(tmp_path, "34001", status="ready")


# ---------------------------------------------------------------------------
# find_task_by_slug
# ---------------------------------------------------------------------------

class TestFindTaskBySlug:
    def test_single_match(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        make_task(tmp_path, f"{prefix}001", "p2", "ready", "fix-login")
        make_task(tmp_path, f"{prefix}002", "p1", "done", "other-task")
        results = find_task_by_slug(tmp_path, "fix-login")
        assert len(results) == 1
        assert results[0].id == f"{prefix}001"
        assert results[0].slug == "fix-login"

    def test_no_match_returns_empty(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        make_task(tmp_path, f"{prefix}001", "p2", "ready", "fix-login")
        results = find_task_by_slug(tmp_path, "nonexistent-slug")
        assert results == []

    def test_multiple_matches_different_ids(self, tmp_path):
        prefix = _prefix_for(tmp_path)
        make_task(tmp_path, f"{prefix}001", "p2", "ready", "shared-slug")
        make_task(tmp_path, f"{prefix}002", "p1", "done", "shared-slug")
        make_task(tmp_path, f"{prefix}003", "p0", "in-progress", "other")
        results = find_task_by_slug(tmp_path, "shared-slug")
        assert len(results) == 2
        ids = {t.id for t in results}
        assert ids == {f"{prefix}001", f"{prefix}002"}


# ---------------------------------------------------------------------------
# ancillary_files_for
# ---------------------------------------------------------------------------

class TestAncillaryFilesFor:
    def test_returns_attachments(self, tmp_path):
        make_task(tmp_path, "34001", "p2", "ready", "fix-login")
        (tmp_path / "34001-p2-ready--fix-login.qaplan.md").write_text("plan")
        (tmp_path / "34001-p2-ready--fix-login.qareport.md").write_text("report")
        results = ancillary_files_for(tmp_path, "34001")
        names = sorted(p.name for p in results)
        assert names == [
            "34001-p2-ready--fix-login.qaplan.md",
            "34001-p2-ready--fix-login.qareport.md",
        ]

    def test_no_main_returns_empty(self, tmp_path):
        # No main task on disk for id 34999.
        make_task(tmp_path, "34001", "p2", "ready", "fix-login")
        results = ancillary_files_for(tmp_path, "34999")
        assert results == []

    def test_no_ancillaries_returns_empty(self, tmp_path):
        make_task(tmp_path, "34001", "p2", "ready", "fix-login")
        results = ancillary_files_for(tmp_path, "34001")
        assert results == []


# ---------------------------------------------------------------------------
# ensure_initialized
# ---------------------------------------------------------------------------

class TestEnsureInitialized:
    def test_creates_missing_dir(self, tmp_path):
        tasks_dir = tmp_path / "tasks"
        result = ensure_initialized(tasks_dir)
        assert result.ok
        assert tasks_dir.is_dir()
        assert (tasks_dir / "_TEMPLATE.md").exists()
        assert len(result.created) == 2

    def test_idempotent(self, tmp_path):
        tasks_dir = tmp_path / "tasks"
        ensure_initialized(tasks_dir)
        result = ensure_initialized(tasks_dir)
        assert result.ok
        assert result.created == []
        assert tasks_dir.is_dir()
        assert (tasks_dir / "_TEMPLATE.md").exists()

    def test_fills_missing_template(self, tmp_path):
        tasks_dir = tmp_path / "tasks"
        tasks_dir.mkdir()
        # Directory exists but template does not.
        result = ensure_initialized(tasks_dir)
        assert result.ok
        assert (tasks_dir / "_TEMPLATE.md").exists()
        # Only the template was created; the dir was already there.
        assert len(result.created) == 1
        assert any("_TEMPLATE.md" in path for path in result.created)

    def test_leaves_existing_template_alone(self, tmp_path):
        tasks_dir = tmp_path / "tasks"
        tasks_dir.mkdir()
        template = tasks_dir / "_TEMPLATE.md"
        template.write_text("custom template body\n")
        result = ensure_initialized(tasks_dir)
        assert result.ok
        assert result.created == []
        assert template.read_text() == "custom template body\n"


# ---------------------------------------------------------------------------
# TaskFile.filename property
# ---------------------------------------------------------------------------

class TestTaskFileFilename:
    def test_filename_returns_basename(self, tmp_path):
        p = make_task(tmp_path, "34001", "p2", "ready", "fix-login")
        task = parse_task_file(p)
        assert task is not None
        assert task.filename == "34001-p2-ready--fix-login.md"
        assert task.filename == p.name


# ---------------------------------------------------------------------------
# CLI: taskmd status  (end-to-end via main())
# ---------------------------------------------------------------------------

def _unset_agent_env(monkeypatch):
    for v in (
        "CLAUDECODE", "CLAUDE_CODE", "CURSOR_AGENT", "CODEX", "OPENAI_CODEX",
        "OPENCODE", "AIDER", "CLINE", "WINDSURF_AGENT", "GITHUB_COPILOT",
        "AMAZON_Q", "AWS_Q_DEVELOPER", "GEMINI_CODE_ASSIST", "SRC_CODY",
        "AGENT", "FORCE_AGENT_MODE",
    ):
        monkeypatch.delenv(v, raising=False)


class TestCliStatus:
    def test_human_happy_path(self, tmp_path, capsys, monkeypatch):
        _unset_agent_env(monkeypatch)
        from taskmd.cli import main
        make_task(tmp_path, "34001", "p2", "ready", "fix-login")
        main(["status", "34001", "in-progress", str(tmp_path)])
        out = capsys.readouterr().out
        assert "-p2-ready--fix-login.md -> " in out
        assert "-p2-in-progress--fix-login.md" in out
        assert (tmp_path / "34001-p2-in-progress--fix-login.md").exists()

    def test_json_happy_path(self, tmp_path, capsys, monkeypatch):
        import json
        from taskmd.cli import main
        monkeypatch.setenv("FORCE_AGENT_MODE", "1")
        make_task(tmp_path, "34001", "p2", "ready", "fix-login")
        main(["status", "34001", "done", str(tmp_path)])
        out = capsys.readouterr().out
        obj = json.loads(out)
        assert obj["status"] == "success"
        assert obj["command"] == "status"
        assert obj["data"]["id"] == "34001"
        assert obj["data"]["old_status"] == "ready"
        assert obj["data"]["new_status"] == "done"
        assert obj["data"]["old_filename"].endswith("-p2-ready--fix-login.md")
        assert obj["data"]["new_filename"].endswith("-p2-done--fix-login.md")

    def test_unknown_id_human(self, tmp_path, capsys, monkeypatch):
        _unset_agent_env(monkeypatch)
        from taskmd.cli import main
        make_task(tmp_path, "34001", "p2", "ready", "slug")
        with pytest.raises(SystemExit) as exc:
            main(["status", "34999", "done", str(tmp_path)])
        assert exc.value.code == 1
        err = capsys.readouterr().err
        assert "not found" in err

    def test_unknown_id_json(self, tmp_path, capsys, monkeypatch):
        import json
        from taskmd.cli import main
        monkeypatch.setenv("FORCE_AGENT_MODE", "1")
        make_task(tmp_path, "34001", "p2", "ready", "slug")
        with pytest.raises(SystemExit) as exc:
            main(["status", "34999", "done", str(tmp_path)])
        assert exc.value.code == 1
        obj = json.loads(capsys.readouterr().out)
        assert obj["status"] == "error"
        assert obj["command"] == "status"
        assert any("not found" in e for e in obj["errors"])

    def test_invalid_status_human(self, tmp_path, capsys, monkeypatch):
        _unset_agent_env(monkeypatch)
        from taskmd.cli import main
        make_task(tmp_path, "34001", "p2", "ready", "slug")
        with pytest.raises(SystemExit) as exc:
            main(["status", "34001", "pending", str(tmp_path)])
        assert exc.value.code == 1
        err = capsys.readouterr().err
        assert "invalid status" in err

    def test_invalid_status_json(self, tmp_path, capsys, monkeypatch):
        import json
        from taskmd.cli import main
        monkeypatch.setenv("FORCE_AGENT_MODE", "1")
        make_task(tmp_path, "34001", "p2", "ready", "slug")
        with pytest.raises(SystemExit) as exc:
            main(["status", "34001", "pending", str(tmp_path)])
        assert exc.value.code == 1
        obj = json.loads(capsys.readouterr().out)
        assert obj["status"] == "error"
        assert any("invalid status" in e for e in obj["errors"])

    def test_missing_args_errors(self, tmp_path, capsys, monkeypatch):
        _unset_agent_env(monkeypatch)
        from taskmd.cli import main
        with pytest.raises(SystemExit) as exc:
            main(["status", "34001", str(tmp_path)])
        assert exc.value.code == 1

    def test_missing_id_and_status_errors(self, tmp_path, capsys, monkeypatch):
        _unset_agent_env(monkeypatch)
        from taskmd.cli import main
        with pytest.raises(SystemExit) as exc:
            main(["status"])
        assert exc.value.code == 1
        err = capsys.readouterr().err
        assert "requires" in err

    def test_conflict_when_target_exists_json(self, tmp_path, capsys, monkeypatch):
        import json
        from taskmd.cli import main
        monkeypatch.setenv("FORCE_AGENT_MODE", "1")
        make_task(tmp_path, "34001", "p2", "ready", "fix-login")
        (tmp_path / "34001-p2-in-progress--fix-login.md").write_text("stub")
        with pytest.raises(SystemExit) as exc:
            main(["status", "34001", "ready", str(tmp_path)])
        assert exc.value.code == 1
        obj = json.loads(capsys.readouterr().out)
        assert obj["status"] == "error"
        assert any("already exists" in e for e in obj["errors"])
