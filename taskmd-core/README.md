# taskmd-core

Pure-Rust core for [`taskmd`](https://github.com/scottopell/taskmd) — a
markdown-native task manager where the **filename is the sole source of truth**
for task metadata (id, priority, status, slug). Task bodies are free-form
markdown.

The `taskmd` CLI (shipped as a Python package via PyO3) is a thin wrapper over
this crate. Use this crate directly if you want to build your own integration
(editor plugin, web UI, custom CLI) on the same on-disk format.

## Filename grammar

```
<id>-<priority>-<status>--<slug>.md
00042-p2-ready--fix-token-expiry-check.md
```

- `id`: 5 ASCII digits, prefixed by a 2-digit machine+directory hash so multiple
  checkouts on different machines don't collide. Sequence allocation includes
  locally known Git refs and reflogs, preventing reuse across sibling branches
  created in one worktree without a separate allocation ledger.
- `priority`: one of `p0`, `p1`, `p2`, `p3`, `p4`.
- `status`: one of `new`, `ready`, `in-progress`, `blocked`, `done`, `cancelled`.
- `slug`: `[a-z0-9-]+`, ≤ 40 chars, derived from the title.

## Quick example

```rust
use std::path::Path;
use taskmd_core::{create::create_task, tasks::list_tasks, validate::validate};

let dir = Path::new("./tasks");

let created = create_task(dir, "p2", "new", "fix-login-bug", "Repro steps...\n")?;
println!("created {} at {}", created.id, created.filename);

for task in list_tasks(dir) {
    println!("{} [{}] {}", task.id, task.status, task.slug);
}

let report = validate(dir);
assert!(report.ok(), "{:?}", report.errors);
# Ok::<(), taskmd_core::error::Error>(())
```

## Public API

Modules — full docs at [docs.rs/taskmd-core](https://docs.rs/taskmd-core):

| Module      | What it does                                                                  |
|-------------|-------------------------------------------------------------------------------|
| `constants` | `VALID_STATUSES`, `VALID_PRIORITIES`, `TEMPLATE_FILENAME`, `DEFAULT_TASKS_DIR_NAME`. |
| `filename`  | `parse_filename`, `format_filename`, `derive_slug`, `MAX_SLUG_LEN`.            |
| `ids`       | `next_id`, `prefix_for`, `parse_id_parts`, legacy-ID detection.                |
| `tasks`     | `TaskFile`, `list_tasks`, `find_task_by_id`, `find_task_by_slug`, `update_task`, `ancillary_files_for`. |
| `create`    | `create_task` — atomic ID-allocate + write.                                    |
| `discover`  | `candidates` / `discover` — find immediate child dirs containing `_TEMPLATE.md`. `discover_or_default` — never-fails policy (prefer `tasks`, else lexically-first, else fall back to `tasks`). |
| `init`      | `init` — scaffold a fresh tasks directory. `ensure_initialized` — idempotent variant. |
| `validate`  | `validate` — check filename conformance and ID uniqueness.                     |
| `fix`       | `fix` — auto-rename non-conforming files, renumber duplicates, plus a one-shot legacy-format migration (slated for removal in 1.1). |
| `error`     | `Error` enum: `Io`, `InvalidPriority`, `InvalidStatus`, `InvalidSlug`, `EmptyBody`, `TasksDirNotFound`, `TaskNotFound`, `TargetExists`, `IdAllocationExhausted`. |

All filesystem operations take a `&Path` to the tasks directory. The library
does not assume a current working directory; callers resolve paths.

## Stability

Pre-1.0. Public API may shift. Pin exact versions until 1.0.0.

## License

MIT
