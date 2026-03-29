"""taskmd core library — pure functions, no CLI concerns.

REQ-TM-001 through REQ-TM-010. See specs/taskmd/requirements.md.

Every public function operates on a Path to a tasks directory and returns
structured results. No global state, no printing, no sys.exit.
"""

from __future__ import annotations

import hashlib
import re
import subprocess
from dataclasses import dataclass, field
from datetime import date
from pathlib import Path
from textwrap import dedent

# ---------------------------------------------------------------------------
# Constants (REQ-TM-002, REQ-TM-004)
# ---------------------------------------------------------------------------

VALID_STATUSES = frozenset({
    "ready",
    "in-progress",
    "blocked",
    "done",
    "wont-do",
    "brainstorming",
})

VALID_PRIORITIES = frozenset({"p0", "p1", "p2", "p3", "p4"})

# Task ID formats:
#   New:    AANNN  (2 chars from _ID_ALPHABET + 3 digits)  e.g. AB042-p2-ready--fix-the-bug.md
#   Legacy: NNNN   (4 digits)                               e.g. 0042-p2-ready--fix-the-bug.md
# The alternation tries 5-char new format first; 4-char legacy is the fallback.
# Note: I and O are excluded from the alphabet (ambiguous with 1 and 0).
_FILENAME_RE = re.compile(
    r"^([A-HJ-NP-Z0-9]{2}\d{3}|\d{4})-(p[0-4])-("
    + "|".join(sorted(VALID_STATUSES))
    + r")--(.+)\.md$"
)

_DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")

VALID_FIELDS = frozenset({"created", "priority", "status", "artifact"})

_ID_ALPHABET = "0123456789ABCDEFGHJKLMNPQRSTUVWXYZ"  # no I or O (ambiguous with 1 and 0)
_ID_BASE = len(_ID_ALPHABET)  # 34


def _prefix_for(tasks_dir: Path) -> str:
    """Derive a deterministic 2-char prefix from the tasks dir realpath."""
    h = hashlib.sha256(str(tasks_dir.resolve()).encode()).digest()
    val = int.from_bytes(h[:2], "big") % (_ID_BASE * _ID_BASE)
    return _ID_ALPHABET[val // _ID_BASE] + _ID_ALPHABET[val % _ID_BASE]


def _parse_id_parts(task_id: str) -> tuple[str, int]:
    """Decompose a task ID into (prefix, sequence_number).

    New format "AB042" -> ("AB", 42). Legacy "0042" -> ("", 42).
    """
    if _is_legacy_id(task_id):
        return ("", int(task_id))
    return (task_id[:2], int(task_id[2:]))


def _is_legacy_id(task_id: str) -> bool:
    """True if task_id is the old 4-digit numeric format."""
    return len(task_id) == 4 and task_id.isdigit()

_TEMPLATE_CONTENT = dedent("""\
    ---
    created: YYYY-MM-DD
    priority: p2
    status: ready
    artifact: path/to/output
    ---

    ## Summary

    What this task is about.

    ## Done When

    - [ ] Acceptance criteria here
""")


# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------

@dataclass
class TaskFile:
    """Parsed representation of a task file."""

    path: Path
    id: str  # opaque task ID (e.g. "AB042" or legacy "0042")
    priority: str
    status: str
    slug: str
    fields: dict[str, str]  # raw frontmatter key-value pairs


@dataclass
class ValidationResult:
    """Result of validating a tasks directory.

    ``file_count`` is the total number of task files examined — including
    files that failed validation, not just the ones that passed.
    """

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
    """Per-file patch details: list of (filename, inferred_date) pairs."""
    renames: list[tuple[str, str]] = field(default_factory=list)
    """Per-file rename details: list of (old_filename, new_filename) pairs."""
    errors: list[str] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return len(self.errors) == 0

    def summary(self) -> str:
        """Human-readable summary of what changed.

        Returns e.g. "Patched 2 file(s), renamed 3 file(s)" or
        "All files already correct".
        """
        if not self.patched and not self.renamed:
            return "All files already correct"
        parts = []
        if self.patched:
            parts.append(f"patched {self.patched} file(s)")
        if self.renamed:
            parts.append(f"renamed {self.renamed} file(s)")
        if self.migrated:
            parts.append(f"migrated {self.migrated} file(s) to AANNN format")
        return ", ".join(parts).capitalize()


@dataclass
class InitResult:
    """Result of initializing a tasks directory."""

    tasks_dir: Path
    created: list[str] = field(default_factory=list)
    """Paths created (directory and template file)."""
    template_fields: list[str] = field(default_factory=list)
    """Frontmatter fields present in the template."""
    error: str | None = None

    @property
    def ok(self) -> bool:
        return self.error is None


# ---------------------------------------------------------------------------
# File detection helpers
# ---------------------------------------------------------------------------

def _is_template(path: Path) -> bool:
    return path.name == "_TEMPLATE.md"


def _is_ancillary(path: Path) -> bool:
    """Ancillary files have a second dot segment: 0042-p2-ready--foo.qaplan.md"""
    stem = path.stem  # e.g., "0042-p2-ready-foo.qaplan"
    return "." in stem


def _task_files(tasks_dir: Path) -> list[Path]:
    """Return main task .md files, excluding template and ancillary files."""
    return sorted(
        p
        for p in tasks_dir.glob("*.md")
        if not _is_template(p) and not _is_ancillary(p)
    )


# ---------------------------------------------------------------------------
# Listing (REQ-TM-001)
# ---------------------------------------------------------------------------

def list_tasks(tasks_dir: Path | str = "tasks") -> list[TaskFile]:
    """Return all parseable task files in a directory, sorted by ID.

    Skips template and ancillary files. Files whose names don't match the
    expected pattern are silently skipped (use ``validate`` to find those).
    """
    tasks_dir = Path(tasks_dir)
    if not tasks_dir.exists():
        return []

    result: list[TaskFile] = []
    for path in _task_files(tasks_dir):
        task = parse_task_file(path)
        if task is not None:
            result.append(task)

    return sorted(result, key=lambda t: t.id)


# ---------------------------------------------------------------------------
# Parsing (REQ-TM-001, REQ-TM-003)
# ---------------------------------------------------------------------------

def parse_task_file(path: Path) -> TaskFile | None:
    """Parse a task file's filename and frontmatter.

    Returns None if the filename doesn't match the expected pattern.
    """
    m = _FILENAME_RE.match(path.name)
    if not m:
        return None

    task_id = m.group(1)
    priority = m.group(2)
    status = m.group(3)
    slug = m.group(4)

    fields = _parse_frontmatter(path)

    return TaskFile(
        path=path,
        id=task_id,
        priority=priority,
        status=status,
        slug=slug,
        fields=fields,
    )


def _parse_frontmatter(path: Path) -> dict[str, str]:
    """Extract YAML frontmatter as flat key-value pairs.

    Uses partition(":") to handle values containing colons correctly.
    E.g., title: "QA Report: Streaming" → key="title", value='"QA Report: Streaming"'
    """
    content = path.read_text(encoding="utf-8")
    if not content.startswith("---\n"):
        return {}

    end = content.find("\n---\n", 4)
    if end == -1:
        return {}

    fields: dict[str, str] = {}
    for line in content[4:end].strip().split("\n"):
        key, sep, value = line.partition(":")
        if sep:
            fields[key.strip()] = value.strip()

    return fields


# ---------------------------------------------------------------------------
# Filename generation (REQ-TM-002)
# ---------------------------------------------------------------------------

def get_expected_filename(task_id: str, priority: str, status: str, slug: str) -> str:
    """Generate the canonical filename for a task. Double-dash before slug."""
    return f"{task_id}-{priority}-{status}--{slug}.md"


# ---------------------------------------------------------------------------
# Validation (REQ-TM-004)
# ---------------------------------------------------------------------------

def validate(tasks_dir: Path | str = "tasks") -> ValidationResult:
    """Validate all task files in a directory.

    Returns a ValidationResult with errors (if any) and file count.
    """
    tasks_dir = Path(tasks_dir)
    result = ValidationResult()

    if not tasks_dir.exists():
        return result  # empty is valid

    files = _task_files(tasks_dir)
    result.file_count = len(files)

    # Per-file checks
    for path in files:
        content = path.read_text(encoding="utf-8")

        # Frontmatter exists
        if not content.startswith("---\n"):
            result.errors.append(f"{path.name}: missing YAML frontmatter (must start with ---)")
            continue

        end = content.find("\n---\n", 4)
        if end == -1:
            result.errors.append(f"{path.name}: malformed YAML frontmatter (no closing ---)")
            continue

        fields = _parse_frontmatter(path)

        # Required fields
        if "status" not in fields:
            result.errors.append(f"{path.name}: missing 'status' field")
        elif fields["status"] not in VALID_STATUSES:
            result.errors.append(
                f"{path.name}: invalid status '{fields['status']}' "
                f"(valid: {', '.join(sorted(VALID_STATUSES))})"
            )

        if "priority" not in fields:
            result.errors.append(f"{path.name}: missing 'priority' field")
        elif fields["priority"] not in VALID_PRIORITIES:
            result.errors.append(
                f"{path.name}: invalid priority '{fields['priority']}' "
                f"(valid: {', '.join(sorted(VALID_PRIORITIES))})"
            )

        if "created" not in fields:
            result.errors.append(f"{path.name}: missing 'created' field")
        elif not _DATE_RE.match(fields["created"]):
            result.errors.append(f"{path.name}: invalid 'created' date format (expected YYYY-MM-DD)")

        if "artifact" not in fields:
            result.errors.append(f"{path.name}: missing 'artifact' field (what file or system change does this task produce?)")
        elif not fields["artifact"]:
            result.errors.append(f"{path.name}: 'artifact' field is empty (must name a concrete output, e.g. a file path, config change, or commit)")

        unknown = sorted(set(fields) - VALID_FIELDS)
        if unknown:
            result.errors.append(
                f"{path.name}: unknown field(s): {', '.join(unknown)} "
                f"(valid: {', '.join(sorted(VALID_FIELDS))})"
            )

        # Filename matches frontmatter
        task = parse_task_file(path)
        if task and fields.get("status") and fields.get("priority"):
            expected = get_expected_filename(
                task.id, fields["priority"], fields["status"], task.slug
            )
            if path.name != expected:
                result.errors.append(
                    f"{path.name}: filename doesn't match frontmatter, expected: {expected}"
                )

    # Duplicate task IDs
    id_map: dict[str, list[str]] = {}
    for path in files:
        task = parse_task_file(path)
        if task:
            id_map.setdefault(task.id, []).append(path.name)

    for tid, filenames in sorted(id_map.items()):
        if len(filenames) > 1:
            result.errors.append(
                f"duplicate task id {tid}: {', '.join(filenames)}"
            )

    return result


# ---------------------------------------------------------------------------
# Fix (REQ-TM-005)
# ---------------------------------------------------------------------------

def fix(tasks_dir: Path | str = "tasks") -> FixResult:
    """Auto-fix task files: inject missing 'created', rename to match frontmatter.

    Also migrates legacy 4-digit (NNNN) filenames to the new AANNN format.
    Returns a FixResult with counts and any errors.
    """
    tasks_dir = Path(tasks_dir)
    result = FixResult()

    if not tasks_dir.exists():
        return result

    files = _task_files(tasks_dir)
    prefix = _prefix_for(tasks_dir)

    for path in files:
        task = parse_task_file(path)
        if not task:
            result.errors.append(f"{path.name}: could not parse file")
            continue

        fields = task.fields

        # Fix missing or malformed 'created'
        if "created" not in fields or not _DATE_RE.match(fields.get("created", "")):
            created = _infer_created_date(path)
            content = path.read_text(encoding="utf-8")

            if re.search(r"^created:.*$", content, re.MULTILINE):
                content = re.sub(
                    r"^created:.*$",
                    f"created: {created}",
                    content,
                    count=1,
                    flags=re.MULTILINE,
                )
            else:
                content = content.replace("---\n", f"---\ncreated: {created}\n", 1)

            path.write_text(content, encoding="utf-8")
            fields["created"] = created
            result.patches.append((path.name, created))
            result.patched += 1

        # Rename to match frontmatter
        if not fields.get("status") or not fields.get("priority"):
            result.errors.append(f"{path.name}: missing status or priority in frontmatter")
            continue

        # Migrate legacy NNNN -> AANNN
        task_id = task.id
        if _is_legacy_id(task_id):
            _, seq = _parse_id_parts(task_id)
            if seq > 999:
                result.errors.append(
                    f"{path.name}: legacy task number {seq} exceeds 999, "
                    "cannot migrate to 3-digit format"
                )
                continue
            task_id = prefix + f"{seq:03d}"
            result.migrated += 1

        expected = get_expected_filename(
            task_id, fields["priority"], fields["status"], task.slug
        )

        if path.name != expected:
            new_path = tasks_dir / expected
            if new_path.exists():
                result.errors.append(f"{path.name}: cannot rename to {expected}, file exists")
                continue

            result.renames.append((path.name, expected))
            path.rename(new_path)
            result.renamed += 1

    return result


# ---------------------------------------------------------------------------
# Next ID (REQ-TM-006)
# ---------------------------------------------------------------------------

def next_id(tasks_dir: Path | str = "tasks") -> str:
    """Return the next available task ID for this tasks directory.

    The ID prefix is derived from the directory's realpath, so different
    worktrees get different prefixes and won't conflict.
    """
    tasks_dir = Path(tasks_dir)
    prefix = _prefix_for(tasks_dir)

    if not tasks_dir.exists():
        return prefix + "001"

    max_seq = 0
    for path in _task_files(tasks_dir):
        task = parse_task_file(path)
        if task:
            pfx, seq = _parse_id_parts(task.id)
            # Count legacy files (will be migrated to this prefix) and same-prefix files
            if pfx == prefix or pfx == "":
                max_seq = max(max_seq, seq)

    return prefix + f"{max_seq + 1:03d}"


# ---------------------------------------------------------------------------
# Init (REQ-TM-007)
# ---------------------------------------------------------------------------

def init(tasks_dir: Path | str = "tasks") -> InitResult:
    """Initialize a tasks directory with a template file.

    Creates the directory and a _TEMPLATE.md file. Fails if the directory
    already exists.
    """
    tasks_dir = Path(tasks_dir)
    result = InitResult(tasks_dir=tasks_dir)

    if tasks_dir.exists():
        result.error = f"tasks directory already exists at {tasks_dir}"
        return result

    tasks_dir.mkdir(parents=True)
    result.created.append(str(tasks_dir) + "/")

    template_path = tasks_dir / "_TEMPLATE.md"
    template_path.write_text(_TEMPLATE_CONTENT, encoding="utf-8")
    result.created.append(str(template_path))

    result.template_fields = sorted(VALID_FIELDS)
    return result


# ---------------------------------------------------------------------------
# Date inference (REQ-TM-005)
# ---------------------------------------------------------------------------

def _infer_created_date(path: Path) -> str:
    """Infer creation date: git log → file mtime → today."""
    # Try git
    try:
        result = subprocess.run(
            ["git", "log", "--follow", "--diff-filter=A", "--format=%as", str(path)],
            capture_output=True,
            text=True,
            timeout=5,
            cwd=path.parent,
        )
        dates = result.stdout.strip().split("\n")
        if dates and dates[-1]:
            return dates[-1]
    except (subprocess.SubprocessError, FileNotFoundError):
        pass

    # Try mtime
    try:
        mtime = path.stat().st_mtime
        return date.fromtimestamp(mtime).isoformat()
    except OSError:
        pass

    # Fallback
    return date.today().isoformat()
