# task.md

Most task trackers assume a human is driving. But when an LLM agent picks up a task, it needs structured metadata -- priority, status, completion criteria -- not a Jira ticket behind an OAuth wall. And when a human reviews that same task, they don't want to parse JSON. They want to read a file.

**taskmd makes tasks that work for both.** Each task is a markdown file. Metadata lives in the filename and YAML frontmatter. The filesystem is the data store, git is the audit trail -- no database, no config, no coordination server.

```
tasks/
  34001-p1-done--initial-setup.md
  34002-p2-ready--add-feature.md
  34003-p3-blocked--waiting-on-api.md
```

Three properties make this work:

- **Filesystem-native.** Tasks are files. Create them with your editor, move them with `mv`, search them with `grep`. Every tool you already have works.
- **Git-native.** Status changes are commits. Branches get their own task state. Merges resolve naturally. `git log` is your audit trail.
- **Agent-native.** When taskmd detects it's running inside Claude Code, Cursor, Codex, or a dozen other agents, it automatically switches to structured JSON output -- same files, same commands, zero configuration.

## Install

```bash
pip install taskmd
```

Or run without installing:

```bash
uvx taskmd validate
uvx taskmd list
```

## Quick start

```bash
taskmd init                                                                                         # create tasks/ with a template
echo "Fix the login redirect loop when JWT is expired." | taskmd new --slug fix-login --artifact src/auth.py
cat body.md | taskmd new --slug add-oauth --artifact src/oauth.py --priority p1                      # body from file
taskmd status 34042 in-progress                                                                      # change a task's status atomically
taskmd validate                                                                                      # check all task files for consistency
taskmd fix                                                                                           # auto-repair filenames, dates, legacy formats, duplicate IDs
taskmd list                                                                                          # list tasks with metadata
taskmd list --status ready --priority p0                                                             # filter to what matters
```

`taskmd new` is the recommended way to create tasks — it allocates the ID, formats the filename, synthesizes the frontmatter, and writes the file in one atomic step. The body is required on stdin; a task with no description is a placeholder that pollutes triage. `taskmd next` exists for integrations that need just an ID string, but it's a sharp edge (two concurrent callers can receive the same ID).

All commands auto-detect `./tasks` or `./tasksmd` as the default directory. Pass a path to override.

## Agent mode

When run inside an LLM coding agent, taskmd auto-detects the environment and switches to JSON output. The agent gets structured data; the human reading the same file gets markdown. No separate "agent API" to maintain.

```bash
taskmd --agent validate          # structured JSON envelope
taskmd --agent --help            # self-documenting schema for agents
taskmd --agent --compact --help  # minimal schema (fewer tokens)
```

Detection is automatic for Claude Code, Cursor, Codex, Windsurf, Aider, Cline, Amazon Q, Gemini Code Assist, Cody, and any tool that sets `AGENT=1`.

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

**Artifact:** the concrete output this task produces (file path, config change, commit). Required -- if you can't name one, the task probably shouldn't exist.

Only these four fields are allowed in frontmatter. Unknown fields are rejected by validation.

**To change status:** `taskmd status <id> <new-status>` updates the frontmatter and renames the file to match in one atomic step. It refuses to clobber an existing target filename and rejects invalid statuses up front. Hand-editing the `status:` field and running `taskmd fix` still works as an escape hatch, but `taskmd status` is the preferred path.

### Task IDs

IDs are 5-digit numbers (e.g., `34042`). The first digit is derived from the machine's hostname, the second from the tasks directory path. This avoids ID collisions across machines and git worktrees without any coordination.

Set `TASKMD_MACHINE_ID=0` to pin the first digit on your primary machine.
## Library

taskmd is also a Python library -- the CLI is a thin wrapper around it.

```bash
pip install taskmd
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

### Use from a PEP 723 script

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

## License

MIT
