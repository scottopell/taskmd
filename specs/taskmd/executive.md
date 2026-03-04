# task.md — Executive Summary

## Requirements Summary

task.md is a markdown-native task management system for developer projects.
Each task is a single markdown file in a `tasks/` directory with metadata encoded in the
filename (`NNNN-pX-status--slug.md`) and YAML frontmatter as the source of truth.
Six statuses (ready, in-progress, blocked, done, wont-do, brainstorming) and five
priority levels (p0 highest through p4 lowest) cover the full task lifecycle.
Validation enforces consistency between filenames and frontmatter, detects duplicate
task numbers, and checks required fields.
Auto-fix repairs common issues without manual intervention.
A `next` command prints the next available task number for agents and humans.
Ancillary files (QA plans, QA reports) live alongside tasks using a dot convention.
The entire system is a single Python script with no external dependencies.

## Technical Summary

Single-file Python CLI (`taskmd.py`), stdlib only.
No database — the filesystem is the data store, git is the audit trail.
Frontmatter parsing uses `partition(":")` for colon-safe key-value extraction.
Filename format: `NNNN-pX-status--slug.md` with double-dash separating status from slug
for visual clarity. Date inference falls through git log, file mtime, then today’s date.
Ancillary file detection uses a second-dot-segment pattern to skip `.qaplan.md` and
`.qareport.md` consistently in both validate and fix.

## Status Summary

| Requirement | Status | Notes |
| --- | --- | --- |
| **REQ-TM-001:** View Tasks Without Tooling | ❌ Not Started | File-per-task, metadata in filename |
| **REQ-TM-002:** Query Tasks by Status and Priority | ❌ Not Started | `NNNN-pX-status--slug.md` format |
| **REQ-TM-003:** Unambiguous Task State | ❌ Not Started | Frontmatter as source of truth |
| **REQ-TM-004:** Catch Inconsistencies Before Merge | ❌ Not Started | validate command |
| **REQ-TM-005:** Repair Common Issues Automatically | ❌ Not Started | fix command |
| **REQ-TM-006:** Discover Next Available Task Number | ❌ Not Started | next command |
| **REQ-TM-007:** Consistent Starting Point | ❌ Not Started | _TEMPLATE.md |
| **REQ-TM-008:** Associate QA Artifacts with Tasks | ❌ Not Started | `.qaplan.md`, `.qareport.md` |
| **REQ-TM-009:** Self-Contained Agent Prompts | ❌ Not Started | Convention, not schema |
| **REQ-TM-010:** Zero-Friction Adoption | ❌ Not Started | Single file, stdlib only, CI exit codes |

**Progress:** 0 of 10 complete
