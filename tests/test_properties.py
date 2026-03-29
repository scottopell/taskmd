"""Property-based tests for taskmd core library.

Uses Hypothesis to verify invariants across round-trips, structural
constraints, idempotency, and filesystem operations.
"""
from __future__ import annotations

import re
import tempfile
from pathlib import Path

import pytest
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
    """5-character task IDs: 2 chars from ID alphabet (no O) + 3-digit sequence."""
    prefix = st.from_regex(r"[A-HJ-NP-Z0-9]{2}", fullmatch=True)
    seq = st.integers(min_value=1, max_value=999)
    return st.tuples(prefix, seq).map(lambda ps: ps[0] + f"{ps[1]:03d}")


def priorities():
    """Sampled from the set of valid priorities."""
    return st.sampled_from(sorted(VALID_PRIORITIES))


def statuses():
    """Sampled from the set of valid statuses."""
    return st.sampled_from(sorted(VALID_STATUSES))


def slugs():
    """Kebab-case slug strings -- letters, digits, hyphens; no dots or slashes.

    At least one word segment so the slug is non-empty and valid.
    """
    word = st.from_regex(r"[a-z][a-z0-9]{0,8}", fullmatch=True)
    return st.lists(word, min_size=1, max_size=5).map(lambda parts: "-".join(parts))


def valid_task_params():
    """Composite strategy producing (task_id, priority, status, slug) tuples."""
    return st.tuples(task_ids(), priorities(), statuses(), slugs())


def _write_task_file(tasks_dir: Path, task_id: str, priority: str, status: str, slug: str) -> Path:
    """Write a valid task file to tasks_dir and return its path."""
    filename = get_expected_filename(task_id, priority, status, slug)
    path = tasks_dir / filename
    fm = f"---\ncreated: 2026-01-01\npriority: {priority}\nstatus: {status}\nartifact: src/{slug}.py\n---\n\n# Task {task_id}\n"
    path.write_text(fm, encoding="utf-8")
    return path


def task_directories(n: int):
    """Strategy that yields a tmp dir Path populated with N valid, unique-ID task files."""
    @st.composite
    def _make(draw):
        params_list = draw(
            st.lists(
                st.tuples(priorities(), statuses(), slugs()),
                min_size=n,
                max_size=n,
            )
        )
        # Draw N distinct IDs so no duplicate-ID conflict
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
        fm = f"---\ncreated: 2026-01-01\npriority: {priority}\nstatus: {status}\n---\n"
        path.write_text(fm, encoding="utf-8")
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
        fm = f"---\ncreated: 2026-01-01\npriority: {priority}\nstatus: {status}\n---\n"
        path.write_text(fm, encoding="utf-8")
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
        fm = f"---\ncreated: 2026-01-01\npriority: {priority}\nstatus: {status}\n---\n"
        path.write_text(fm, encoding="utf-8")
        task = parse_task_file(path)
        assert task is not None
        assert task.slug == slug


# ---------------------------------------------------------------------------
# Structural invariants
# ---------------------------------------------------------------------------

@given(valid_task_params())
def test_parsed_id_format(params):
    """Property 4: parsed ID matches the AANNN format."""
    task_id, priority, status, slug = params
    with tempfile.TemporaryDirectory() as tmp_str:
        tmp = Path(tmp_str)
        path = tmp / get_expected_filename(task_id, priority, status, slug)
        path.write_text(f"---\ncreated: 2026-01-01\npriority: {priority}\nstatus: {status}\n---\n")
        task = parse_task_file(path)
        assert task is not None
        assert re.match(r'^[A-HJ-NP-Z0-9]{2}\d{3}$', task.id)


@given(valid_task_params())
def test_parsed_priority_valid(params):
    """Property 5: parsed priority is always in VALID_PRIORITIES."""
    task_id, priority, status, slug = params
    with tempfile.TemporaryDirectory() as tmp_str:
        tmp = Path(tmp_str)
        path = tmp / get_expected_filename(task_id, priority, status, slug)
        path.write_text(f"---\ncreated: 2026-01-01\npriority: {priority}\nstatus: {status}\n---\n")
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
        path.write_text(f"---\ncreated: 2026-01-01\npriority: {priority}\nstatus: {status}\n---\n")
        task = parse_task_file(path)
        assert task is not None
        assert task.status in VALID_STATUSES


@given(valid_task_params())
def test_filename_starts_with_five_char_id(params):
    """Property 7: generated filename always starts with a 5-char AANNN ID."""
    task_id, priority, status, slug = params
    filename = get_expected_filename(task_id, priority, status, slug)
    assert re.match(r"^[A-HJ-NP-Z0-9]{2}\d{3}-", filename), f"Expected AANNN prefix, got: {filename!r}"
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
            return  # skip -- Hypothesis generated a valid filename
        try:
            path = tmp / safe_name
            path.write_text("---\nstatus: ready\n---\n", encoding="utf-8")
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
    """Property 10: fix(fix(dir)) == fix(dir) -- second run patches/renames 0."""
    fix(tasks_dir)
    result2 = fix(tasks_dir)
    assert result2.patched == 0
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


@given(task_directories(2))
@settings(suppress_health_check=[HealthCheck.function_scoped_fixture])
def test_fix_does_not_alter_non_created_frontmatter(tasks_dir):
    """Property 13: fix does not alter frontmatter fields other than 'created'."""
    from taskmd.core import _task_files, _parse_frontmatter

    before_fields: dict[str, dict[str, str]] = {}
    for path in _task_files(tasks_dir):
        fields = _parse_frontmatter(path)
        task = parse_task_file(path)
        if task:
            key = f"{task.id}-{task.slug}"
            before_fields[key] = {k: v for k, v in fields.items() if k != "created"}

    fix(tasks_dir)

    for path in _task_files(tasks_dir):
        task = parse_task_file(path)
        if task:
            key = f"{task.id}-{task.slug}"
            if key not in before_fields:
                continue
            fields_after = _parse_frontmatter(path)
            for k, v in before_fields[key].items():
                assert fields_after.get(k) == v, (
                    f"fix altered field {k!r}: was {v!r}, now {fields_after.get(k)!r}"
                )


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
    """Property 15: next_id always returns a valid 5-char AANNN string."""
    result = next_id(tasks_dir)
    assert re.match(r'^[A-HJ-NP-Z0-9]{2}\d{3}$', result), f"Invalid next_id: {result!r}"


@given(valid_task_params())
def test_template_and_ancillary_transparent(params):
    """Property 17: template and ancillary files are transparent to all operations."""
    task_id, priority, status, slug = params
    with tempfile.TemporaryDirectory() as tmp_str:
        tasks_dir = Path(tmp_str)
        _write_task_file(tasks_dir, task_id, priority, status, slug)
        # Add template
        (tasks_dir / "_TEMPLATE.md").write_text("---\ncreated: YYYY\npriority: p2\nstatus: ready\n---\n")
        # Add ancillary file
        task_stem = get_expected_filename(task_id, priority, status, slug)[:-3]
        (tasks_dir / f"{task_stem}.qaplan.md").write_text("some ancillary content\n")

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
        assert result.patched == 0
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
    task_ids(),
    priorities(),
    statuses(),
    slugs(),
    priorities(),
    statuses(),
    slugs(),
)
def test_duplicate_ids_not_fixable(task_id, pri1, sta1, slug1, pri2, sta2, slug2):
    """Property 21: duplicate task IDs cannot be fixed by fix."""
    assume(
        get_expected_filename(task_id, pri1, sta1, slug1)
        != get_expected_filename(task_id, pri2, sta2, slug2)
    )
    with tempfile.TemporaryDirectory() as tmp_str:
        tasks_dir = Path(tmp_str)
        _write_task_file(tasks_dir, task_id, pri1, sta1, slug1)
        _write_task_file(tasks_dir, task_id, pri2, sta2, slug2)
        fix(tasks_dir)
        result = validate(tasks_dir)
        assert not result.ok
        assert any("duplicate task id" in e for e in result.errors)


@given(task_directories(4))
@settings(suppress_health_check=[HealthCheck.function_scoped_fixture])
def test_validate_file_count_matches_actual(tasks_dir):
    """Property 22: validate.file_count matches the actual count of task files."""
    from taskmd.core import _task_files
    actual = len(_task_files(tasks_dir))
    result = validate(tasks_dir)
    assert result.file_count == actual


@given(valid_task_params())
def test_validate_errors_reference_originating_filename(params):
    """Property 23: validate errors reference the filename that caused them."""
    task_id, priority, status, slug = params
    with tempfile.TemporaryDirectory() as tmp_str:
        tasks_dir = Path(tmp_str)
        other_status = next(s for s in sorted(VALID_STATUSES) if s != status)
        filename = get_expected_filename(task_id, priority, status, slug)
        path = tasks_dir / filename
        fm = f"---\ncreated: 2026-01-01\npriority: {priority}\nstatus: {other_status}\n---\n"
        path.write_text(fm, encoding="utf-8")
        result = validate(tasks_dir)
        mismatch_errors = [e for e in result.errors if "doesn't match frontmatter" in e]
        assert mismatch_errors, "Expected a mismatch error"
        for err in mismatch_errors:
            assert filename in err, f"Error {err!r} does not reference filename {filename!r}"


_VALUE_SAFE_CHARS = st.characters(
    whitelist_categories=("Lu", "Ll", "Nd"),
    whitelist_characters="-_ /",
)

@given(
    task_ids(),
    priorities(),
    statuses(),
    slugs(),
    st.from_regex(r"[a-zA-Z][a-zA-Z0-9_-]{0,19}", fullmatch=True),
    st.text(alphabet=_VALUE_SAFE_CHARS, min_size=1, max_size=20),
    st.text(alphabet=_VALUE_SAFE_CHARS, min_size=1, max_size=20),
)
def test_frontmatter_key_preservation_with_colon_values(
    task_id, priority, status, slug, extra_key, value_prefix, value_suffix
):
    """Property 24: frontmatter keys with colon-containing values are preserved correctly."""
    extra_value = f"{value_prefix}: {value_suffix}"
    assert ":" in extra_value
    with tempfile.TemporaryDirectory() as tmp_str:
        tasks_dir = Path(tmp_str)
        filename = get_expected_filename(task_id, priority, status, slug)
        path = tasks_dir / filename
        fm = (
            f"---\n"
            f"created: 2026-01-01\n"
            f"priority: {priority}\n"
            f"status: {status}\n"
            f"{extra_key}: {extra_value}\n"
            f"---\n"
        )
        path.write_text(fm, encoding="utf-8")
        task = parse_task_file(path)
        assert task is not None
        assert task.fields.get(extra_key) == extra_value.strip(), (
            f"Key {extra_key!r} value corrupted: expected {extra_value.strip()!r}, "
            f"got {task.fields.get(extra_key)!r}"
        )


@given(valid_task_params())
def test_one_error_per_filename_frontmatter_mismatch(params):
    """Property 25: exactly one error is reported per filename/frontmatter mismatch."""
    task_id, priority, status, slug = params
    other_status = next(s for s in sorted(VALID_STATUSES) if s != status)
    with tempfile.TemporaryDirectory() as tmp_str:
        tasks_dir = Path(tmp_str)
        filename = get_expected_filename(task_id, priority, status, slug)
        path = tasks_dir / filename
        fm = f"---\ncreated: 2026-01-01\npriority: {priority}\nstatus: {other_status}\n---\n"
        path.write_text(fm, encoding="utf-8")
        result = validate(tasks_dir)
        mismatch_errors = [
            e for e in result.errors
            if "doesn't match frontmatter" in e and filename in e
        ]
        assert len(mismatch_errors) == 1, (
            f"Expected exactly 1 mismatch error for {filename!r}, got {len(mismatch_errors)}: {mismatch_errors}"
        )
