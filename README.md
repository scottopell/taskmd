# task.md

Task.md is a is structured but simple task management system.
Tasks can be used as prompts for LLM Agents or tickets for humans.

Each task is a markdown file.
Metadata lives in the filename and YAML frontmatter.
No database, no config file — the filesystem is the data store, git is the audit trail.

```
tasks/
  0001-p1-done--initial-setup.md
  0002-p2-ready--add-feature.md
  0003-p3-blocked--waiting-on-api.md
```

## CLI

Run directly with `uvx` — no install needed:

```bash
uvx --from "git+https://github.com/scottopell/taskmd.git" taskmd validate
uvx --from "git+https://github.com/scottopell/taskmd.git" taskmd fix
uvx --from "git+https://github.com/scottopell/taskmd.git" taskmd next
```

Or install as a persistent tool:

```bash
uv tool install git+https://github.com/scottopell/taskmd.git
taskmd validate     # check all task files for consistency
taskmd fix          # auto-repair (missing dates, mismatched filenames)
taskmd next         # print next available task number
```

All commands default to `./tasks` — pass a path to use a different directory.

## Library

Install as a dependency:

```bash
uv pip install git+https://github.com/scottopell/taskmd.git
```

```python
from taskmd import validate, fix, next_number, list_tasks

result = validate("tasks")
if not result.ok:
    for err in result.errors:
        print(err)

tasks = list_tasks("tasks")
for t in tasks:
    print(f"{t.number:04d} {t.priority} {t.status:12s} {t.slug}")

n = next_number("tasks")
print(f"Next task: {n:04d}")
```

## Use from a PEP 723 script (e.g., dev.py)

```python
#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# dependencies = ["taskmd @ git+https://github.com/scottopell/taskmd.git"]
# ///

from taskmd import validate

result = validate("tasks")
if not result.ok:
    print(f"Task validation failed: {len(result.errors)} errors")
```

## Task file format

Filename: `NNNN-pX-status--slug.md`

```yaml
---
created: 2026-03-04
priority: p2
status: ready
artifact: src/feature.py
---

# Task Title

## Summary
What needs to be done.

## Done When
- [ ] First criterion
- [ ] Second criterion
```

**Statuses:** ready, in-progress, blocked, done, wont-do, brainstorming

**Priorities:** p0 (critical) through p4 (polish)

**Artifact:** the concrete output this task produces (file path, config change, commit). Required.

**To change status:** edit the `status:` field in frontmatter, then `taskmd fix`.

## License

MIT
