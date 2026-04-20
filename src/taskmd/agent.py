"""Agent detection, JSON envelope formatting, and schema generation.

Provides agent auto-detection via environment variables, structured JSON
output envelopes, and a self-documenting schema for agent --help.
"""

from __future__ import annotations

import json
import os
from typing import Any

from taskmd.core import VALID_FIELDS, VALID_PRIORITIES, VALID_STATUSES

# ---------------------------------------------------------------------------
# Agent detection
# ---------------------------------------------------------------------------

# (env_var, agent_name) — first match wins
_AGENT_DETECTORS: list[tuple[list[str], str]] = [
    (["CLAUDECODE", "CLAUDE_CODE"], "claude-code"),
    (["CURSOR_AGENT"], "cursor"),
    (["CODEX", "OPENAI_CODEX"], "codex"),
    (["OPENCODE"], "opencode"),
    (["AIDER"], "aider"),
    (["CLINE"], "cline"),
    (["WINDSURF_AGENT"], "windsurf"),
    (["GITHUB_COPILOT"], "github-copilot"),
    (["AMAZON_Q", "AWS_Q_DEVELOPER"], "amazon-q"),
    (["GEMINI_CODE_ASSIST"], "gemini"),
    (["SRC_CODY"], "sourcegraph-cody"),
    (["AGENT"], "generic"),
]


def _is_env_truthy(name: str) -> bool:
    return os.environ.get(name, "").lower() in ("1", "true", "yes")


def detect_agent() -> str | None:
    """Return the detected agent name, or None if not in agent mode."""
    if _is_env_truthy("FORCE_AGENT_MODE"):
        return "force"
    for env_vars, name in _AGENT_DETECTORS:
        for var in env_vars:
            if _is_env_truthy(var):
                return name
    return None


def is_agent_mode(flag: bool = False) -> bool:
    """True if --agent flag was passed or an agent env var is set."""
    return flag or detect_agent() is not None


# ---------------------------------------------------------------------------
# JSON envelope
# ---------------------------------------------------------------------------

def success_envelope(command: str, data: Any, **metadata: Any) -> str:
    """Wrap a successful result in the standard agent envelope."""
    obj: dict[str, Any] = {
        "status": "success",
        "command": command,
        "data": data,
    }
    if metadata:
        obj["metadata"] = metadata
    return json.dumps(obj, indent=2, default=str, sort_keys=True)


def error_envelope(
    command: str,
    errors: list[str],
    suggestions: list[str] | None = None,
) -> str:
    """Wrap errors in the standard agent envelope."""
    obj: dict[str, Any] = {
        "status": "error",
        "command": command,
        "errors": errors,
    }
    if suggestions:
        obj["suggestions"] = suggestions
    return json.dumps(obj, indent=2, default=str, sort_keys=True)


# ---------------------------------------------------------------------------
# Schema generation
# ---------------------------------------------------------------------------

def schema(compact: bool = False) -> dict[str, Any]:
    """Return a JSON-serialisable schema describing the taskmd CLI.

    When compact=True, omits examples and descriptions to save tokens.
    """
    commands: dict[str, Any] = {
        "init": {
            "description": "Create a new tasks directory with a _TEMPLATE.md file. Fails if directory already exists.",
            "args": {"tasks_dir": {"type": "path", "default": "./tasks"}},
            "output": "InitResult with tasks_dir, created[], template_fields[]",
        },
        "new": {
            "description": "Create a new task atomically: allocate next ID, synthesize frontmatter, and write the file in one step. This is the recommended way to create tasks — prefer it over 'next' + manual file writes. Body is read from stdin; omit stdin to get a template skeleton.",
            "args": {
                "tasks_dir": {"type": "path", "default": "./tasks or ./tasksmd"},
                "--slug": {"type": "string", "required": True, "description": "URL-safe slug (e.g. 'fix-login-bug'). Dirty input is normalized via derive_slug."},
                "--artifact": {"type": "string", "required": True, "description": "The concrete output this task produces (file path, config change, commit). Required — if you cannot name one, the task probably should not exist."},
                "--priority": {"type": "string", "default": "p2", "values": sorted(VALID_PRIORITIES)},
                "--status": {"type": "string", "default": "ready", "values": sorted(VALID_STATUSES)},
                "stdin": {"type": "markdown body", "description": "Task body (no frontmatter — that's synthesized). If stdin is empty, a skeleton body is used."},
            },
            "output": "CreateResult with id, path, filename",
            "examples": [
                "taskmd new --slug fix-login --artifact src/auth.py",
                "echo 'body text' | taskmd new --slug fix-login --artifact src/auth.py",
                "cat body.md | taskmd new --slug fix-login --artifact src/auth.py --priority p1",
            ],
        },
        "validate": {
            "description": "Check all task files for consistency",
            "args": {"tasks_dir": {"type": "path", "default": "./tasks or ./tasksmd"}},
            "output": "ValidationResult with errors[] and file_count",
        },
        "fix": {
            "description": "Auto-repair fixable issues (missing dates, mismatched filenames, legacy ID formats)",
            "args": {"tasks_dir": {"type": "path", "default": "./tasks or ./tasksmd"}},
            "output": "FixResult with patches[], renames[], migrated count, errors[]",
        },
        "next": {
            "description": "Print the next available task ID (prefix derived from hostname + directory path). DISCOURAGED: this is a read-only advisory that doesn't claim the ID — two concurrent callers can receive the same ID. Prefer 'taskmd new' for creation; use 'next' only for integrations that must do their own write path.",
            "args": {"tasks_dir": {"type": "path", "default": "./tasks or ./tasksmd"}},
            "output": "Task ID string (5-digit numeric DDNNN format)",
            "prefer_instead": "new",
        },
        "list": {
            "description": "List all task files with metadata",
            "args": {
                "tasks_dir": {"type": "path", "default": "./tasks or ./tasksmd"},
                "--status": {"type": "string", "description": "Filter by status"},
                "--priority": {"type": "string", "description": "Filter by priority"},
            },
            "output": "Array of TaskFile objects",
        },
    }

    s: dict[str, Any] = {
        "name": "taskmd",
        "description": "Markdown-native task management. Each task is a file. No database, no config -- the filesystem is the data store, git is the audit trail.",
        "global_flags": {
            "--agent": {"description": "Force agent mode (JSON output, structured --help)"},
            "--output": {"type": "json|text", "default": "text (json in agent mode)"},
            "--compact": {"description": "Minimal schema output (fewer tokens)"},
            "--version, -V": {"description": "Print version and exit"},
        },
        "commands": commands,
        "task_format": {
            "filename_pattern": "DDNNN-pX-status--slug.md",
            "id_format": "D1 = hostname-derived digit, D2 = directory-derived digit, NNN = 3-digit sequence (see environment_variables for overrides)",
            "example": "34042-p2-ready--fix-the-bug.md",
            "frontmatter_fields": {
                "created": {"required": True, "format": "YYYY-MM-DD"},
                "priority": {"required": True, "values": sorted(VALID_PRIORITIES)},
                "status": {"required": True, "values": sorted(VALID_STATUSES)},
                "artifact": {"required": True, "description": "The concrete output this task produces (file path, config change, commit, etc.). If you cannot name one, the task probably should not exist."},
            },
        },
        "valid_statuses": sorted(VALID_STATUSES),
        "valid_priorities": sorted(VALID_PRIORITIES),
        "valid_fields": sorted(VALID_FIELDS),
        "environment_variables": {
            "TASKMD_MACHINE_ID": {
                "description": "Override D1 (machine digit) in task ID generation",
                "values": "single digit 0-9",
                "default": "sha256(hostname) mod 10",
            },
            "FORCE_AGENT_MODE": {
                "description": "Force agent mode regardless of caller",
                "values": "1, true, yes",
            },
            "agent_detection": {
                "description": "Agent mode activates automatically when any of these are truthy",
                "vars": [var for vars, _ in _AGENT_DETECTORS for var in vars],
            },
        },
    }

    # Compact mode gets essential guidance only; full mode gets everything.
    s["guidance"] = [
        "Use 'taskmd new' to create tasks. Do NOT hand-craft filenames or pattern-match ID prefixes you see on disk — 'new' allocates the ID, formats the filename, and writes the file atomically. Mimicking an on-disk ID is the #1 cause of duplicate-ID bugs.",
        "Tasks are markdown files. After 'new' creates them, edit them directly -- that's the primary interface.",
        "A task tracks work blocked by something: user input, a different environment, passage of time, or an unmade decision. If nothing blocks you from doing it now, just do it instead of creating a task.",
        "The artifact: field names what this task produces when done (a file, a config change, a commit). If you can't fill it in, the task probably shouldn't exist.",
        "Frontmatter is the source of truth. To change status, edit the status: field. 'taskmd fix' will rename the file to match, or you can rename it yourself.",
    ]

    if not compact:
        s["guidance"] += [
            "Filenames use double-dash before the slug: 'status--slug', not 'status-slug'. Slugs are kebab-case, 3-5 words. 'taskmd new' handles this for you — you only supply --slug.",
            "Only these fields belong in frontmatter: " + ", ".join(sorted(VALID_FIELDS)) + ". Everything else goes in the markdown body.",
            "'taskmd next' returns an ID without claiming it and is discouraged — it's kept only for integrations that do their own write path. Two concurrent 'next' callers can get the same ID.",
            "Run 'taskmd validate' after editing task files to catch issues early. If duplicate IDs appear, it will flag them.",
            "One concern per task file -- split large tasks into subtasks.",
        ]
        s["workflows"] = [
            {
                "name": "Initialize a tasks directory",
                "steps": [
                    "taskmd init  # creates ./tasks/ with _TEMPLATE.md",
                    "# Or: taskmd init my-tasks/  # custom path",
                ],
            },
            {
                "name": "Create a new task (recommended)",
                "steps": [
                    "taskmd new --slug fix-login --artifact src/auth.py  # skeleton body",
                    "# Or pipe a prewritten body:",
                    "#   cat body.md | taskmd new --slug fix-login --artifact src/auth.py --priority p1",
                    "# 'new' prints the created path on success.",
                    "taskmd validate  # confirm it's valid",
                ],
            },
            {
                "name": "Create a task when you need to do the write yourself (discouraged)",
                "steps": [
                    "taskmd next  # get next ID, e.g. 34042 (sharp edge: unclaimed)",
                    "Create file: tasks/34042-p2-ready--short-slug.md",
                    "Add frontmatter: created, priority, status, artifact",
                    "Write task body with Summary, Context, Done When, and Notes sections",
                    "taskmd validate  # confirm it's valid",
                    "# Prefer 'taskmd new' for this flow unless you have a specific reason not to.",
                ],
            },
            {
                "name": "Change task status",
                "steps": [
                    "Edit the 'status' field in the file's YAML frontmatter",
                    "taskmd fix  # renames file to match new status",
                ],
            },
            {
                "name": "Triage tasks",
                "steps": [
                    "taskmd list  # see all tasks",
                    "taskmd list --status ready  # filter to actionable tasks",
                    "taskmd list --priority p0  # find critical items",
                ],
            },
        ]
    return s


def schema_json(compact: bool = False) -> str:
    """Return the schema as a formatted JSON string."""
    return json.dumps(schema(compact), indent=2, sort_keys=True)
