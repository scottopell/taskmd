# Discover any top-level `_TEMPLATE.md` task directory in taskmd-core

## Problem

After rebasing onto latest `main`, task directory discovery is now centralized in Rust under `taskmd-core/src/discover.rs` and exposed through `taskmd-py`. That is the right layer to change.

The current core discovery still only accepts immediate child directories whose name starts with `task`:

- `taskmd-core/src/discover.rs::candidates` filters with `TASKS_DIR_PREFIX`.
- `taskmd-core/src/constants.rs` defines `TASKS_DIR_PREFIX = "task"`.
- Python and CLI wrappers delegate to Rust discovery, so changing only `src/taskmd/cli.py` would leave library consumers wrong.

This means a valid marker-bearing top-level directory such as `tickets/_TEMPLATE.md`, `work-items/_TEMPLATE.md`, or `plans/_TEMPLATE.md` is ignored.

## Goal

Make `taskmd-core` discover any immediate child directory containing `_TEMPLATE.md`, regardless of directory name. Keep discovery marker-based and top-level only.

## Proposed implementation

1. Update `taskmd-core/src/discover.rs`:
   - Remove the `name.starts_with(TASKS_DIR_PREFIX)` filter from `candidates`.
   - Treat `<child>/_TEMPLATE.md` being a file as the discovery signal.
   - Keep deterministic lexical sorting.
   - Keep symlink behavior if covered by the current implementation/tests.
   - Update module docs and function docs from “task*/_TEMPLATE.md” to “immediate child directory containing `_TEMPLATE.md`”.

2. Update constants/API surface as appropriate:
   - Remove `TASKS_DIR_PREFIX` if no remaining code uses it, or stop exporting/documenting it if backwards compatibility requires leaving it in place temporarily.
   - Update `taskmd-core/README.md`, `CHANGELOG.md`, Python docstrings, CLI help/error text, and agent schema text that currently say auto-detection scans `task*/_TEMPLATE.md`.

3. Update Rust tests in `taskmd-core/src/discover.rs`:
   - Replace the current “non_task_prefixed_marked_dir_is_ignored” expectation with a test that a non-`task*` marked directory is found.
   - Add/confirm that unmarked directories are ignored.
   - Add/confirm ambiguity returns all marker-bearing top-level directories sorted, including non-`task*` names.
   - Preserve `discover_or_default` policy: prefer `tasks` if present, else lexically first marker-bearing candidate, else fallback to `tasks`.

4. Update Python wrapper/CLI tests in `tests/test_core.py`:
   - `discover_tasks_dir` should find a directory like `tickets/` when it has `_TEMPLATE.md`.
   - CLI auto-detect should work for a non-`task*` marked directory.
   - Update any tests currently asserting non-`task*` marked directories are ignored.

## Validation

Run both Rust and Python coverage after rebuilding the extension:

```bash
cargo test -p taskmd-core
uv run maturin develop
uv run pytest tests/
```

## Acceptance criteria

- `taskmd_core::discover::discover(root)` returns `Found("tickets")` for `root/tickets/_TEMPLATE.md` even though `tickets` does not start with `task`.
- `taskmd validate` / `fix` / `next` / `list` auto-detect a single top-level non-`task*` marker-bearing directory through the Python bindings.
- Multiple marker-bearing top-level directories remain ambiguous for CLI/read commands.
- Explicit `--tasks-dir` and positional task directory arguments continue to bypass discovery.
- Discovery does not recurse below immediate children of the scan root.
