# Operations

Release, install, and maintenance procedures for `taskmd`.

## Cutting a release

Releases are tag-triggered. Pushing any tag matching `v*` runs
`.github/workflows/publish.yml`, which:

1. Builds wheels on Linux/macOS/Windows.
2. Builds an sdist.
3. Publishes the Python package to PyPI via OIDC trusted publishing
   (`pypi` GitHub environment is the trust anchor — no token in repo).
4. Publishes `taskmd-core` to crates.io via OIDC trusted publishing
   (`crates-io` GitHub environment is the trust anchor — no token in repo).

Both registries are configured to trust this repo's `publish.yml`. If
either trust config is removed, the corresponding job fails fast.

```bash
# 1. Pick the version. See "Versioning" below.
NEW=1.1.0

# 2. Bump pyproject.toml AND both Cargo manifests in lockstep.
#    The Cargo manifests use the SemVer pre-release form ("1.1.0-rc1"),
#    pyproject.toml uses the PEP 440 form ("1.1.0rc1"). The sed below
#    inserts the hyphen for Cargo if NEW contains "rc".
sed -i '' "s/^version = \".*\"/version = \"$NEW\"/" pyproject.toml
sed -i '' "s/^version = \".*\"/version = \"$(echo $NEW | sed 's/rc/-rc/')\"/" \
    taskmd-core/Cargo.toml taskmd-py/Cargo.toml
cargo update -p taskmd-core -p taskmd-py

# 3. Update CHANGELOG.md: rename the "Unreleased" heading to the new
#    version and date it. Add an "Unreleased" stub above for next time.

# 4. Verify both test suites are green before tagging.
cargo test --manifest-path taskmd-core/Cargo.toml
uv run pytest tests/ -q

# 5. Commit pyproject.toml + Cargo manifests + Cargo.lock + CHANGELOG.md.
git add pyproject.toml taskmd-core/Cargo.toml taskmd-py/Cargo.toml \
        Cargo.lock CHANGELOG.md
git commit -m "chore: $NEW"
git push

# 6. Tag and push the tag. This triggers the publish workflow.
git tag v$NEW
git push origin v$NEW

# 7. Watch it go green.
gh run watch
```

Release is done when both registries show the new version and the
workflow is green:
- https://pypi.org/project/taskmd/
- https://crates.io/crates/taskmd-core

Note: PyPI hides pre-release versions from the "latest" badge. After
publishing `1.x.y-rc1`, `pip install taskmd` still resolves to the most
recent stable. Users who want the RC must either pass `--pre` or pin
the exact version.

### Versioning

Semver-ish. The surface area is small, so calibrate mostly by user impact:

- **Patch** (`0.2.0 -> 0.2.1`): bug fixes, doc-only changes, internal
  refactors, performance work. No behavior change users would notice.
- **Minor** (`0.2.0 -> 0.3.0`): new CLI verb, new flag, new JSON field,
  anything agents or humans can newly depend on.
- **Major** (`0.x.y -> 1.0.0`): breaking CLI/API change — renamed or
  removed commands, changed JSON envelope shape, non-backwards-compatible
  filename grammar. Don't ship one without a migration note in the
  release commit body.

If multiple classes of change landed since the last tag, use the highest
one. Skim `git log v<prev>..main` to categorize.

### Trusted publisher configuration

Both registries publish via OIDC; no API tokens are stored anywhere.

- **PyPI**: trust is anchored to the `pypi` GitHub environment via
  `pypa/gh-action-pypi-publish@release/v1`. Manage at
  https://pypi.org/manage/project/taskmd/settings/publishing/.
- **crates.io**: trust is anchored to the `crates-io` GitHub environment
  via `rust-lang/crates-io-auth-action@v1`. Manage at
  https://crates.io/crates/taskmd-core/settings (Trusted Publishers
  section).

If trust config is removed or the workflow filename changes, the
corresponding job fails on the very next tag push. Re-add the trusted
publisher (repo owner, repo name, workflow filename = `publish.yml`,
environment) and re-run the workflow.

### Recovering from a bad release

If the workflow publishes a broken version:

1. **Yank, don't delete.**
   - PyPI: yank at https://pypi.org/manage/project/taskmd/. `pip install`
     won't resolve to a yanked version (without explicit pin) but
     existing pins keep working.
   - crates.io: `cargo yank --version <version> taskmd-core`. Same
     semantics — new resolutions skip yanked, old pins keep working.
2. Fix the bug on main, bump to the next patch version, tag, publish.
3. Never re-tag an existing version — both registries reject re-uploads
   of the same filename, and yanked versions cannot be replaced.

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
# Rebuild the Rust extension into the source tree. The --release flag
# matches what CI ships; drop it for a faster debug build during
# iteration.
uv run --with maturin maturin build --release --out /tmp/taskmd-build
unzip -o /tmp/taskmd-build/taskmd-*.whl '*/_core.*.so' -d src/taskmd/
mv src/taskmd/taskmd/_core.*.so src/taskmd/   # flatten the wheel layout
rmdir src/taskmd/taskmd 2>/dev/null

uv run taskmd ...             # exercise the dev build
uv run pytest tests/ -q       # run the Python integration tests
```

This roundabout sequence exists because `maturin develop --uv`
currently fails on `uv >= 0.5` (it invokes `uv pip install --group dev`
which uv doesn't accept). When maturin or uv ship a fix, the
`maturin develop --uv` one-liner can come back.

`src/taskmd/_core.*.so` is gitignored — the build produces it on demand
and it ships inside the wheel CI publishes. `uv run taskmd` then
resolves to the editable install in `.venv/`, not your `uv tool`–
installed copy.

### Running the test suites

```bash
cargo test --manifest-path taskmd-core/Cargo.toml       # Rust unit + proptests
uv run pytest tests/ -q                                 # Python integration
```

Both should be green before tagging a release. The publish workflow
does NOT gate on tests — if you want that, add a `test` job and
`needs: test` to `build-wheels` / `build-sdist`.
