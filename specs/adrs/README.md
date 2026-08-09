# Architecture Decision Records

This is taskmd's shared, project-level ADR chain. Each accepted record is a
frozen account of one decision: the context, alternatives, choice, and cost.
Requirements remain the normative user-facing contract; ADRs explain why that
contract and implementation took their present shape.

## Quick reference

| ADR | Title | Status | Affects |
| --- | --- | --- | --- |
| [000](000_no-living-design-document.md) | taskmd has no living design document | Accepted | methodology-level |
| [001](001_constrain-gix-to-read-only-history-access.md) | Constrain gix to read-only history access | Accepted | REQ-TM-006 |

## For agents: which decisions bind your task

| Task type | Relevant ADRs |
| --- | --- |
| Adding or reorganizing specifications | 000 |
| Changing task-ID allocation or Git traversal | 001 |
| Introducing taskmd-owned allocation state | 001 |

## Decision dependencies

```text
ADR-000 (no living design document)
   └── ADR-001 records the gix decision in the project-level chain
```

## Conventions

- Number ADRs sequentially across the project: `000`, `001`, and so on.
- Declare scope through `Affects:`, never through directory placement.
- Never rewrite an accepted ADR to match later reality. Supersede it with a new
  record and retain the old one.
- Add every ADR to both tables above where applicable.
