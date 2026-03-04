"""taskmd core library — pure functions, no CLI concerns.

REQ-TM-001 through REQ-TM-010. See specs/taskmd/requirements.md.

Every public function operates on a Path to a tasks directory and returns
structured results. No global state, no printing, no sys.exit.
"""

from __future__ import annotations

import re
import subprocess
from dataclasses import dataclass, field
from datetime import date
from pathlib import Path

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

# Double-dash separates status from slug for visual clarity:
#   0042-p2-ready--fix-the-bug.md
_FILENAME_RE = re.compile(
    r"^(\d{4})-(p[0-4])-("
    + "|".join(sorted(VALID_STATUSES))
    + r")--(.+)\.md$"
)

_DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")


# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------

@dataclass
class TaskFile:
    """Parsed representation of a task file."""

    path: Path
    number: int
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
        return ", ".join(parts).capitalize()


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
# Parsing (REQ-TM-001, REQ-TM-003)
# ---------------------------------------------------------------------------

def parse_task_file(path: Path) -> TaskFile | None:
    """Parse a task file's filename and frontmatter.

    Returns None if the filename doesn't match the expected pattern.
    """
    m = _FILENAME_RE.match(path.name)
    if not m:
        return None

    number = int(m.group(1))
    priority = m.group(2)
    status = m.group(3)
    slug = m.group(4)

    fields = _parse_frontmatter(path)

    return TaskFile(
        path=path,
        number=number,
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

def get_expected_filename(number: int, priority: str, status: str, slug: str) -> str:
    """Generate the canonical filename for a task. Always 4-digit, double-dash before slug."""
    return f"{number:04d}-{priority}-{status}--{slug}.md"


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

        # Filename matches frontmatter
        task = parse_task_file(path)
        if task and fields.get("status") and fields.get("priority"):
            expected = get_expected_filename(
                task.number, fields["priority"], fields["status"], task.slug
            )
            if path.name != expected:
                result.errors.append(
                    f"{path.name}: filename doesn't match frontmatter, expected: {expected}"
                )

    # Duplicate task numbers
    number_map: dict[int, list[str]] = {}
    for path in files:
        task = parse_task_file(path)
        if task:
            number_map.setdefault(task.number, []).append(path.name)

    for num, filenames in sorted(number_map.items()):
        if len(filenames) > 1:
            result.errors.append(
                f"duplicate task number {num}: {', '.join(filenames)}"
            )

    return result


# ---------------------------------------------------------------------------
# Fix (REQ-TM-005)
# ---------------------------------------------------------------------------

def fix(tasks_dir: Path | str = "tasks") -> FixResult:
    """Auto-fix task files: inject missing 'created', rename to match frontmatter.

    Returns a FixResult with counts and any errors.
    """
    tasks_dir = Path(tasks_dir)
    result = FixResult()

    if not tasks_dir.exists():
        return result

    files = _task_files(tasks_dir)

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
            result.patched += 1

        # Rename to match frontmatter
        if not fields.get("status") or not fields.get("priority"):
            result.errors.append(f"{path.name}: missing status or priority in frontmatter")
            continue

        expected = get_expected_filename(
            task.number, fields["priority"], fields["status"], task.slug
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
# Next number (REQ-TM-006)
# ---------------------------------------------------------------------------

def next_number(tasks_dir: Path | str = "tasks") -> int:
    """Return the next available task number (max existing + 1)."""
    tasks_dir = Path(tasks_dir)

    if not tasks_dir.exists():
        return 1

    numbers: list[int] = []
    for path in _task_files(tasks_dir):
        task = parse_task_file(path)
        if task:
            numbers.append(task.number)

    if not numbers:
        return 1

    return max(numbers) + 1


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
