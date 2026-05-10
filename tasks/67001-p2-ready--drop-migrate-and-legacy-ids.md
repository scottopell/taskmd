# 1.1: Drop fix --migrate machinery and legacy ID format support

## Why now

The migration code path was the bridge from pre-1.0 (YAML frontmatter,
alpha-prefix `AANNN` and 4-digit `0042` IDs) to 1.0 (filename-only,
all-numeric `DDNNN` IDs). It carried us through the rc1 → 1.0 cut.

Every public consumer has had a full minor version (1.0 line) to run
`taskmd fix --migrate` once. Anyone landing on 1.1 fresh has no
frontmatter to strip and no legacy IDs to renumber. The machinery is
dead weight from this point on.

## What goes

### Rust core (`taskmd-core`)
- `fix::MigrateMode` enum and the `mode` parameter on `fix()`. Signature
  becomes `fix(tasks_dir: &Path) -> FixResult`.
- `fix::has_frontmatter`, `fix::strip_frontmatter` (private), the entire
  prompt-mode branch in `fix()`.
- `FixResult.frontmatter_stripped`, `FixResult.frontmatter_pending`
  fields.
- `fix::fix_summary` `frontmatter_stripped` parameter.
- `ids::is_legacy_id`, `ids::needs_migration` — only callers are the
  migration path inside `fix()`.
- The legacy ID branches in the `FILENAME_PATTERN` regex
  (`[A-HJ-NP-Z]{2}\d{3}` and `\d{4}` alternatives). Pattern collapses
  to just `\d{5}`.
- All migration-only proptests / unit tests in `fix.rs` and `proptests.rs`.

### PyO3 binding (`taskmd-py`)
- `fix_summary` argument list (drop `frontmatter_stripped`).
- `do_fix` `migrate` parameter (drop entirely).
- `is_legacy_id`, `needs_migration` pyfunctions and their module
  registration.
- Frontmatter fields in `do_fix`'s output dict.

### Python wrapper (`src/taskmd`)
- `core.fix()` `migrate` keyword arg.
- `FixResult.frontmatter_stripped`, `FixResult.frontmatter_pending`
  fields.
- `cli.py` `--migrate` / `--no-migrate` flag handling, the
  frontmatter-pending error layout, the `[frontmatter]` bucket in fix
  text output, the per-file `stripped frontmatter:` lines.
- `agent.py` schema entries for `--migrate` / `--no-migrate` under
  `commands.fix.args`. Update `fix.description` to drop the migration
  prose.
- Test classes that exercise migration only:
  `TestFixFrontmatterMigration`, `TestCliFixMigrate`,
  `TestCliFixBuckets::test_fix_text_output_buckets_frontmatter`.

### Docs
- `taskmd-core/README.md` `fix` row: drop the migration parenthetical.
- `CHANGELOG.md`: 1.1 entry listing every removed surface.

## Order of operations (one PR per bullet, or one big PR)

1. Drop legacy regex branches in `FILENAME_PATTERN`. Verify no proptests
   regress.
2. Delete `MigrateMode`, `has_frontmatter`, `strip_frontmatter`, the
   prompt-mode branch in `fix()`. Slim `FixResult`.
3. Delete `is_legacy_id`, `needs_migration`. Inline-delete their
   PyO3 wrappers.
4. PyO3 binding: simplify `do_fix` signature, drop migration fields
   from result dict, drop legacy-ID pyfunctions.
5. Python wrapper: simplify `FixResult` and `fix()`. Drop the CLI flags
   and frontmatter-bucket output.
6. Tests: delete migration test classes; verify the rest still pass.
7. Bump version to 1.1.0. Tag, publish.

## Migration note for users

Anyone still on 1.0 with frontmatter at the time 1.1 ships needs to
run the rc2/1.0 migration first. CHANGELOG will spell this out:

> If you skipped `taskmd fix --migrate` during the 1.0 line, run it
> against your 1.0 install before upgrading to 1.1. 1.1 cannot
> migrate — `fix` will simply error on files it doesn't recognise.

## Done when

- [ ] All listed surface removed from Rust core, PyO3 binding,
      Python wrapper, CLI, schema, and tests.
- [ ] cargo + pytest green.
- [ ] CHANGELOG 1.1.0 entry written.
- [ ] taskmd-core README updated.
- [ ] Version bumped, tag pushed, both registries publish via OIDC.
