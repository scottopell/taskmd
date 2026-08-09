# ADR-000: taskmd Has No Living Design Document

- **Status:** Accepted
- **Date:** 2026-08-09
- **Affects:** methodology-level

## Context

Taskmd's original spEARS layout paired timeless requirements and current status
with `specs/taskmd/design.md`, a prose description of the current architecture,
data model, and command algorithms. That file had to change whenever the code
changed, but nothing could mechanically detect when it drifted. It mixed
standing behavior with implementation details and design rationale.

spEARS v2 separates information by its relationship to time. Requirements hold
the timeless user-facing contract, the executive document holds current status,
ADRs preserve point-in-time reasoning, and Allium is available on demand when
behavior needs a checked formal model.

## Options considered

1. **Keep the living design document.** It gives readers one architectural
   overview, but creates a permanent synchronization obligation with no
   reliable drift detector.
2. **Delete design documentation and rely entirely on code.** This removes the
   stale mirror, but loses the reasoning behind consequential choices.
3. **Adopt the spEARS v2 artifact model.** Remove the living design document,
   preserve user-facing behavior in requirements, record consequential choices
   in one project-level ADR chain, and add Allium only when behavioral
   complexity justifies it.

## Decision

Adopt option 3. Taskmd has no living `design.md`. `requirements.md` is the
normative user-facing contract, `executive.md` reports current status, and
`specs/adrs/` is the authoritative history of design decisions. Current
implementation details remain in code and tests. No Allium layer is introduced
by this migration because the present task-file and CLI behavior is adequately
specified by the requirements and acceptance tests.

## Consequences

- **Positive:** There is no hand-maintained prose mirror of current code to
  drift silently. Design reasoning becomes a navigable, immutable chain.
- **Negative:** Readers seeking a complete implementation tour must follow code,
  tests, and relevant ADRs rather than opening one architecture document.
- **Neutral:** Allium remains available for future stateful or cross-boundary
  behavior; its absence is deliberate rather than a migration gap.

## References

- [`../taskmd/requirements.md`](../taskmd/requirements.md)
- [`../taskmd/executive.md`](../taskmd/executive.md)
- [`_TEMPLATE.md`](_TEMPLATE.md)
