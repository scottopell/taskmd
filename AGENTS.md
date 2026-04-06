# Agent Instructions for task.md

## What Is This?

A markdown-native task management library and CLI. Core logic in Rust (`taskmd-core/`),
exposed to Python via PyO3 (`taskmd-py/`). Python `core.py` is a thin shim.

## Structure

```
taskmd-core/src/   # Rust: all logic (ids, filename, fix, validate, etc.)
taskmd-py/src/     # Rust: PyO3 bindings
src/taskmd/
  __init__.py      # Public API exports
  core.py          # Python shim over Rust _core extension
  cli.py           # CLI wrapper (thin, calls core)
  agent.py         # Agent detection, JSON envelopes, schema
specs/taskmd/      # spEARS spec (requirements, design, executive)
tasks/             # Task tracking (uses task.md itself)
```

## Development

```bash
uv run maturin develop          # Build Rust extension
cargo test -p taskmd-core       # Run Rust tests
uv run pytest tests/            # Run Python tests
taskmd validate                 # Run validation
```

## Task Tracking

This project uses itself for task management.

**Format:** `DDNNN-pX-status--slug.md` (e.g., `34042-p1-ready--fix-bug.md`)

- `DD`: 2-digit prefix (D1 from hostname, D2 from directory path)
- `NNN`: 3-digit sequence number
- `pX`: Priority (p0 highest)
- `status`: ready, in-progress, blocked, done, wont-do, brainstorming

Set `TASKMD_MACHINE_ID=0` to pin D1 on your primary machine.

**To change status:** edit frontmatter `status:` field, then `taskmd fix` to rename
the file to match. You can also rename the file directly if you prefer.

**To create a new task:** `taskmd next` prints the next ID.
Create the file.

## Code Conventions

- Core logic lives in Rust (`taskmd-core/`), Python delegates via `_core`
- Every public function takes `tasks_dir: Path | str` and returns a dataclass
- No printing, no sys.exit in `core.py` -- that's `cli.py`'s job
- Tests use `tmp_path` fixtures with real task files on disk
