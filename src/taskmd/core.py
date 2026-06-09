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
    ancillary_files_for as _ancillary_files_for,
    derive_slug,
    discover_tasks_dir as _discover_tasks_dir,
    discover_tasks_dir_or_default as _discover_tasks_dir_or_default,
    do_create as _create,
    do_ensure_initialized as _ensure_initialized,
    do_fix as _fix,
    do_init as _init,
    find_task_by_id as _find_task_by_id,
    find_task_by_slug as _find_task_by_slug,
    fix_summary as _fix_summary,
    get_expected_filename as _get_expected_filename,
    is_legacy_id as _is_legacy_id,
    needs_migration as _needs_migration_raw,
    list_tasks as _list_tasks,
    next_id as _next_id,
    parse_id_parts as _parse_id_parts_raw,
    parse_task_file as _parse_task_file,
    prefix_for as _prefix_for_raw,
    task_files as _task_files_raw,
    update_task as _update_task,
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

    @property
    def filename(self) -> str:
        """Return the basename of the task file."""
        return self.path.name


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
    # Filenames whose YAML frontmatter was stripped (only populated when
    # `migrate=True` was passed). Empty otherwise.
    frontmatter_stripped: list[str] = field(default_factory=list)
    # Filenames detected as having frontmatter when `migrate=None` (the
    # default "prompt" mode). When non-empty, `errors` will contain a single
    # message pointing the user at --migrate / --no-migrate.
    frontmatter_pending: list[str] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return len(self.errors) == 0

    def summary(self) -> str:
        return _fix_summary(
            self.renamed,
            self.migrated,
            len(self.renumbered),
            len(self.frontmatter_stripped),
        )


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
class EnsureResult:
    """Result of idempotently ensuring a tasks directory exists."""

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


def update_task(
    tasks_dir: Path | str,
    task_id: str,
    *,
    priority: str | None = None,
    status: str | None = None,
    slug: str | None = None,
) -> tuple[str, str]:
    """Apply changes to an existing task by renaming the file.

    Each kwarg is None to leave unchanged. A no-op call (all kwargs None or
    matching the current value) returns ``(old_filename, old_filename)``
    without touching the filesystem.

    Returns ``(old_filename, new_filename)``.

    Raises ``ValueError`` if priority/status/slug are invalid;
    ``RuntimeError`` if the task is not found or the target file exists.
    """
    return _update_task(str(Path(tasks_dir)), task_id, priority, status, slug)


def find_task_by_slug(tasks_dir: Path | str, slug: str) -> list[TaskFile]:
    """Return all tasks whose slug matches the given slug. May be empty."""
    return [
        _dict_to_task(t) for t in _find_task_by_slug(str(Path(tasks_dir)), slug)
    ]


def ancillary_files_for(tasks_dir: Path | str, task_id: str) -> list[Path]:
    """Return the paths of all ancillary files attached to a task ID.

    Ancillary files are sibling files in the tasks directory whose filename
    begins with the task's filename stem and has an additional segment
    (e.g. ``34001-p2-ready--slug.qaplan.md``).
    """
    return [Path(p) for p in _ancillary_files_for(str(Path(tasks_dir)), task_id)]


def validate(tasks_dir: Path | str = "tasks") -> ValidationResult:
    """Validate all task files in a directory."""
    d = _validate(str(Path(tasks_dir)))
    return ValidationResult(errors=d["errors"], file_count=d["file_count"])


def fix(
    tasks_dir: Path | str = "tasks",
    *,
    migrate: bool | None = None,
) -> FixResult:
    """Auto-fix task files: optionally strip legacy frontmatter, migrate legacy
    IDs, and renumber duplicate IDs.

    ``migrate`` controls how legacy YAML frontmatter is handled:
      - ``None`` (default): refuse to run if any file has frontmatter, returning
        an error pointing the user at ``migrate=True`` or ``migrate=False``.
      - ``True``: strip frontmatter from every file that has it. Destructive —
        commit before running.
      - ``False``: skip the frontmatter check entirely.
    """
    d = _fix(str(Path(tasks_dir)), migrate)
    return FixResult(
        renamed=d["renamed"],
        migrated=d["migrated"],
        renames=[tuple(r) for r in d["renames"]],
        renumbered=[tuple(r) for r in d["renumbered"]],
        frontmatter_stripped=list(d["frontmatter_stripped"]),
        frontmatter_pending=list(d["frontmatter_pending"]),
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


def ensure_initialized(tasks_dir: Path | str = "tasks") -> EnsureResult:
    """Idempotently ensure a tasks directory exists with a ``_TEMPLATE.md``.

    Safe to call repeatedly: creates only what's missing. Returns an
    ``EnsureResult`` listing the paths that were created on this call (empty
    if everything was already in place).
    """
    d = _ensure_initialized(str(Path(tasks_dir)))
    return EnsureResult(
        tasks_dir=Path(d["tasks_dir"]),
        created=d["created"],
        error=d["error"],
    )


def discover_tasks_dir(start: Path | str = ".") -> tuple[Path | None, list[str]]:
    """Scan ``start`` for a taskmd tasks directory (an immediate child directory
    holding ``_TEMPLATE.md``).

    Returns ``(path, candidates)``:
        - ``(Path, [name])`` when exactly one match is found (``path`` is
          ``start / name``).
        - ``(None, [])`` when no candidate is found.
        - ``(None, [name1, name2, ...])`` sorted alphabetically when 2+ match;
          the caller must disambiguate.
    """
    start_path = Path(start)
    sole, candidates = _discover_tasks_dir(str(start_path))
    found = start_path / sole if sole is not None else None
    return found, list(candidates)


def discover_tasks_dir_or_default(start: Path | str = ".") -> Path:
    """Resolve a tasks directory under ``start``, never failing.

    Prefers a candidate named exactly ``tasks``; otherwise the lexically-first
    candidate; otherwise falls back to ``start / "tasks"`` even though nothing
    exists there yet. Useful for editor integrations that want a usable path
    without prompting.
    """
    start_path = Path(start)
    return start_path / _discover_tasks_dir_or_default(str(start_path))


def create_task(
    tasks_dir: Path | str,
    *,
    slug: str,
    body: str,
    priority: str = "p2",
    status: str = "ready",
) -> CreateResult:
    """Atomically allocate an ID and write a new task file containing only `body`.

    `body` is required and must be non-empty (after trimming whitespace) — a
    task with no description is a placeholder. Raises ``RuntimeError`` if it's
    missing or whitespace-only.
    """
    d = _create(str(Path(tasks_dir)), priority, status, slug, body)
    return CreateResult(id=d["id"], path=Path(d["path"]), filename=d["filename"])
