---
created: 2026-04-20
priority: p2
status: done
artifact: taskmd-core/src/fix.rs
---
# `taskmd fix` should auto-renumber duplicate task IDs

## Summary

When two task files share the same numeric ID (same DDNNN prefix+sequence but different slugs/statuses), `taskmd validate` already flags it, but recovery is currently manual: rename one file, edit its frontmatter, and grep commits/cross-links for the old ID. Fold this into `taskmd fix` so the common case is one command.

## Context

Observed in the wild in the phoenix-ide worktree: two agents in the same `(D1, D2)` partition both got sequence `663`, each wrote a distinct file, and filesystem-level uniqueness kept everything quiet until the ID collision was noticed later. See GitHub issue #7 for the full narrative.

The `taskmd new` command (separate PR) is the happy path for new task creation, but:
- Agents still observe on-disk filename patterns via `ls` and mimic them, bypassing `new`.
- Stale-knowledge scenarios (two worktrees sharing a tasks dir, one hasn't pulled) can still produce dupes even with `new`.

So the escape hatch matters. Make the fix trivial.

## Done When

- [ ] `fix` detects two or more task files sharing the same `(prefix, sequence)` parsed ID.
- [ ] A tiebreaker picks a "winner" (keeps original ID) and one or more "losers" (get renumbered). Proposed tiebreaker: earliest git-first-seen date wins; fall back to mtime; fall back to lexicographic filename. Document the chosen rule.
- [ ] Each loser is assigned a fresh ID via `next_id`, its filename is renamed, and its frontmatter stays internally consistent (no ID in frontmatter today, but re-check after migration).
- [ ] `FixResult` grows a `renumbered: Vec<(old_id, new_id, old_filename, new_filename)>` field surfaced in both text and JSON output, so a human can grep commits and cross-links for the old ID.
- [ ] `validate`'s duplicate-ID error message points at `taskmd fix` as the remediation.
- [ ] Tests: two dupes, three dupes, dupes across priorities/statuses, tiebreaker ordering, validate-suggests-fix wording.

## Notes

- Cross-reference repair is explicitly **out of scope**. The mapping in the output is the user's lead; they grep and patch themselves.
- This is the "make recovery easy" counterpart to `taskmd new`'s "make the happy path easy." Together they bracket the problem.
- Follow-up question once implemented: should `fix` renumber by default, or behind a flag (`--renumber-duplicates`)? Default-on is friendlier; flag-gated is safer for tooling that runs fix in CI. Decide after dogfooding.
