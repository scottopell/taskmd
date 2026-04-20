# Operations

Release, install, and maintenance procedures for `taskmd`.

## Cutting a release

Releases are tag-triggered. Pushing any tag matching `v*` runs
`.github/workflows/publish.yml`, which builds wheels on Linux/macOS/Windows,
builds an sdist, and publishes everything to PyPI via trusted publishing
(no token stored in the repo — the `pypi` GitHub environment is the trust
anchor).

```bash
# 1. Pick the version. See "Versioning" below.
NEW=0.2.0

# 2. Bump pyproject.toml. Cargo manifests are NOT bumped — the two Rust
#    crates are internal and never published to crates.io.
sed -i '' "s/^version = \".*\"/version = \"$NEW\"/" pyproject.toml

# 3. Commit directly to main.
git add pyproject.toml
git commit -m "chore: bump version to $NEW"
git push

# 4. Tag and push the tag. This triggers the publish workflow.
git tag v$NEW
git push origin v$NEW

# 5. Watch it go green.
gh run watch
```

Release is done when PyPI shows the new version
(https://pypi.org/project/taskmd/) and the workflow is green.

### Versioning

Semver-ish. The surface area is small, so calibrate mostly by user impact:

- **Patch** (`0.2.0 -> 0.2.1`): bug fixes, doc-only changes, internal
  refactors, performance work. No behavior change users would notice.
- **Minor** (`0.2.0 -> 0.3.0`): new CLI verb, new flag, new JSON field,
  anything agents or humans can newly depend on.
- **Major** (`0.x.y -> 1.0.0`): breaking CLI/API change — renamed or
  removed commands, changed JSON envelope shape, non-backwards-compatible
  frontmatter requirements. Don't ship one without a migration note in
  the release commit body.

If multiple classes of change landed since the last tag, use the highest
one. Skim `git log v<prev>..main` to categorize.

### Recovering from a bad release

If the workflow publishes a broken version to PyPI:

1. **Yank, don't delete.** On PyPI, yank the bad release so `uv tool install`
   / `pip install` won't resolve to it but existing pins still work.
2. Fix the bug on main, bump to the next patch version, tag, publish.
3. Never re-tag an existing version — PyPI rejects re-uploads of the same
   filename, and yanked versions cannot be replaced.

## Installing / upgrading

### As an end user (recommended)

```bash
uv tool install taskmd       # first time
uv tool upgrade taskmd       # after a new release
uv tool uninstall taskmd     # remove
```

`uv tool` installs into an isolated virtualenv and exposes the `taskmd`
binary on `PATH`. This is what goes on a dev machine.

### From a local checkout (dev loop)

```bash
uv run maturin develop --uv   # rebuild the Rust extension in-place
uv run taskmd ...             # exercise the dev build
```

`maturin develop` produces `src/taskmd/_core*.so` and installs `taskmd`
as an editable package in `.venv/`. `uv run taskmd` then resolves to the
dev build, not your `uv tool`–installed copy.

### Running the test suites

```bash
cargo test --manifest-path taskmd-core/Cargo.toml       # Rust unit + proptests
uv run pytest tests/ -q                                 # Python integration
```

Both should be green before tagging a release. The publish workflow
does NOT gate on tests — if you want that, add a `test` job and
`needs: test` to `build-wheels` / `build-sdist`.
