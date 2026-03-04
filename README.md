# task.md

Markdown-native task management for developer projects.

Each task is a markdown file. Metadata lives in the filename and YAML frontmatter.
No database, no config file — the filesystem is the data store, git is the audit trail.

```
tasks/
  0001-p1-done--initial-setup.md
  0002-p2-ready--add-feature.md
  0003-p3-blocked--waiting-on-api.md
```

## Install

```bash
uv pip install taskmd
# or
pip install taskmd
```

## CLI

```bash
taskmd validate        # check all task files for consistency
taskmd fix             # auto-repair (missing dates, mismatched filenames)
taskmd next            # print next available task number
```

## Library

```python
from taskmd import validate, fix, next_number

result = validate("tasks")
if not result.ok:
    for err in result.errors:
        print(err)

n = next_number("tasks")
print(f"Next task: {n:04d}")
```

## Use from a PEP 723 script (e.g., dev.py)

```python
#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# dependencies = ["taskmd"]
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
---

# Task Title

## Summary
What needs to be done.

## Acceptance Criteria
- [ ] First criterion
- [ ] Second criterion
```

**Statuses:** ready, in-progress, blocked, done, wont-do, brainstorming

**Priorities:** p0 (critical) through p4 (polish)

**To change status:** edit the `status:` field in frontmatter, then `taskmd fix`.

## License

MIT
