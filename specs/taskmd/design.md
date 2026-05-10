# task.md — Technical Design

## Architecture Overview (REQ-TM-001, REQ-TM-010)

task.md is a CLI tool (Python wrapper over a Rust core) that operates on a
directory of markdown files. No database, no configuration file. The filesystem
IS the database; git IS the audit trail.

```
project/
  tasks/
    _TEMPLATE.md
    34001-p1-done--initial-setup.md
    34002-p2-ready--add-feature.md
    34002-p2-ready--add-feature.qaplan.md
    34003-p3-blocked--waiting-on-api.md
```

## Data Model

### Task File (REQ-TM-001, REQ-TM-002, REQ-TM-003)

A task is a markdown file whose identity is encoded **entirely in its filename**:

```
DDNNN-pX-status--slug.md
 |     |    |      |
 |     |    |      +-- kebab-case description
 |     |    +--------- lifecycle state (6 values)
 |     +-------------- priority (p0-p4)
 +-------------------- 5-digit ID (DD prefix + NNN sequence)
```

The body is free-form markdown. There is no YAML frontmatter — every piece of
structured metadata lives in the filename. Adding new structured fields requires
extending the filename grammar, not bolting on a side channel.

### Constants (REQ-TM-002)

```rust
VALID_STATUSES = ["blocked", "brainstorming", "done", "in-progress", "ready", "wont-do"]
VALID_PRIORITIES = ["p0", "p1", "p2", "p3", "p4"]
```

### Ancillary Files (REQ-TM-008)

Pattern: `DDNNN-pX-status--slug.{qaplan,qareport}.md`

Associated with a task by sharing the same number prefix. Skipped during
validation and fix. Detection rule: any `.md` file whose stem contains a second
dot segment (i.e., the filename matches `*.*.md` after stripping the `.md`
extension).

### Status Definitions

| Status | Meaning |
| --- | --- |
| `ready` | Prepared and ready to start |
| `in-progress` | Currently being worked on |
| `blocked` | Cannot proceed — external dependency, decision needed, or waiting |
| `done` | Complete |
| `wont-do` | Decided not to implement |
| `brainstorming` | Early exploration, not yet actionable |

## CLI Commands

### `validate` (REQ-TM-004)

```
taskmd validate [tasks/]
```

Algorithm:
1. Glob `*.md` in task directory (default: `./tasks/` or `./taskmds/`)
2. Skip template file (`_TEMPLATE.md`) and ancillary files (`*.*.md` pattern)
3. For each remaining file: check the filename matches the canonical pattern
4. After all files: check for duplicate task numbers
5. Report errors or success count
6. Exit 0 on success, 1 on errors

### `fix` (REQ-TM-005)

```
taskmd fix [tasks/]
```

Algorithm:
1. Glob `*.md` in task directory (same skip rules as validate)
2. For each file with a legacy ID format (`NNNN` or `AANNN`), migrate to the
   canonical `DDNNN` format and rename the file
3. For each pair of files sharing the same parsed ID, pick a winner
   (git-first-seen → mtime → lexicographic), renumber the loser via `next_id`,
   and report the mapping
4. Report summary (renamed N, migrated N, renumbered N)

Cross-references to renumbered IDs elsewhere in the repo are **not** rewritten.
The `renumbered` list is the hand-off so a human can grep and patch.

### `next` (REQ-TM-006)

```
taskmd next [tasks/]
```

Algorithm:
1. Glob `*.md` in task directory (same skip rules)
2. Parse task numbers from all filenames whose prefix matches the local prefix
3. Print `max(numbers) + 1`, formatted as DDNNN
4. If no tasks exist for this prefix, print `DD001`

### `new` (REQ-TM-006)

```
echo "body text" | taskmd new --slug fix-login [--priority p2] [--status ready] [tasks/]
```

Algorithm:
1. Validate inputs (slug present, priority/status valid, body non-empty)
2. Loop with O_EXCL writes:
   - Compute `next_id`
   - Format filename
   - Open with `create_new(true)` and write the body verbatim
   - On collision (concurrent claimer), recompute `next_id` and retry
3. Return the created path

The body is written to disk exactly as supplied (with a trailing newline). No
frontmatter is synthesized.

### `status` (REQ-TM-003)

```
taskmd status <id> <new-status> [tasks/]
```

Algorithm:
1. Validate the new status
2. Look up the task by ID
3. Format the new filename with the new status
4. Refuse to clobber an existing target
5. `std::fs::rename` the file

A status change is therefore a single rename — and a single git commit if
the user is tracking the directory.

## Body Conventions (REQ-TM-009)

The body of a task file is free-form markdown. The template demonstrates a
recommended structure (Summary, Context, Done When, Notes) but does not enforce
it. Body format is convention, not schema.

## File Organization (REQ-TM-007, REQ-TM-010)

```
taskmd-core/        # Rust: all logic
taskmd-py/          # Rust: PyO3 bindings
src/taskmd/         # Python shim + CLI
tasks/
  _TEMPLATE.md      # Recommended starting point for new tasks
```
