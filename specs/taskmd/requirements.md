# task.md — Markdown-Native Task Management

## User Story

As a developer managing a project with LLM agent assistance, I need a task management
system that lives entirely in plain markdown files within my repository so that tasks
are version-controlled, grep-able, agent-friendly, and require no external tooling
beyond a single CLI script.

## Transparency Contract

The user must be able to confidently answer these questions:

**At a glance:**
1. What tasks exist and what’s their status?
2. Which tasks are ready to work on right now?
3. What’s the highest priority open work?

**For any specific task:** 4. When was this task created?
5. What needs to be done?
6. How do I verify it’s complete?
7. Has it been QA’d?

**For project health:** 8. Are there duplicate task numbers?
9. Are all task files consistently named?
10. Can I trust the filenames reflect the actual status?

## Requirements

### REQ-TM-001: View Tasks Without Tooling

WHEN a task is created THE SYSTEM SHALL represent it as a single markdown file in a
designated task directory AND the filename SHALL encode task number, priority, status,
and a human-readable slug

WHEN displaying the task list THE SYSTEM SHALL derive all metadata from filenames alone

**Rationale:** Users need task state visible in `ls` output and git diffs without any
tooling. Agents can create tasks by writing files.
Team members can create tasks simultaneously without conflicts because each task is a
separate file with its own git history.

* * *

### REQ-TM-002: Query Tasks by Status and Priority

THE SYSTEM SHALL enforce the filename format: `NNNN-pX-status--slug.md`

WHERE `NNNN` is a zero-padded 4-digit task number (0001 through 9999) AND `pX` is a
priority level (p0 highest through p4 lowest) AND `status` is one of: ready,
in-progress, blocked, done, wont-do, brainstorming AND `slug` is a kebab-case
description

WHEN a filename does not match this format THE SYSTEM SHALL report a validation error

**Rationale:** Encoding metadata in the filename enables shell-native queries:
`ls tasks/*-ready-*.md` lists actionable tasks, `ls tasks/*-p1-*.md` lists high-priority
items. Sort-by-name groups by task number.
No parsing needed for basic queries.

* * *

### REQ-TM-003: Unambiguous Task State

WHEN a task file is created THE SYSTEM SHALL require YAML frontmatter delimited by `---`
containing at minimum:
- `created`: date in YYYY-MM-DD format
- `priority`: one of p0, p1, p2, p3, p4
- `status`: one of ready, in-progress, blocked, done, wont-do, brainstorming

THE SYSTEM SHALL treat the frontmatter as the source of truth AND SHALL require the
filename to match the frontmatter values

WHEN frontmatter and filename disagree THE SYSTEM SHALL report a validation error AND
the auto-fix command SHALL rename the file to match frontmatter

WHEN a user or agent needs to change task status THE SYSTEM SHALL require editing the
`status:` field in frontmatter and running the fix command — never renaming the file
directly

**Rationale:** Frontmatter is the authoritative record; the filename is a derived
convenience. This ensures the file content is never ambiguous about the task’s state.
Direct file renaming is explicitly unsupported because it creates frontmatter/filename
disagreement.

* * *

### REQ-TM-004: Catch Inconsistencies Before Merge

WHEN the validate command runs THE SYSTEM SHALL check every task file for:
- Well-formed YAML frontmatter (opens and closes with `---`)
- Required fields present with valid values
- Filename matches frontmatter
- No duplicate task numbers across all files

WHEN validation errors exist THE SYSTEM SHALL report each error with the filename and
specific issue AND exit with a non-zero status code

WHEN all files pass validation THE SYSTEM SHALL report the count of validated files and
exit successfully

THE SYSTEM SHALL skip the template file and all ancillary files during validation

**Rationale:** Validation runs as part of CI/pre-commit.
A non-zero exit code blocks merging task files with inconsistent metadata.

* * *

### REQ-TM-005: Repair Common Issues Automatically

WHEN the fix command runs THE SYSTEM SHALL repair files that can be fixed automatically:
- Inject missing `created` field using git history or file modification time
- Replace malformed `created` values in-place (not insert duplicates)
- Rename files to match their frontmatter

WHEN a fix would create a naming conflict (target filename exists) THE SYSTEM SHALL
report the conflict and skip that file

WHEN fix completes THE SYSTEM SHALL report what was changed (patched count, renamed
count)

THE SYSTEM SHALL skip the template file and all ancillary files during fix

**Rationale:** Agents create task files with correct frontmatter but sometimes wrong
filenames or missing creation dates.
Fix resolves this automatically.

* * *

### REQ-TM-006: Discover Next Available Task Number

WHEN the next command runs THE SYSTEM SHALL scan all existing task files and print the
next available task number (one greater than the current maximum)

WHEN no task files exist THE SYSTEM SHALL print 0001

**Rationale:** Every agent and human creating a task needs to know what number to use.
Without this command, parallel agents race on the same number and duplicate detection
catches it after the fact rather than preventing it.

* * *

### REQ-TM-007: Consistent Starting Point for New Tasks

THE SYSTEM SHALL provide a template file that demonstrates the standard frontmatter and
recommended body sections

THE SYSTEM SHALL skip the template file during validation and fix

**Rationale:** Gives humans and agents a starting point.
The template documents the expected structure without being prescriptive about body
content.

* * *

### REQ-TM-008: Associate QA Artifacts with Tasks

THE SYSTEM SHALL support ancillary files associated with a task using dot-segment
patterns in the filename (e.g., `.qaplan.md`, `.qareport.md`)

THE SYSTEM SHALL skip all ancillary files (any `.md` file containing a second dot
segment) during validation and fix

THE SYSTEM SHALL NOT require ancillary files to exist for a task to be valid

**Rationale:** The implement/QA two-agent pattern needs a place for QA plans and reports
that lives alongside the task without polluting the task list.
The dot convention makes them invisible to `ls tasks/*-ready-*.md` while keeping them
adjacent in the filesystem.

* * *

### REQ-TM-009: Self-Contained Agent Prompts

WHEN a task file is created for agent execution THE SYSTEM SHALL support body sections
that serve as self-contained agent prompts:
- “Read first” pointers to spec files or other context
- “What to Do” with explicit steps
- “Acceptance Criteria” with checkable items
- “Files Likely Involved” for scope guidance

THE SYSTEM SHALL NOT enforce body structure — body format is convention, not schema

**Rationale:** Tasks double as agent prompts.
A fresh agent can read the task file and execute it without any other context.
But human-written tasks (bug reports, brainstorming notes) should not be forced into
agent-prompt format.

* * *

### REQ-TM-010: Zero-Friction Adoption

THE SYSTEM SHALL be adoptable by copying a single file into any repository with no
installation, configuration, or build step required

THE SYSTEM SHALL have no dependencies beyond the language’s standard library

THE SYSTEM SHALL exit with code 0 on success and non-zero on failure for CI integration

**Rationale:** The tool must be trivially adoptable.
No package manager, no build step, no configuration file.
It runs in CI alongside linting and testing using standard exit code conventions.
