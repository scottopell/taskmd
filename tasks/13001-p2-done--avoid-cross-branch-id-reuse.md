# Prevent task ID reuse across sibling branches

Git branches created from the same base in one reused worktree must not allocate the same task ID merely because task files committed on sibling branches are absent from the current checkout.

Track GitHub issue #15. Preserve stateless operation: Git history may be consulted, but taskmd must not maintain a hidden local counter or allocation ledger.

## Done when

- [x] Allocation considers task filenames reachable from local refs, remote-tracking refs, and reflogs.
- [x] Non-Git directories retain filesystem-only allocation.
- [x] A regression test reproduces sequential sibling branches from one base and proves distinct IDs.
- [x] Rust and Python tests pass.
