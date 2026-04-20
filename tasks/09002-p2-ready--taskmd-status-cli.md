---
created: 2026-04-20
priority: p2
status: ready
artifact: taskmd-core/src/tasks.rs
---

# `taskmd status <id> <new-status>` — atomic status transitions

## Summary

Expose the existing `tasks::rename_status` function as a first-class CLI verb: `taskmd status <id> <new-status>`. Agents should never need to hand-edit frontmatter or rename a task file to move it through its lifecycle.

## Context

`taskmd new` removed the two-step "call `taskmd next`, then hand-craft the filename" ritual for creation and replaced it with one atomic command. Status transitions still require the old two-step ritual: edit the `status:` field in YAML frontmatter, then `taskmd fix` to rename the file to match (or rename the file by hand and let `fix` normalize the frontmatter). This leaves the same shape of sharp edge `taskmd new` was designed to eliminate — an agent who's learned "don't hand-craft filenames for creation" happily hand-edits frontmatter and renames files when moving a task to `in-progress` or `done`, because that's what the workflow currently requires.

The symmetric fix already exists in the Rust core: `taskmd-core/src/tasks.rs::rename_status` updates the frontmatter, renames the file, validates the new status, and refuses to clobber an existing target. It's exposed through the PyO3 binding as `_rename_status` and the Python public API as `core.rename_status`. The only missing piece is the CLI verb.

## Done When

- [ ] `taskmd status <id> <new-status> [tasks_dir]` added to `src/taskmd/cli.py`.
- [ ] Errors clearly (human + JSON) when: the task ID doesn't exist, the new status is invalid, the target filename already exists, or the tasks directory is missing.
- [ ] Human-mode output: `old-filename.md -> new-filename.md` (mirrors `taskmd fix`'s rename format).
- [ ] JSON-mode output: `{"status":"success","command":"status","data":{"id","old_filename","new_filename","old_status","new_status"}}`.
- [ ] `--help` text lists `status` under Commands with a short description.
- [ ] `--agent` schema entry under `commands.status` with args, description, output shape, and examples. Marked as the recommended way to change status (parallel to `new`'s description).
- [ ] At least one agent-mode workflow updated to use `taskmd status` instead of "edit frontmatter then run `taskmd fix`". The "edit frontmatter" flow can stay as a secondary, documented alternative — it is not wrong, just not preferred.
- [ ] Integration test(s) in `tests/test_core.py` or a new CLI-level test file exercising: happy path, invalid status, unknown ID, and conflict (target filename exists — e.g. an ancillary task with the same slug).
- [ ] README and AGENTS.md updated to mention `taskmd status` as the preferred way to change status, paralleling how they currently recommend `taskmd new` for creation.

## Notes

- The Rust primitive is `taskmd-core/src/tasks.rs::rename_status` — takes `(tasks_dir, id, new_status)`, returns `(old_filename, new_filename)`, errors as `Error::NotFound` / `Error::InvalidValue` / `Error::Conflict`. Reuse it as-is; no core changes needed.
- Priority of the task is read from frontmatter (not filename) when computing the new filename — see the existing Rust test `rename_status_uses_frontmatter_priority`. Don't re-derive priority from the old filename in the CLI layer.
- Naming: `status` is the most direct verb. Alternatives considered: `set-status`, `move`, `transition`. `status` wins on brevity and reads naturally (`taskmd status 34042 done`). It does collide conceptually with a future "query" meaning of `status`, but a mutation that takes an argument is unambiguous from context.
- Don't add `taskmd set <id> <field> <value>` generically — out of scope. Status is the mutation agents need atomically; other fields can wait.
- Mirror the "discouraged" framing used for `taskmd next`: don't fully remove the "edit frontmatter + taskmd fix" path (it's a useful escape hatch), just steer agents away from it by default.
