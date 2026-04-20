"""Type stubs for the taskmd._core Rust extension module."""

from typing import Optional, TypedDict

# ── Constants ─────────────────────────────────────────────────────────────────

FILENAME_PATTERN: str
VALID_STATUSES: list[str]
VALID_PRIORITIES: list[str]
VALID_FIELDS: list[str]

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

# ── Frontmatter ───────────────────────────────────────────────────────────────

def parse_frontmatter(content: str) -> dict[str, str]: ...

# ── Task file operations ──────────────────────────────────────────────────────

class TaskDict(TypedDict):
    path: str
    id: str
    priority: str
    status: str
    slug: str
    fields: dict[str, str]

def parse_task_file(path: str) -> Optional[TaskDict]: ...
def list_tasks(tasks_dir: str) -> list[TaskDict]: ...
def find_task_by_id(tasks_dir: str, id: str) -> Optional[TaskDict]: ...
def rename_status(tasks_dir: str, id: str, new_status: str) -> tuple[str, str]: ...

# ── Validate ─────────────────────────────────────────────────────────────────

class ValidateDict(TypedDict):
    errors: list[str]
    file_count: int

def validate(tasks_dir: str) -> ValidateDict: ...

# ── Fix ───────────────────────────────────────────────────────────────────────

def fix_summary(patched: int, renamed: int, migrated: int) -> str: ...

class FixDict(TypedDict):
    patched: int
    renamed: int
    migrated: int
    patches: list[tuple[str, str]]
    renames: list[tuple[str, str]]
    errors: list[str]

def do_fix(tasks_dir: str) -> FixDict: ...

# ── Init ──────────────────────────────────────────────────────────────────────

class InitDict(TypedDict):
    tasks_dir: str
    created: list[str]
    template_fields: list[str]
    error: Optional[str]

def do_init(tasks_dir: str) -> InitDict: ...

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
    artifact: str,
    body: str,
) -> CreateDict: ...
