# ADR-001: Constrain gix to Read-Only History Access

- **Status:** Accepted
- **Date:** 2026-08-09
- **Affects:** REQ-TM-006

## Context

Task IDs committed on sibling branches must remain unavailable when their files
are absent from the current checkout. The allocation scan therefore needs the
locally known Git graph: refs, reflogs, commits, and trees. Taskmd must not add a
private counter or allocation ledger whose state can diverge from the repository.

The pure-Rust gix library can traverse this graph without invoking a Git
executable. Its open permissions control configuration and environment inputs,
not repository mutation. `gix::Repository` exposes both read and write methods;
there is no repository-wide read-only mode or read-only repository type.

## Options considered

1. **Maintain taskmd-owned allocation state.** Allocation can be independent of
   Git traversal, but introduces hidden local state, synchronization rules, and
   a second source of truth.
2. **Invoke `git log --all --reflog`.** This is compact and conventionally
   read-only, with Git itself handling graph edge cases. It depends on an
   external executable and delegates behavior to the installed Git version.
3. **Traverse with gix through a constrained internal interface.** This removes
   the executable dependency and makes every operation visible in Rust. It adds
   dependency and binary weight, raises the Rust version floor, and cannot
   enforce read-only access through gix's type system.

## Decision

Adopt option 3. Production allocation uses gix only through the private
`git_history::task_filenames` boundary. The repository handle never escapes that
module, and the implementation limits itself to discovery, ref and reflog
iteration, object lookup, revision walking, and tree reads. It does not call
object, reference, index, worktree, configuration, fetch, or push mutation APIs.

Read-only behavior is a taskmd invariant established by the narrow interface,
code review, and acceptance testing—not a guarantee supplied by gix. The lack of
a repository-wide read-only mode is accepted. A capability-restricted
`ReadOnlyRepository` or equivalent open mode is a potential future contribution
upstream to gitoxide, not a prerequisite for taskmd.

## Consequences

- **Positive:** Allocation has no taskmd-owned ledger and no Git executable
  dependency. The Rust call graph is directly auditable, and differential tests
  compare observable allocation outcomes against Git CLI behavior.
- **Negative:** A future edit inside the private module could call a gix mutation
  method; the compiler cannot prevent it. Gix increases the dependency tree from
  46 to 170 packages, raises MSRV from Rust 1.75 to 1.85, and increased the
  measured CPython wheel from 978 KB to 1.70 MB.
- **Neutral:** `taskmd new` still writes the requested Markdown task file to the
  working tree. This decision concerns mutation of Git's object database, refs,
  reflogs, index, configuration, and HEAD.

## References

- `git_history::task_filenames`
- `ids::next_id`
- `ids::differential_tests`
- [`../taskmd/requirements.md`](../taskmd/requirements.md) — REQ-TM-006
- [gix repository API](https://docs.rs/gix/latest/gix/struct.Repository.html)
- [gix open permissions](https://docs.rs/gix/latest/src/gix/open/permissions.rs.html)
