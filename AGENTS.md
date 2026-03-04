# Agent Instructions for task.md

## What Is This?

A markdown-native task management library and CLI. Zero dependencies beyond Python stdlib.

## Structure

```
src/taskmd/
  __init__.py    # Public API exports
  core.py        # All logic: validate, fix, next_number, parse_task_file
  cli.py         # CLI wrapper (thin, calls core)
specs/taskmd/    # spEARS spec (requirements, design, executive)
tasks/           # Task tracking (uses task.md itself)
```

## Development

```bash
uv pip install -e .           # Install in dev mode
taskmd validate               # Run validation
python -m pytest tests/       # Run tests
```

## Task Tracking

This project uses itself for task management.

**Format:** `NNNN-pX-status-slug.md` (e.g., `0042-p1-ready-fix-bug.md`)

- `NNNN`: 4-digit task number
- `pX`: Priority (p0 highest)
- `status`: ready, in-progress, blocked, done, wont-do, brainstorming

**To change status:** edit frontmatter `status:` field, then `taskmd fix`.
Never rename task files directly.

**To create a new task:** `taskmd next` prints the next number. Create the file.

## Code Conventions

- Single module (`core.py`) contains all logic
- Every public function takes `tasks_dir: Path | str` and returns a dataclass
- No printing, no sys.exit in `core.py` — that's `cli.py`'s job
- Tests use `tmp_path` fixtures with real task files on disk
