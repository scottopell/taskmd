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
ID allocation reads working-tree filenames and locally known Git history through
gix, falling back to the working tree outside Git. The project has no living
design document; significant rationale lives in the shared
[`specs/adrs/`](../adrs/README.md) chain.

## Status Summary

| Requirement | Status | Notes |
| --- | --- | --- |
| **REQ-TM-001:** View Tasks Without Tooling | ✅ Complete | File-per-task storage; list and discovery tests |
| **REQ-TM-002:** Query Tasks by Status and Priority | ✅ Complete | Filename parser and round-trip property tests |
| **REQ-TM-003:** Unambiguous Task State | ✅ Complete | Status/update tests; no-frontmatter creation contract |
| **REQ-TM-004:** Catch Inconsistencies Before Merge | ✅ Complete | Validation unit and property tests |
| **REQ-TM-005:** Repair Common Issues Automatically | ✅ Complete | Fix, migration, collision, and idempotence tests |
| **REQ-TM-006:** Discover Next Available Task Number | ✅ Complete | Differential Git-history fixtures plus Rust and Python suites |
| **REQ-TM-007:** Consistent Starting Point | ✅ Complete | Init and template tests |
| **REQ-TM-008:** Associate QA Artifacts with Tasks | ✅ Complete | Ancillary-file discovery and exclusion tests |
| **REQ-TM-009:** Self-Contained Agent Prompts | ✅ Complete | Free-form body contract and repository task template |
| **REQ-TM-010:** Zero-Friction Adoption | ✅ Complete | Maturin wheel build and CLI exit-code tests |

**Progress:** 10 of 10 complete

## Open Questions & Future Directions

- If read-only repository traversal becomes a recurring need across gitoxide
  consumers, propose a capability-restricted repository API upstream. Taskmd's
  private gix boundary remains the local safeguard unless such an API emerges.
