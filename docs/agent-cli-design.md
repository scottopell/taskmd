# Agent-Friendly CLI Design Principles

Design principles for making the `taskmd` CLI work well for both humans and LLM
coding agents. Derived from studying Datadog's `pup` CLI, which is agent-native
from the ground up.

## Core Insight

An agent-friendly CLI is also a better CLI for humans. Structured output,
actionable errors, and self-documenting commands benefit everyone. The key
difference: agents can't ask clarifying questions, so the CLI must be
unambiguous by default.

## Principles

### 1. Auto-detect agent callers

Detect known coding agents via environment variables (`CLAUDECODE`,
`CURSOR_AGENT`, `AIDER`, `CLINE`, `WINDSURF_AGENT`, `GITHUB_COPILOT`, etc.)
and switch behavior automatically. Provide `--agent` as a manual override.

When agent mode is active:
- Output structured JSON instead of human-formatted text
- Skip interactive prompts (auto-approve or fail with guidance)
- Include machine-parseable metadata in responses

### 2. JSON envelope for all output

Every command response in agent mode uses a consistent envelope:

```json
{
  "status": "success",
  "command": "validate",
  "data": { ... },
  "metadata": { ... }
}
```

Errors use the same structure with actionable suggestions:

```json
{
  "status": "error",
  "command": "validate",
  "errors": [ ... ],
  "suggestions": ["Run 'taskmd fix' to auto-repair these issues"]
}
```

Agents should never need to parse prose to understand what happened.

### 3. Self-documenting schema via --help

In agent mode, `--help` returns a JSON schema describing all commands, flags,
valid values, and the task file format. This lets agents discover capabilities
without reading external docs.

The schema should include:
- Command names, flags, types, and defaults
- Valid statuses and priorities (from the source of truth in core.py)
- Task file format spec (filename pattern, frontmatter fields)
- Example workflows
- Common anti-patterns to avoid

Offer a `--compact` variant for token-constrained agents.

### 4. Embed operational guidance in the CLI itself

Don't rely on agents finding and reading documentation files. Bake guidance
directly into the schema output:

- **Workflows**: "To triage tasks: `taskmd list`, review priorities, edit
  frontmatter, `taskmd fix`"
- **Anti-patterns**: "Don't rename task files directly -- edit frontmatter and
  run `taskmd fix`"
- **Format spec**: full filename pattern and valid field values

This is the single biggest win from pup's design. An agent that calls
`taskmd --help --agent` should have everything it needs to use the tool
correctly without reading any other file.

### 5. Actionable error messages

Every error should tell the caller what to do next. Not just "invalid status"
but "invalid status 'started' -- valid values: ready, in-progress, blocked,
done, wont-do, brainstorming".

In agent mode, structure this as data:

```json
{
  "field": "status",
  "value": "started",
  "message": "invalid status",
  "valid_values": ["ready", "in-progress", "blocked", "done", "wont-do", "brainstorming"],
  "fix": "Edit the 'status' field in the file's YAML frontmatter"
}
```

### 6. Safety by default

- `--read-only` flag (or env var) to block mutations -- useful when agents are
  exploring
- Confirmation prompts for destructive operations in human mode, auto-skipped
  in agent mode (agents can't answer stdin prompts)
- Clear indication of what changed (renames, patches) in structured output so
  agents can verify

### 7. Expose the library, not just the CLI

taskmd already does this well: `core.py` has pure functions returning
dataclasses, and `cli.py` is a thin wrapper. Agents that run as Python
processes can import the library directly and skip the CLI entirely.

The CLI's agent mode is for agents that interact via shell commands (which is
most of them today). But keep the library API as the source of truth.

## What This Means for taskmd

The existing CLI has 3 commands (`validate`, `fix`, `next`) and a library
function (`list_tasks`) not yet exposed in the CLI. The implementation plan:

1. Add agent auto-detection (env vars + `--agent` flag)
2. Add `--output json` flag (default to text for humans, JSON in agent mode)
3. JSON envelope wrapping existing dataclass output
4. Structured `--help` schema in agent mode
5. Expose `list` as a CLI subcommand with filtering
6. Upgrade error messages to include suggestions and valid values
7. Embed task format spec and workflows in schema output

The library layer (`core.py`) needs minimal changes -- most of the work is in
`cli.py` and a new `schema.py` or similar module for generating the agent
schema.
