# task.md — Markdown-Native Task Management

## User Story

As a developer managing a project with LLM agent assistance, I need a task
management system that lives entirely in plain markdown files within my
repository so that tasks are version-controlled, grep-able, agent-friendly, and
require no external tooling beyond a single CLI.

## Transparency Contract

The user must be able to confidently answer these questions:

**At a glance:**
1. What tasks exist and what's their status?
2. Which tasks are ready to work on right now?
3. What's the highest priority open work?

**For any specific task:** 4. What needs to be done?
5. How do I verify it's complete?
6. Has it been QA'd?

**For project health:** 7. Are there duplicate task numbers?
8. Are all task files consistently named?

## Requirements

### REQ-TM-001: View Tasks Without Tooling

WHEN a task is created THE SYSTEM SHALL represent it as a single markdown file
in a designated task directory AND the filename SHALL encode task number,
priority, status, and a human-readable slug

WHEN displaying the task list THE SYSTEM SHALL derive all metadata from
filenames alone

**Rationale:** Users need task state visible in `ls` output and git diffs
without any tooling. Agents can create tasks by writing files. Team members can
create tasks simultaneously without conflicts because each task is a separate
file with its own git history.

* * *

### REQ-TM-002: Query Tasks by Status and Priority

THE SYSTEM SHALL enforce the filename format: `DDNNN-pX-status--slug.md`

WHERE `DDNNN` is a 5-digit numeric ID (DD prefix + NNN sequence) AND `pX` is a
priority level (p0 highest through p4 lowest) AND `status` is one of: ready,
in-progress, blocked, done, wont-do, brainstorming AND `slug` is a kebab-case
description

WHEN a filename does not match this format THE SYSTEM SHALL report a validation
error

**Rationale:** Encoding metadata in the filename enables shell-native queries:
`ls tasks/*-ready-*.md` lists actionable tasks, `ls tasks/*-p1-*.md` lists
high-priority items. Sort-by-name groups by task number. No parsing needed for
basic queries.

* * *

### REQ-TM-003: Unambiguous Task State

THE SYSTEM SHALL treat the filename as the single source of truth for task
metadata (id, priority, status, slug)

THE SYSTEM SHALL NOT require or read YAML frontmatter — task files have no
frontmatter, and the body is free-form markdown

WHEN a user or agent needs to change task status THE SYSTEM SHALL provide a
single command (`taskmd status <id> <new-status>`) that renames the file
atomically

**Rationale:** A single source of truth is unambiguous. Storing metadata in
both the filename and a frontmatter block creates a class of "they disagree"
bugs that adds no value — the filename is already in `ls` output and `git
diff`. Status changes become pure renames, which become single git commits.

* * *

### REQ-TM-004: Catch Inconsistencies Before Merge

WHEN the validate command runs THE SYSTEM SHALL check every task file for:
- Filename matching the canonical pattern
- No duplicate task numbers across all files

WHEN validation errors exist THE SYSTEM SHALL report each error with the
filename and specific issue AND exit with a non-zero status code

WHEN all files pass validation THE SYSTEM SHALL report the count of validated
files and exit successfully

THE SYSTEM SHALL skip the template file and all ancillary files during
validation

**Rationale:** Validation runs as part of CI/pre-commit. A non-zero exit code
blocks merging task files with malformed filenames or duplicate IDs.

* * *

### REQ-TM-005: Repair Common Issues Automatically

WHEN the fix command runs THE SYSTEM SHALL repair files that can be fixed
automatically:
- Migrate legacy ID formats (4-digit `NNNN`, alpha-prefix `AANNN`) to the
  canonical `DDNNN` format by renaming the file
- Renumber files that share a duplicate task ID, picking a winner via
  git-first-seen → mtime → lexicographic filename

WHEN a fix would create a naming conflict (target filename exists) THE SYSTEM
SHALL report the conflict and skip that file

WHEN fix completes THE SYSTEM SHALL report what was changed (renamed count,
migrated count, renumbered count)

THE SYSTEM SHALL skip the template file and all ancillary files during fix

THE SYSTEM SHALL NOT rewrite cross-references to renumbered IDs elsewhere in
the repository — the renumbered mapping is the hand-off so a human can grep and
patch

**Rationale:** Legacy filename migration and duplicate-ID renumbering are
purely mechanical and safe to automate. Cross-reference rewriting touches code
the tool does not understand and is intentionally out of scope.

* * *

### REQ-TM-006: Discover Next Available Task Number

WHEN the next command runs THE SYSTEM SHALL print the next available task
number (one greater than the local-prefix maximum visible in the working tree
or locally known Git history)

WHEN a task ID exists on another locally known Git branch or reflog THE SYSTEM
SHALL treat that ID as unavailable even when its file is absent from the current
checkout

WHEN no task files exist for the local prefix in either the working tree or
locally known Git history THE SYSTEM SHALL print `DD001`

THE SYSTEM SHALL provide a `new` command that allocates an ID, formats the
filename, and writes the file in one atomic step using O_EXCL

**Rationale:** Every agent and human creating a task needs to know what number
to use. A single worktree may create several unrelated branches from the same
base; IDs already committed on sibling branches must not be reused merely
because those task files are hidden by the current checkout. `new` is the
recommended path because it eliminates the race condition between getting an
ID and writing the file. `next` exists for integrations that must do their own
write path.

* * *

### REQ-TM-007: Consistent Starting Point for New Tasks

THE SYSTEM SHALL provide a template file that demonstrates the recommended body
sections

THE SYSTEM SHALL skip the template file during validation and fix

**Rationale:** Gives humans and agents a starting point. The template documents
the expected structure without being prescriptive about body content.

* * *

### REQ-TM-008: Associate QA Artifacts with Tasks

THE SYSTEM SHALL support ancillary files associated with a task using
dot-segment patterns in the filename (e.g., `.qaplan.md`, `.qareport.md`)

THE SYSTEM SHALL skip all ancillary files (any `.md` file containing a second
dot segment) during validation and fix

THE SYSTEM SHALL NOT require ancillary files to exist for a task to be valid

**Rationale:** The implement/QA two-agent pattern needs a place for QA plans
and reports that lives alongside the task without polluting the task list. The
dot convention makes them invisible to `ls tasks/*-ready-*.md` while keeping
them adjacent in the filesystem.

* * *

### REQ-TM-009: Self-Contained Agent Prompts

WHEN a task file is created for agent execution THE SYSTEM SHALL support body
sections that serve as self-contained agent prompts:
- "Read first" pointers to spec files or other context
- "What to Do" with explicit steps
- "Done When" with checkable items
- "Files Likely Involved" for scope guidance

THE SYSTEM SHALL NOT enforce body structure — body format is convention, not
schema

**Rationale:** Tasks double as agent prompts. A fresh agent can read the task
file and execute it without any other context. But human-written tasks (bug
reports, brainstorming notes) should not be forced into agent-prompt format.

* * *

### REQ-TM-010: Zero-Friction Adoption

THE SYSTEM SHALL be installable via `pip install taskmd` (or `uv tool install
taskmd`)

THE SYSTEM SHALL exit with code 0 on success and non-zero on failure for CI
integration

**Rationale:** The tool must be trivially adoptable. Standard Python packaging
means no bespoke install path, and CI exit codes plug into linting and testing
without configuration.
