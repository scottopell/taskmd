# Changelog

All notable changes to `taskmd` are documented here. The Python package
(`taskmd`) and the Rust core crate (`taskmd-core`) ship in lockstep.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com),
and this project follows [SemVer](https://semver.org/) calibrated by user impact
(see `OPERATIONS.md` for the calibration rules).

## [Unreleased]

### Fixed

- Task creation in a reused Git worktree no longer allocates the same ID on
  unrelated sibling branches created from the same base. Allocation now treats
  task filenames reachable through local refs, remote-tracking refs, and
  reflogs as already used. No taskmd-owned counter or allocation ledger is
  created; non-Git directories retain filesystem-only allocation.

## [1.3.0] — 2026-06-09

### Changed

- Task directory auto-detection now accepts any immediate child directory that
  contains `_TEMPLATE.md` (for example `tickets/`), not only names beginning
  with `task`.

## [1.2.0] — 2026-05-25

### Changed — Rust core (`taskmd-core`)

- Task ID prefix is now `hash(machine_identity, tasks_directory_path) mod 100`
  instead of `D1 = hash(hostname) mod 10` concatenated with
  `D2 = hash(path) mod 10`. The split scheme effectively partitioned same-host
  concurrent worktrees into only 10 buckets (D1 was fixed per machine); the
  combined scheme uses the full 100. Birthday-50% collision point moves from
  ~4 worktrees to ~12 for the same-host case. Cross-machine allocations also
  span 100 buckets now instead of 10 (D1 alone). Same total prefix width
  (2 digits), same total ID length (5 chars). Existing well-formed task IDs
  are never migrated, so old tasks keep their prefixes; only newly minted IDs
  use the new scheme.
- `TASKMD_MACHINE_ID` env var now accepts any string (was: single digit 0-9).
  Its value replaces the hostname in the prefix hash. Single-digit values
  still work but no longer produce a specific D1 — they feed the hash like
  any other string. Anyone who set the env var to pin a specific prefix will
  see a different prefix after upgrade.

## [1.1.0] — 2026-05-15

### Added — Rust core (`taskmd-core`)

- `discover` module: `candidates(dir) -> Vec<String>` and
  `discover(dir) -> Discovery { Found, NotFound, Ambiguous }` locate the
  `_TEMPLATE.md`-marked tasks directory under a directory (the same scan the
  Python CLI does for auto-detect, now a reusable Rust API).
  `discover_or_default(dir) -> PathBuf` applies a never-fails policy: prefer a
  candidate named exactly `tasks`, else the lexically-first candidate, else
  fall back to the bare name `tasks`.
- `constants`: `TEMPLATE_FILENAME` (`"_TEMPLATE.md"`) and
  `DEFAULT_TASKS_DIR_NAME` (`"tasks"`) — the literals discovery and `init`
  were each hard-coding.

### Added — Python (`taskmd`)

- `taskmd.core.discover_tasks_dir(start=".") -> (Path | None, list[str])` and
  `taskmd.core.discover_tasks_dir_or_default(start=".") -> Path`, wrapping the
  Rust `discover` module. The CLI's marker auto-detect now delegates to it
  instead of carrying its own `os.scandir` walk.
- `taskmd._core` exposes `discover_tasks_dir`, `discover_tasks_dir_or_default`,
  and the `TEMPLATE_FILENAME` / `DEFAULT_TASKS_DIR_NAME` constants.

## [1.0.0] — 2026-05-10

The 1.0 line is the first stable API. Everything below was bundled across
`1.0.0-rc1` and `1.0.0-rc2`. Pinning to `>=1.0,<2` is now safe.

### Added — Rust core (`taskmd-core`)

- Typed `Priority { P0..P4 }` and `Status { Ready, InProgress, Brainstorming, Blocked, Done, WontDo }` enums with `as_str()`, `Display`, `FromStr`, and `ALL` constants.
- `ParsedFilename` struct returned by `parse_filename` — replaces the
  4-tuple `(String, String, String, String)`.
- Structured `Error` variants: `InvalidPriority { got }`, `InvalidStatus { got }`,
  `InvalidSlug { got, reason }`, `EmptyBody`, `TasksDirNotFound { path }`,
  `TaskNotFound { id }`, `TargetExists { path }`,
  `IdAllocationExhausted { tasks_dir, tries }`. Display impls build the
  priority/status allow-lists from the enums so they cannot drift.
- `update_task(tasks_dir, id, TaskUpdate { priority, status, slug })` —
  generalises status-only renames to any combination of the three filename
  axes in one atomic rename.
- `ensure_initialized(tasks_dir)` — idempotent counterpart to `init`.
- `find_task_by_slug(tasks_dir, slug) -> Vec<TaskFile>`.
- `ancillary_files_for(tasks_dir, id) -> Vec<PathBuf>` — extension-agnostic.
  Picks up `<task-stem>.<tag>.<ext>` siblings of any extension, so screenshots
  and PDFs follow tasks they're attached to.
- `TaskFile::filename(&self) -> &str` convenience accessor.

### Changed — Rust core (BREAKING)

- `parse_filename` returns `Option<ParsedFilename>` instead of
  `Option<(String, String, String, String)>`.
- `format_filename(id, priority: Priority, status: Status, slug)` — typed
  enum args replace `&str`.
- `TaskFile` flattens with `priority: Priority`, `status: Status` enum fields.
- `create_task` takes `Priority` and `Status` enums instead of `&str`. The
  runtime validation guards are gone — invalid input is unrepresentable.

### Removed — Rust core (BREAKING)

- `rename_status` — replaced by `update_task`. Callers that only changed
  status pass `TaskUpdate { status: Some(s), ..Default::default() }`.
- YAML frontmatter support. Filename is the sole source of truth. The
  one-shot migration code path (`fix --migrate`) remains for now and is
  scheduled for removal in `1.1`.

### Added — Python (`taskmd`)

- `update_task(tasks_dir, task_id, *, priority=None, status=None, slug=None)`.
- `find_task_by_slug`, `ancillary_files_for`, `ensure_initialized`,
  `EnsureResult`, `TaskFile.filename` (property), all bubbled to
  `from taskmd import …`.
- `create_task` and friends now raise `ValueError` on invalid priority/status
  inputs (was `RuntimeError`); runtime failures (not-found, target-exists,
  filesystem errors) still raise `RuntimeError`.

### Removed — Python (BREAKING)

- `taskmd.core.rename_status` and `taskmd._core.rename_status`. Replace with
  `update_task(..., status=new_status)`.

### Added — CLI

- `--tasks-dir P` global flag. Mutually exclusive with the positional
  `tasks_dir`. Matches the `git -C` / `cargo --manifest-path` pattern.
- `taskmd list --slug-contains <substr>` — case-sensitive substring filter.
  Composes with `--status` and `--priority` via intersection.
- `taskmd <subcommand> --help` now prints per-command help in text mode
  (was: always the global help).
- `taskmd fix` text output is bucketed:
  `[frontmatter]` / `[rename]` / `[migrate]` / `[renumber]` sections in
  fixed order. JSON envelope unchanged.

### Changed — CLI (BREAKING)

- `taskmd init` is strict: defaults to `./tasks`, fails loudly when the
  target already exists. The previous silent fallback to `taskmds/` is
  gone — pass an explicit name (`taskmd init taskmds`) to create a
  different directory.
- Read commands (`validate`, `fix`, `list`, `next`, `new`, `status`) auto-detect
  the tasks directory by scanning cwd for direct subdirs that contain a
  `_TEMPLATE.md` marker. Multiple matches now error with both names; zero
  matches now error pointing at `taskmd init`. The previous "first existing of
  `tasks/`/`taskmds/` by name" fallback is gone.
- `taskmd status <id> <new-status>` no-op (target == current) prints
  `<id>: already <status> (no change)` instead of a misleading
  self-rename arrow.

### Tooling

- Rust core (`taskmd-core`) now publishes to crates.io alongside the Python
  package on every `v*` tag. Both registries use OIDC trusted publishing —
  no API tokens stored in the repo.

## [0.4.1] — 2026-04-23

- `taskmd init` falls through to `taskmds/` when `tasks/` is taken (now
  removed in 1.0.0; see above).

## [0.4.0] — 2026-04-22

- Renamed default auto-detect dir from `tasksmd` to `taskmds`.

## [0.3.0] — earlier

- Agent-mode JSON output is now minified by default.

## [0.2.0] — earlier

- Documented the release process.
- `taskmd new` and `taskmd status` subcommands added.

## [0.1.x] — earlier

- Initial PyPI release.
