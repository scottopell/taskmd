# task.md

A structured but simple task management system.
Tasks work as prompts for LLM agents or tickets for humans.

Each task is a markdown file.
Metadata lives in the filename and YAML frontmatter.
No database, no config file — the filesystem is the data store, git is the audit trail.

```
tasks/
  34001-p1-done--initial-setup.md
  34002-p2-ready--add-feature.md
  34003-p3-blocked--waiting-on-api.md
```

Task IDs are 5-digit numbers (e.g., `34042`) where the first digit is derived from the machine's hostname and the second from the tasks directory path. This avoids ID conflicts across machines and git worktrees without coordination.

Set `TASKMD_MACHINE_ID=0` to pin the first digit on your primary machine.

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
taskmd init         # create tasks directory with a template file
taskmd validate     # check all task files for consistency
taskmd fix          # auto-repair (missing dates, mismatched filenames, legacy ID formats)
taskmd next         # print the next available task ID
taskmd list         # list all tasks with metadata
```

All commands auto-detect `./tasks` or `./tasksmd` as the default directory. Pass a path to override.

### Agent mode

When run inside an LLM coding agent (Claude Code, Cursor, Codex, etc.), taskmd auto-detects the environment and switches to JSON output. You can also force it:

```bash
taskmd --agent validate        # structured JSON envelope
taskmd --agent --help          # self-documenting schema for agents
taskmd --agent --compact --help  # minimal schema (fewer tokens)
```

## Library

Install as a dependency:

```bash
uv pip install git+https://github.com/scottopell/taskmd.git
```

```python
from taskmd import validate, fix, next_id, list_tasks

result = validate("tasks")
if not result.ok:
    for err in result.errors:
        print(err)

tasks = list_tasks("tasks")
for t in tasks:
    print(f"{t.id:5s} {t.priority} {t.status:12s} {t.slug}")

n = next_id("tasks")
print(f"Next task: {n}")
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

Filename: `DDNNN-pX-status--slug.md`

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

Only the four fields above are allowed in frontmatter — unknown fields are rejected by validation.

**To change status:** edit the `status:` field in frontmatter, then `taskmd fix`.

**Legacy migration:** files using old formats (4-digit `NNNN` or alpha-prefix `AANNN`) are auto-migrated by `taskmd fix`.

## License

MIT
