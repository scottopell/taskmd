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
            "description": "Print the next available task ID (prefix derived from hostname + directory path)",
            "args": {"tasks_dir": {"type": "path", "default": "./tasks or ./tasksmd"}},
            "output": "Task ID string (5-digit numeric DDNNN format)",
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
        },
        "commands": commands,
        "task_format": {
            "filename_pattern": "DDNNN-pX-status--slug.md",
            "id_format": "D1 = hostname-derived digit, D2 = directory-derived digit, NNN = 3-digit sequence",
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
    }

    if not compact:
        s["workflows"] = [
            {
                "name": "Initialize a tasks directory",
                "steps": [
                    "taskmd init  # creates ./tasks/ with _TEMPLATE.md",
                    "# Or: taskmd init my-tasks/  # custom path",
                ],
            },
            {
                "name": "Create a new task",
                "steps": [
                    "taskmd next  # get next ID, e.g. 34042",
                    "Create file: tasks/34042-p2-ready--short-slug.md",
                    "Add frontmatter: created, priority, status, artifact",
                    "Write task body with Summary and Done When sections",
                    "taskmd validate  # confirm it's valid",
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
        s["anti_patterns"] = [
            "Don't rename task files directly -- edit frontmatter and run 'taskmd fix'",
            "Don't use sequence 000 in task IDs -- sequences start at 001",
            "Don't omit the double-dash before the slug -- it's 'status--slug', not 'status-slug'",
            "Don't put spaces in slugs -- use hyphens: 'fix-the-bug' not 'fix the bug'",
            "Don't create tasks for work you can do right now. A task tracks work blocked by something: user input, a different environment, passage of time, or an unmade decision. If nothing prevents you from doing it immediately, it's an action -- just do it.",
            "Don't create tasks that describe transient system states with no durable artifact. If you can't fill in the artifact: field honestly, the task should not exist.",
            "Don't add extra fields to frontmatter (e.g. result:, notes:, assignee:). Only the valid fields are allowed: " + ", ".join(sorted(VALID_FIELDS)) + ". Put everything else in the markdown body.",
        ]
        s["best_practices"] = [
            "Run 'taskmd validate' after creating or editing task files",
            "Run 'taskmd fix' to auto-repair rather than manually renaming files",
            "Use 'taskmd list --status ready' to find work that needs to be done",
            "Keep slugs short and descriptive (3-5 words)",
            "One concern per task file -- split large tasks into subtasks",
            "Before creating a task, ask: what blocks me from doing this now? If the answer is nothing, it's an action, not a task -- just do it.",
            "The artifact: field should name what this task produces or changes when done (a file, a config, a commit). If you struggle to fill it in, reconsider whether the task should exist.",
        ]

    return s


def schema_json(compact: bool = False) -> str:
    """Return the schema as a formatted JSON string."""
    return json.dumps(schema(compact), indent=2, sort_keys=True)
