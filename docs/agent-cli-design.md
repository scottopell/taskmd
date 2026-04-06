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

taskmd already does this well: all logic lives in a Rust core library
(`taskmd-core/`), exposed to Python via PyO3 (`taskmd._core`). `core.py` is a
thin shim that wraps the Rust extension with Python dataclasses. `cli.py` is a
thin wrapper over that. Agents running as Python processes can import the
library directly and skip the CLI entirely.

The CLI's agent mode is for agents that interact via shell commands (which is
most of them today). The Rust library is the source of truth for constants,
validation, and all task operations.

