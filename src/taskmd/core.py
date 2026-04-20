"""taskmd core library — thin Python shim over the taskmd._core Rust extension.

The public API (dataclasses, function signatures, constants) is unchanged.
All logic lives in taskmd._core (compiled from taskmd-py/src/lib.rs via
taskmd-core/).
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING

from taskmd._core import (  # type: ignore[import]
    FILENAME_PATTERN as _FILENAME_PATTERN,
    VALID_FIELDS as _VALID_FIELDS,
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
    parse_frontmatter as _parse_frontmatter_str,
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
VALID_FIELDS: frozenset[str] = frozenset(_VALID_FIELDS)

# Compiled from the canonical Rust constant — single definition, always in sync.
_FILENAME_RE = re.compile(_FILENAME_PATTERN)

# ---------------------------------------------------------------------------
# Data types  (kept in Python for backwards-compatible attribute access)
# ---------------------------------------------------------------------------


@dataclass
class TaskFile:
    """Parsed representation of a task file."""

    path: Path
    id: str
    priority: str
    status: str
    slug: str
    fields: dict[str, str]


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

    patched: int = 0
    renamed: int = 0
    migrated: int = 0
    patches: list[tuple[str, str]] = field(default_factory=list)
    renames: list[tuple[str, str]] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return len(self.errors) == 0

    def summary(self) -> str:
        """Human-readable summary — delegates to the canonical Rust implementation."""
        return _fix_summary(self.patched, self.renamed, self.migrated)


@dataclass
class InitResult:
    """Result of initializing a tasks directory."""

    tasks_dir: Path
    created: list[str] = field(default_factory=list)
    template_fields: list[str] = field(default_factory=list)
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
    """Decompose a task ID into (prefix, sequence_number)."""
    prefix, seq = _parse_id_parts_raw(task_id)
    return (prefix, int(seq))


def _prefix_for(tasks_dir: Path | str) -> str:
    """Derive a deterministic 2-digit prefix from hostname + tasks dir realpath."""
    return _prefix_for_raw(str(tasks_dir))


def _needs_migration(task_id: str, expected_prefix: str) -> bool:
    """True if a task ID needs migration to the expected prefix."""
    return _needs_migration_raw(task_id, expected_prefix)


def _task_files(tasks_dir: Path | str) -> list[Path]:
    """Return main task .md files (excluding template and ancillary), sorted."""
    return [Path(p) for p in _task_files_raw(str(tasks_dir))]


def _parse_frontmatter(path: Path) -> dict[str, str]:
    """Read a file and return its frontmatter fields."""
    return _parse_frontmatter_str(path.read_text(encoding="utf-8"))


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
        fields=d["fields"],
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
    """Parse a task file's filename and frontmatter. Returns None if not a task file."""
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
    """Change a task's status: update frontmatter then rename the file.

    Returns ``(old_filename, new_filename)``.
    Raises ``RuntimeError`` if the task is not found or the target already exists.
    """
    return _rename_status(str(Path(tasks_dir)), task_id, new_status)


def validate(tasks_dir: Path | str = "tasks") -> ValidationResult:
    """Validate all task files in a directory."""
    d = _validate(str(Path(tasks_dir)))
    return ValidationResult(errors=d["errors"], file_count=d["file_count"])


def fix(tasks_dir: Path | str = "tasks") -> FixResult:
    """Auto-fix task files: inject missing 'created', rename to match frontmatter."""
    d = _fix(str(Path(tasks_dir)))
    return FixResult(
        patched=d["patched"],
        renamed=d["renamed"],
        migrated=d["migrated"],
        patches=[tuple(p) for p in d["patches"]],
        renames=[tuple(r) for r in d["renames"]],
        errors=d["errors"],
    )


def init(tasks_dir: Path | str = "tasks") -> InitResult:
    """Initialise a tasks directory with a template file."""
    d = _init(str(Path(tasks_dir)))
    return InitResult(
        tasks_dir=Path(d["tasks_dir"]),
        created=d["created"],
        template_fields=d["template_fields"],
        error=d["error"],
    )


def create_task(
    tasks_dir: Path | str,
    *,
    slug: str,
    artifact: str,
    priority: str = "p2",
    status: str = "ready",
    body: str = "",
) -> CreateResult:
    """Atomically allocate an ID, synthesize frontmatter, and write a new task file.

    Raises ``RuntimeError`` on invalid input, missing tasks dir, or collision
    exhaustion.  Does not depend on the caller having first run
    ``taskmd next`` — the ID is allocated and claimed as a single operation.
    """
    d = _create(str(Path(tasks_dir)), priority, status, slug, artifact, body)
    return CreateResult(id=d["id"], path=Path(d["path"]), filename=d["filename"])
