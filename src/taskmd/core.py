"""taskmd core library — thin Python shim over the taskmd._core Rust extension."""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING

from taskmd._core import (  # type: ignore[import]
    FILENAME_PATTERN as _FILENAME_PATTERN,
    VALID_PRIORITIES as _VALID_PRIORITIES,
    VALID_STATUSES as _VALID_STATUSES,
    derive_slug,
    do_create as _create,
    do_fix as _fix,
    do_init as _init,
    find_task_by_id as _find_task_by_id,
    fix_summary as _fix_summary,
    get_expected_filename as _get_expected_filename,
    is_legacy_id as _is_legacy_id,
    needs_migration as _needs_migration_raw,
    list_tasks as _list_tasks,
    next_id as _next_id,
    parse_id_parts as _parse_id_parts_raw,
    parse_task_file as _parse_task_file,
    prefix_for as _prefix_for_raw,
    rename_status as _rename_status,
    task_files as _task_files_raw,
    validate as _validate,
)

if TYPE_CHECKING:
    from taskmd._core import TaskDict as _TaskDict

# ---------------------------------------------------------------------------
# Constants  (single source of truth: Rust; Python wraps in frozenset)
# ---------------------------------------------------------------------------

VALID_STATUSES: frozenset[str] = frozenset(_VALID_STATUSES)
VALID_PRIORITIES: frozenset[str] = frozenset(_VALID_PRIORITIES)

# Compiled from the canonical Rust constant — single definition, always in sync.
_FILENAME_RE = re.compile(_FILENAME_PATTERN)

# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------


@dataclass
class TaskFile:
    """Parsed representation of a task file. All fields come from the filename."""

    path: Path
    id: str
    priority: str
    status: str
    slug: str


@dataclass
class ValidationResult:
    """Result of validating a tasks directory."""

    errors: list[str] = field(default_factory=list)
    file_count: int = 0

    @property
    def ok(self) -> bool:
        return len(self.errors) == 0


@dataclass
class FixResult:
    """Result of fixing a tasks directory."""

    renamed: int = 0
    migrated: int = 0
    renames: list[tuple[str, str]] = field(default_factory=list)
    # Each entry: (old_id, new_id, old_filename, new_filename).
    renumbered: list[tuple[str, str, str, str]] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return len(self.errors) == 0

    def summary(self) -> str:
        return _fix_summary(self.renamed, self.migrated, len(self.renumbered))


@dataclass
class InitResult:
    """Result of initializing a tasks directory."""

    tasks_dir: Path
    created: list[str] = field(default_factory=list)
    error: str | None = None

    @property
    def ok(self) -> bool:
        return self.error is None


@dataclass
class CreateResult:
    """Result of atomically creating a new task file."""

    id: str
    path: Path
    filename: str


# ---------------------------------------------------------------------------
# Private helpers re-exported for the test suite
# ---------------------------------------------------------------------------


def _parse_id_parts(task_id: str) -> tuple[str, int]:
    prefix, seq = _parse_id_parts_raw(task_id)
    return (prefix, int(seq))


def _prefix_for(tasks_dir: Path | str) -> str:
    return _prefix_for_raw(str(tasks_dir))


def _needs_migration(task_id: str, expected_prefix: str) -> bool:
    return _needs_migration_raw(task_id, expected_prefix)


def _task_files(tasks_dir: Path | str) -> list[Path]:
    return [Path(p) for p in _task_files_raw(str(tasks_dir))]


# ---------------------------------------------------------------------------
# Internal conversion helper
# ---------------------------------------------------------------------------


def _dict_to_task(d: _TaskDict) -> TaskFile:
    return TaskFile(
        path=Path(d["path"]),
        id=d["id"],
        priority=d["priority"],
        status=d["status"],
        slug=d["slug"],
    )


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------


def next_id(tasks_dir: Path | str = "tasks") -> str:
    """Return the next available task ID for this tasks directory."""
    return _next_id(str(Path(tasks_dir)))


def get_expected_filename(task_id: str, priority: str, status: str, slug: str) -> str:
    """Generate the canonical filename for a task. Double-dash before slug."""
    return _get_expected_filename(task_id, priority, status, slug)


def parse_task_file(path: Path) -> TaskFile | None:
    """Parse a task filename. Returns None if it doesn't match the pattern."""
    d = _parse_task_file(str(path))
    return None if d is None else _dict_to_task(d)


def list_tasks(tasks_dir: Path | str = "tasks") -> list[TaskFile]:
    """Return all parseable task files in a directory, sorted by ID."""
    return [_dict_to_task(t) for t in _list_tasks(str(Path(tasks_dir)))]


def find_task_by_id(tasks_dir: Path | str, task_id: str) -> TaskFile | None:
    """Find a single task by its ID. Returns None if not found."""
    d = _find_task_by_id(str(Path(tasks_dir)), task_id)
    return None if d is None else _dict_to_task(d)


def rename_status(
    tasks_dir: Path | str, task_id: str, new_status: str
) -> tuple[str, str]:
    """Change a task's status by renaming the file.

    Returns ``(old_filename, new_filename)``.
    Raises ``RuntimeError`` if the task is not found or the target already exists.
    """
    return _rename_status(str(Path(tasks_dir)), task_id, new_status)


def validate(tasks_dir: Path | str = "tasks") -> ValidationResult:
    """Validate all task files in a directory."""
    d = _validate(str(Path(tasks_dir)))
    return ValidationResult(errors=d["errors"], file_count=d["file_count"])


def fix(tasks_dir: Path | str = "tasks") -> FixResult:
    """Auto-fix task files: migrate legacy IDs and renumber duplicate IDs."""
    d = _fix(str(Path(tasks_dir)))
    return FixResult(
        renamed=d["renamed"],
        migrated=d["migrated"],
        renames=[tuple(r) for r in d["renames"]],
        renumbered=[tuple(r) for r in d["renumbered"]],
        errors=d["errors"],
    )


def init(tasks_dir: Path | str = "tasks") -> InitResult:
    """Initialise a tasks directory with a template file."""
    d = _init(str(Path(tasks_dir)))
    return InitResult(
        tasks_dir=Path(d["tasks_dir"]),
        created=d["created"],
        error=d["error"],
    )


def create_task(
    tasks_dir: Path | str,
    *,
    slug: str,
    priority: str = "p2",
    status: str = "ready",
    body: str = "",
) -> CreateResult:
    """Atomically allocate an ID and write a new task file containing only `body`."""
    d = _create(str(Path(tasks_dir)), priority, status, slug, body)
    return CreateResult(id=d["id"], path=Path(d["path"]), filename=d["filename"])
