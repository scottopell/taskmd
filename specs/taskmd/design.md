# task.md — Technical Design

## Architecture Overview (REQ-TM-001, REQ-TM-010)

task.md is a single-file Python CLI tool that operates on a directory of markdown files.
No database, no configuration file, no external dependencies beyond Python’s standard
library. The filesystem IS the database; git IS the audit trail.

```
project/
  tasks/
    _TEMPLATE.md
    0001-p1-done--initial-setup.md
    0002-p2-ready--add-feature.md
    0002-p2-ready--add-feature.qaplan.md
    0003-p3-blocked--waiting-on-api.md
  taskmd.py
```

## Data Model

### Task File (REQ-TM-001, REQ-TM-002, REQ-TM-003)

A task is a markdown file whose identity is encoded in its filename:

```
NNNN-pX-status--slug.md
 |    |    |      |
 |    |    |      +-- kebab-case description
 |    |    +--------- lifecycle state (6 values)
 |    +-------------- priority (p0-p4)
 +------------------- unique 4-digit number (0001-9999)
```

Frontmatter is the source of truth:

```yaml
---
created: 2026-03-04
priority: p2
status: ready
---
```

### Constants (REQ-TM-002, REQ-TM-004)

```python
VALID_STATUSES = {"ready", "in-progress", "blocked", "done", "wont-do", "brainstorming"}
VALID_PRIORITIES = {"p0", "p1", "p2", "p3", "p4"}
FILENAME_PATTERN = r"^(\d{4})-(p[0-4])-(ready|in-progress|blocked|done|wont-do|brainstorming)-(.+)\.md$"
```

### Ancillary Files (REQ-TM-008)

Pattern: `NNNN-pX-status--slug.{qaplan,qareport}.md`

Associated with a task by sharing the same number prefix.
Skipped during validation and fix.
Detection rule: any `.md` file whose stem contains a second dot segment (i.e., the
filename matches `*.*.md` after stripping the `.md` extension).

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
1. Glob `*.md` in task directory (default: `./tasks/`)
2. Skip template file (`_TEMPLATE.md`) and ancillary files (`*.*.md` pattern)
3. For each remaining file: a. Check frontmatter exists and is well-formed b. Check
   required fields present with valid values c. Check `created` matches YYYY-MM-DD
   format d. Parse filename and check it matches frontmatter
4. After all files: check for duplicate task numbers
5. Report errors or success count
6. Exit 0 on success, 1 on errors

### `fix` (REQ-TM-005)

```
taskmd fix [tasks/]
```

Algorithm:
1. Glob `*.md` in task directory (same skip rules as validate)
2. For each file: a. If `created` missing or malformed:
   - Infer date (git log, file mtime, or today — see Date Inference)
   - If `created:` line exists in frontmatter: replace in-place via regex
   - If no `created:` line: insert after opening `---` b. If filename doesn’t match
     frontmatter:
   - Generate expected 4-digit filename
   - If target exists: report conflict, skip
   - Otherwise: rename
3. Report summary (patched N, renamed N)

### `next` (REQ-TM-006)

```
taskmd next [tasks/]
```

Algorithm:
1. Glob `*.md` in task directory (same skip rules)
2. Parse task numbers from all filenames
3. Print `max(numbers) + 1`, zero-padded to 4 digits
4. If no tasks exist, print `0001`

## Date Inference (REQ-TM-005)

When `created` needs to be inferred, try in order:

1. **Git history:** `git log --follow --diff-filter=A --format=%as {path}` — take the
   last line (oldest commit adding the file).
2. **File modification time:** `stat` mtime, converted to YYYY-MM-DD.
3. **Today’s date:** Fallback if both above fail.

Git is optional — date inference degrades gracefully if git is not installed or the file
is not in a git repository.

## Frontmatter Parsing

Simple line-by-line key-value extraction.
Split each line on the FIRST colon only (to handle values containing colons, e.g.,
titles with colons).
No YAML library required — frontmatter uses only flat key-value pairs, never nested
structures.

```python
key, _, value = line.partition(":")
fields[key.strip()] = value.strip()
```

Using `partition` instead of `split` ensures `title: "QA Report: Streaming"` parses
correctly as key=`title`, value=`"QA Report: Streaming"`.

## Body Conventions (REQ-TM-009)

The body after frontmatter is free-form markdown.
Common sections by usage:

| Section | Purpose | Typical use |
| --- | --- | --- |
| `## Summary` | What needs doing | All tasks |
| `## Context` | Why this task exists | All tasks |
| `## Acceptance Criteria` | Checkbox list | All tasks |
| `## Problem` | Bug description | Bug tasks |
| `## Root Cause` | Why the bug exists | Bug tasks |
| `## Reproduction` | Steps to reproduce | Bug tasks |
| `## What to Do` | Implementation steps | Agent tasks |
| `## Files Likely Involved` | Scope hints | Agent tasks |
| `## Dependencies` | Blocking tasks | When relevant |
| `## Notes` | Anything else | Common |

The template demonstrates the recommended structure but does not enforce it.

## Workflow: Changing Task Status (REQ-TM-003)

The canonical workflow for any status change:

1. Edit the `status:` field in the task’s YAML frontmatter
2. Run `taskmd fix` to rename the file to match
3. Commit both the content change and the rename

**Never rename task files directly.** Direct renames create frontmatter/filename
disagreement that validation will catch, but the fix is more work than doing it right in
the first place. Agents should be instructed to edit frontmatter, not rename files.

## File Organization (REQ-TM-007, REQ-TM-010)

```
taskmd.py           # The entire tool — single file, Python stdlib only
tasks/
  _TEMPLATE.md      # Recommended starting point for new tasks
```

No config file, no lockfile, no build step.
Copy `taskmd.py` into any repository and run it.
