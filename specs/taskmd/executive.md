# task.md — Executive Summary

## Requirements Summary

task.md is a markdown-native task management system for developer projects.
Each task is a single markdown file in a `tasks/` directory; the filename
(`DDNNN-pX-status--slug.md`) is the sole source of truth for task metadata.
The body is free-form markdown — there is no YAML frontmatter. Six statuses
(ready, in-progress, blocked, done, wont-do, brainstorming) and five priority
levels (p0 highest through p4 lowest) cover the full task lifecycle. Validation
checks filenames for the canonical pattern and detects duplicate task numbers.
Auto-fix migrates legacy ID formats and renumbers duplicate IDs without manual
intervention. A `next` command prints the next available task number, and
`new` allocates an ID and writes the file in one atomic step. IDs committed in
locally known Git history remain reserved across sibling branch checkouts.
Ancillary files (QA plans, QA reports) live alongside tasks using a dot
convention.

## Technical Summary

Python CLI wrapping a Rust core (taskmd-core) via PyO3. No database — the
filesystem is the data store, git is the audit trail. The filename grammar
(`DDNNN-pX-status--slug.md`) is the only metadata schema; tasks have no
frontmatter and the body is opaque. Status transitions are pure file renames.
Ancillary file detection uses a second-dot-segment pattern to skip `.qaplan.md`
and `.qareport.md` consistently in both validate and fix. Duplicate-ID
renumbering picks a winner via git-first-seen → mtime → lexicographic filename.
ID allocation unions working-tree filenames with filenames reachable from Git
refs and reflogs, falling back to the working tree outside Git.

## Status Summary

| Requirement | Status | Notes |
| --- | --- | --- |
| **REQ-TM-001:** View Tasks Without Tooling | ❌ Not Started | File-per-task, metadata in filename |
| **REQ-TM-002:** Query Tasks by Status and Priority | ❌ Not Started | `NNNN-pX-status--slug.md` format |
| **REQ-TM-003:** Unambiguous Task State | ❌ Not Started | Filename is the sole source of truth (no frontmatter) |
| **REQ-TM-004:** Catch Inconsistencies Before Merge | ❌ Not Started | validate command |
| **REQ-TM-005:** Repair Common Issues Automatically | ❌ Not Started | fix command |
| **REQ-TM-006:** Discover Next Available Task Number | ✅ Complete | Working tree + locally known Git history; Rust and Python suites pass |
| **REQ-TM-007:** Consistent Starting Point | ❌ Not Started | _TEMPLATE.md |
| **REQ-TM-008:** Associate QA Artifacts with Tasks | ❌ Not Started | `.qaplan.md`, `.qareport.md` |
| **REQ-TM-009:** Self-Contained Agent Prompts | ❌ Not Started | Convention, not schema |
| **REQ-TM-010:** Zero-Friction Adoption | ❌ Not Started | Single file, stdlib only, CI exit codes |

**Progress:** 1 of 10 complete
