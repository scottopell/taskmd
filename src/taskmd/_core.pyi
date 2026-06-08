"""Type stubs for the taskmd._core Rust extension module."""

from typing import Optional, TypedDict

# ── Constants ─────────────────────────────────────────────────────────────────

FILENAME_PATTERN: str
TEMPLATE_FILENAME: str
DEFAULT_TASKS_DIR_NAME: str
VALID_STATUSES: list[str]
VALID_PRIORITIES: list[str]

# ── Internal helpers (used by test suite and core.py) ────────────────────────

def task_files(tasks_dir: str) -> list[str]: ...
def is_legacy_id(task_id: str) -> bool: ...
def needs_migration(task_id: str, expected_prefix: str) -> bool: ...
def parse_id_parts(task_id: str) -> tuple[str, int]: ...
def prefix_for(tasks_dir: str) -> str: ...

# ── ID / filename / slug ─────────────────────────────────────────────────────

def next_id(tasks_dir: str) -> str: ...
def get_expected_filename(id: str, priority: str, status: str, slug: str) -> str: ...
def derive_slug(title: str) -> str: ...

# ── Task file operations ──────────────────────────────────────────────────────

class TaskDict(TypedDict):
    path: str
    id: str
    priority: str
    status: str
    slug: str

def parse_task_file(path: str) -> Optional[TaskDict]: ...
def list_tasks(tasks_dir: str) -> list[TaskDict]: ...
def find_task_by_id(tasks_dir: str, id: str) -> Optional[TaskDict]: ...
def find_task_by_slug(tasks_dir: str, slug: str) -> list[TaskDict]: ...
def ancillary_files_for(tasks_dir: str, id: str) -> list[str]: ...
def update_task(
    tasks_dir: str,
    id: str,
    priority: Optional[str] = None,
    status: Optional[str] = None,
    slug: Optional[str] = None,
) -> tuple[str, str]: ...

# ── Validate ─────────────────────────────────────────────────────────────────

class ValidateDict(TypedDict):
    errors: list[str]
    file_count: int

def validate(tasks_dir: str) -> ValidateDict: ...

# ── Fix ───────────────────────────────────────────────────────────────────────

def fix_summary(
    renamed: int, migrated: int, renumbered: int, frontmatter_stripped: int
) -> str: ...

class FixDict(TypedDict):
    renamed: int
    migrated: int
    renames: list[tuple[str, str]]
    # Each tuple is (old_id, new_id, old_filename, new_filename).
    renumbered: list[tuple[str, str, str, str]]
    frontmatter_stripped: list[str]
    frontmatter_pending: list[str]
    errors: list[str]

def do_fix(tasks_dir: str, migrate: Optional[bool] = None) -> FixDict: ...

# ── Init ──────────────────────────────────────────────────────────────────────

class InitDict(TypedDict):
    tasks_dir: str
    created: list[str]
    error: Optional[str]

def do_init(tasks_dir: str) -> InitDict: ...
def do_ensure_initialized(tasks_dir: str) -> InitDict: ...

# ── Discovery ─────────────────────────────────────────────────────────────────

# (sole_match, candidates): sole_match is set iff exactly one candidate exists;
# candidates is sorted and holds all immediate child dirs carrying _TEMPLATE.md.
def discover_tasks_dir(dir: str) -> tuple[Optional[str], list[str]]: ...
def discover_tasks_dir_or_default(dir: str) -> str: ...

# ── Create ────────────────────────────────────────────────────────────────────

class CreateDict(TypedDict):
    id: str
    path: str
    filename: str

def do_create(
    tasks_dir: str,
    priority: str,
    status: str,
    slug: str,
    body: str,
) -> CreateDict: ...
