"""Property-based tests for taskmd core library.

Uses Hypothesis to verify invariants across round-trips, structural
constraints, idempotency, and filesystem operations.
"""
from __future__ import annotations

import re
import tempfile
from pathlib import Path

from hypothesis import given, settings, assume, HealthCheck
from hypothesis import strategies as st

from taskmd.core import (
    VALID_PRIORITIES,
    VALID_STATUSES,
    _prefix_for,
    fix,
    get_expected_filename,
    next_id,
    parse_task_file,
    validate,
)


# ---------------------------------------------------------------------------
# Reusable strategies
# ---------------------------------------------------------------------------

def task_ids():
    prefix = st.from_regex(r"[0-9]{2}", fullmatch=True)
    seq = st.integers(min_value=1, max_value=990)
    return st.tuples(prefix, seq).map(lambda ps: ps[0] + f"{ps[1]:03d}")


def priorities():
    return st.sampled_from(sorted(VALID_PRIORITIES))


def statuses():
    return st.sampled_from(sorted(VALID_STATUSES))


def slugs():
    word = st.from_regex(r"[a-z][a-z0-9]{0,8}", fullmatch=True)
    return st.lists(word, min_size=1, max_size=5).map(lambda parts: "-".join(parts))


def valid_task_params():
    return st.tuples(task_ids(), priorities(), statuses(), slugs())


def _write_task_file(tasks_dir: Path, task_id: str, priority: str, status: str, slug: str) -> Path:
    """Write a valid task file (filename-only metadata, body is plain markdown)."""
    filename = get_expected_filename(task_id, priority, status, slug)
    path = tasks_dir / filename
    path.write_text(f"# Task {task_id}\n", encoding="utf-8")
    return path


def task_directories(n: int):
    @st.composite
    def _make(draw):
        params_list = draw(
            st.lists(
                st.tuples(priorities(), statuses(), slugs()),
                min_size=n,
                max_size=n,
            )
        )
        ids = draw(
            st.lists(
                task_ids(),
                min_size=n,
                max_size=n,
                unique=True,
            )
        )
        tmp = Path(tempfile.mkdtemp())
        for tid, (pri, sta, slug) in zip(ids, params_list):
            _write_task_file(tmp, tid, pri, sta, slug)
        return tmp
    return _make()


# ---------------------------------------------------------------------------
# Round-trip properties
# ---------------------------------------------------------------------------

@given(valid_task_params())
def test_filename_roundtrip(params):
    """Property 1: get_expected_filename then parse_task_file yields the same inputs."""
    task_id, priority, status, slug = params
    filename = get_expected_filename(task_id, priority, status, slug)
    with tempfile.TemporaryDirectory() as tmp_str:
        tmp = Path(tmp_str)
        path = tmp / filename
        path.write_text("# body\n", encoding="utf-8")
        task = parse_task_file(path)
        assert task is not None
        assert task.id == task_id
        assert task.priority == priority
        assert task.status == status
        assert task.slug == slug


@given(valid_task_params())
def test_parse_regenerate_roundtrip(params):
    """Property 2: parse then regenerate equals the original filename."""
    task_id, priority, status, slug = params
    original = get_expected_filename(task_id, priority, status, slug)
    with tempfile.TemporaryDirectory() as tmp_str:
        tmp = Path(tmp_str)
        path = tmp / original
        path.write_text("# body\n", encoding="utf-8")
        task = parse_task_file(path)
        assert task is not None
        regenerated = get_expected_filename(task.id, task.priority, task.status, task.slug)
        assert regenerated == original


@given(
    task_ids(),
    priorities(),
    statuses(),
    st.lists(
        st.from_regex(r"[a-z][a-z0-9]{0,8}", fullmatch=True),
        min_size=2,
        max_size=5,
    ).map(lambda parts: "-".join(parts)),
)
def test_slug_preservation(task_id, priority, status, slug):
    """Property 3: multi-hyphen slugs round-trip without corruption."""
    assume("--" not in slug)
    with tempfile.TemporaryDirectory() as tmp_str:
        tmp = Path(tmp_str)
        filename = get_expected_filename(task_id, priority, status, slug)
        path = tmp / filename
        path.write_text("# body\n", encoding="utf-8")
        task = parse_task_file(path)
        assert task is not None
        assert task.slug == slug


# ---------------------------------------------------------------------------
# Structural invariants
# ---------------------------------------------------------------------------

@given(valid_task_params())
def test_parsed_id_format(params):
    """Property 4: parsed ID matches the DDNNN (5-digit numeric) format."""
    task_id, priority, status, slug = params
    with tempfile.TemporaryDirectory() as tmp_str:
        tmp = Path(tmp_str)
        path = tmp / get_expected_filename(task_id, priority, status, slug)
        path.write_text("# body\n")
        task = parse_task_file(path)
        assert task is not None
        assert re.match(r'^\d{5}$', task.id)


@given(valid_task_params())
def test_parsed_priority_valid(params):
    """Property 5: parsed priority is always in VALID_PRIORITIES."""
    task_id, priority, status, slug = params
    with tempfile.TemporaryDirectory() as tmp_str:
        tmp = Path(tmp_str)
        path = tmp / get_expected_filename(task_id, priority, status, slug)
        path.write_text("# body\n")
        task = parse_task_file(path)
        assert task is not None
        assert task.priority in VALID_PRIORITIES


@given(valid_task_params())
def test_parsed_status_valid(params):
    """Property 6: parsed status is always in VALID_STATUSES."""
    task_id, priority, status, slug = params
    with tempfile.TemporaryDirectory() as tmp_str:
        tmp = Path(tmp_str)
        path = tmp / get_expected_filename(task_id, priority, status, slug)
        path.write_text("# body\n")
        task = parse_task_file(path)
        assert task is not None
        assert task.status in VALID_STATUSES


@given(valid_task_params())
def test_filename_starts_with_five_char_id(params):
    """Property 7: generated filename always starts with a 5-digit numeric ID."""
    task_id, priority, status, slug = params
    filename = get_expected_filename(task_id, priority, status, slug)
    assert re.match(r"^\d{5}-", filename), f"Expected DDNNN prefix, got: {filename!r}"
    prefix = filename.split("-")[0]
    assert len(prefix) == 5


@given(valid_task_params())
def test_filename_contains_exactly_one_double_dash(params):
    """Property 8: generated filename contains exactly one '--' separator."""
    task_id, priority, status, slug = params
    filename = get_expected_filename(task_id, priority, status, slug)
    assert filename.count("--") == 1, f"Expected exactly one '--', got: {filename!r}"


@given(
    st.text(
        alphabet=st.characters(blacklist_categories=("Cs",)),
        min_size=0,
        max_size=80,
    )
)
def test_parse_returns_none_for_non_conforming_filenames(name):
    """Property 9: parse_task_file returns None for all non-conforming filenames."""
    with tempfile.TemporaryDirectory() as tmp_str:
        tmp = Path(tmp_str)
        safe_name = re.sub(r"[/\\\x00]", "_", name)
        if not safe_name.endswith(".md"):
            safe_name = safe_name + ".md"
        from taskmd.core import _FILENAME_RE
        if _FILENAME_RE.match(safe_name):
            return  # skip — Hypothesis generated a valid filename
        try:
            path = tmp / safe_name
            path.write_text("body\n", encoding="utf-8")
            result = parse_task_file(path)
            assert result is None
        except (OSError, ValueError):
            pass


# ---------------------------------------------------------------------------
# Idempotency and relationships
# ---------------------------------------------------------------------------

@given(task_directories(3))
@settings(suppress_health_check=[HealthCheck.function_scoped_fixture])
def test_fix_idempotency(tasks_dir):
    """Property 10: fix(fix(dir)) == fix(dir) — second run renames 0."""
    fix(tasks_dir)
    result2 = fix(tasks_dir)
    assert result2.renamed == 0
    assert result2.ok


@given(task_directories(3))
@settings(suppress_health_check=[HealthCheck.function_scoped_fixture])
def test_fix_implies_validate(tasks_dir):
    """Property 11: after fix(dir).ok, validate(dir).ok is True."""
    fix_result = fix(tasks_dir)
    if fix_result.ok:
        val_result = validate(tasks_dir)
        assert val_result.ok, f"validate failed after successful fix: {val_result.errors}"


@given(task_directories(3))
@settings(suppress_health_check=[HealthCheck.function_scoped_fixture])
def test_fix_does_not_change_file_count(tasks_dir):
    """Property 12: fix does not change the number of files in the directory."""
    before = len(list(tasks_dir.glob("*.md")))
    fix(tasks_dir)
    after = len(list(tasks_dir.glob("*.md")))
    assert before == after


# ---------------------------------------------------------------------------
# next_id properties
# ---------------------------------------------------------------------------

@given(task_directories(0))
@settings(suppress_health_check=[HealthCheck.function_scoped_fixture])
def test_next_id_starts_at_001(tasks_dir):
    """Property 14: next_id ends with 001 for empty/nonexistent dirs."""
    assert next_id(tasks_dir).endswith("001")
    assert next_id(tasks_dir / "nonexistent").endswith("001")


@given(task_directories(3))
@settings(suppress_health_check=[HealthCheck.function_scoped_fixture])
def test_next_id_format(tasks_dir):
    """Property 15: next_id always returns a valid 5-digit numeric string."""
    result = next_id(tasks_dir)
    assert re.match(r'^\d{5}$', result), f"Invalid next_id: {result!r}"


@given(valid_task_params())
def test_template_and_ancillary_transparent(params):
    """Property 17: template and ancillary files are transparent to all operations."""
    task_id, priority, status, slug = params
    with tempfile.TemporaryDirectory() as tmp_str:
        tasks_dir = Path(tmp_str)
        _write_task_file(tasks_dir, task_id, priority, status, slug)
        (tasks_dir / "_TEMPLATE.md").write_text("# Template\n")
        task_stem = get_expected_filename(task_id, priority, status, slug)[:-3]
        (tasks_dir / f"{task_stem}.qaplan.md").write_text("ancillary\n")

        val_result = validate(tasks_dir)
        assert val_result.ok, f"Unexpected errors: {val_result.errors}"
        assert val_result.file_count == 1

        fix_result = fix(tasks_dir)
        assert fix_result.ok


def test_validate_nonexistent_directory():
    """Property 18: validate on a non-existent directory returns empty-valid."""
    with tempfile.TemporaryDirectory() as tmp_str:
        missing = Path(tmp_str) / "does_not_exist"
        result = validate(missing)
        assert result.ok
        assert result.file_count == 0
        assert result.errors == []


def test_fix_nonexistent_directory():
    """Property 19: fix on a non-existent directory returns empty-ok."""
    with tempfile.TemporaryDirectory() as tmp_str:
        missing = Path(tmp_str) / "does_not_exist"
        result = fix(missing)
        assert result.ok
        assert result.renamed == 0
        assert result.errors == []


@given(
    task_ids(),
    priorities(),
    statuses(),
    slugs(),
    priorities(),
    statuses(),
    slugs(),
)
def test_duplicate_ids_always_detected(task_id, pri1, sta1, slug1, pri2, sta2, slug2):
    """Property 20: duplicate task IDs are always detected by validate."""
    assume(
        get_expected_filename(task_id, pri1, sta1, slug1)
        != get_expected_filename(task_id, pri2, sta2, slug2)
    )
    with tempfile.TemporaryDirectory() as tmp_str:
        tasks_dir = Path(tmp_str)
        _write_task_file(tasks_dir, task_id, pri1, sta1, slug1)
        _write_task_file(tasks_dir, task_id, pri2, sta2, slug2)
        result = validate(tasks_dir)
        assert not result.ok
        assert any("duplicate task id" in e for e in result.errors)


@given(
    priorities(),
    statuses(),
    slugs(),
    priorities(),
    statuses(),
    slugs(),
    st.integers(min_value=1, max_value=990),
)
def test_duplicate_ids_fix_renumbers(pri1, sta1, slug1, pri2, sta2, slug2, seq):
    """Property 21: duplicate task IDs are resolved by fix, which renumbers
    the loser. After fix, validate is clean."""
    with tempfile.TemporaryDirectory() as tmp_str:
        tasks_dir = Path(tmp_str)
        task_id = _prefix_for(tasks_dir) + f"{seq:03d}"
        assume(
            get_expected_filename(task_id, pri1, sta1, slug1)
            != get_expected_filename(task_id, pri2, sta2, slug2)
        )
        _write_task_file(tasks_dir, task_id, pri1, sta1, slug1)
        _write_task_file(tasks_dir, task_id, pri2, sta2, slug2)
        fix_result = fix(tasks_dir)
        assert fix_result.ok, fix_result.errors
        assert len(fix_result.renumbered) == 1
        old_id, new_id, _old, _new = fix_result.renumbered[0]
        assert old_id == task_id
        assert new_id != task_id
        result = validate(tasks_dir)
        assert result.ok, result.errors


@given(task_directories(4))
@settings(suppress_health_check=[HealthCheck.function_scoped_fixture])
def test_validate_file_count_matches_actual(tasks_dir):
    """Property 22: validate.file_count matches the actual count of task files."""
    from taskmd.core import _task_files
    actual = len(_task_files(tasks_dir))
    result = validate(tasks_dir)
    assert result.file_count == actual
