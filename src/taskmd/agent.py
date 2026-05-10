"""Agent detection, JSON envelope formatting, and schema generation."""

from __future__ import annotations

import json
import os
from typing import Any

from taskmd.core import VALID_PRIORITIES, VALID_STATUSES

# ---------------------------------------------------------------------------
# Agent detection
# ---------------------------------------------------------------------------

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
    return flag or detect_agent() is not None


# ---------------------------------------------------------------------------
# JSON envelope
# ---------------------------------------------------------------------------

def success_envelope(command: str, data: Any, **metadata: Any) -> str:
    obj: dict[str, Any] = {
        "status": "success",
        "command": command,
        "data": data,
    }
    if metadata:
        obj["metadata"] = metadata
    return json.dumps(obj, separators=(",", ":"), default=str, sort_keys=True)


def error_envelope(
    command: str,
    errors: list[str],
    suggestions: list[str] | None = None,
    data: Any = None,
) -> str:
    obj: dict[str, Any] = {
        "status": "error",
        "command": command,
        "errors": errors,
    }
    if suggestions:
        obj["suggestions"] = suggestions
    if data is not None:
        obj["data"] = data
    return json.dumps(obj, separators=(",", ":"), default=str, sort_keys=True)


# ---------------------------------------------------------------------------
# Schema generation
# ---------------------------------------------------------------------------

def schema(compact: bool = False) -> dict[str, Any]:
    """Return a JSON-serialisable schema describing the taskmd CLI."""
    commands: dict[str, Any] = {
        "init": {
            "description": "Create a new tasks directory with a _TEMPLATE.md file. Fails if directory already exists.",
            "args": {"tasks_dir": {"type": "path", "default": "./tasks"}},
            "output": "InitResult with tasks_dir, created[]",
        },
        "new": {
            "description": "Create a new task atomically: allocate next ID, format the filename, and write the file containing the body from stdin. This is the recommended way to create tasks. Body is REQUIRED on stdin — a task with no description is a placeholder.",
            "args": {
                "tasks_dir": {"type": "path", "default": "./tasks or ./taskmds"},
                "--slug": {"type": "string", "required": True, "description": "URL-safe slug (e.g. 'fix-login-bug'). Dirty input is normalized via derive_slug."},
                "--priority": {"type": "string", "default": "p2", "values": sorted(VALID_PRIORITIES)},
                "--status": {"type": "string", "default": "ready", "values": sorted(VALID_STATUSES)},
                "stdin": {"type": "markdown body", "required": True, "description": "Task body written verbatim to the file. Must be non-empty."},
            },
            "output": "CreateResult with id, path, filename",
            "examples": [
                "echo 'Fix the login redirect loop when the JWT is expired.' | taskmd new --slug fix-login",
                "cat body.md | taskmd new --slug fix-login --priority p1",
            ],
        },
        "status": {
            "description": "Change a task's status by renaming the file. This is the recommended way to transition a task through its lifecycle (ready -> in-progress -> done).",
            "args": {
                "id": {"type": "string", "required": True, "description": "Task ID (e.g. '34042'). Look it up with 'taskmd list' if you don't know it."},
                "new_status": {"type": "string", "required": True, "values": sorted(VALID_STATUSES), "description": "Target status. Must be a valid status."},
                "tasks_dir": {"type": "path", "default": "./tasks or ./taskmds"},
            },
            "output": "{id, old_filename, new_filename, old_status, new_status}",
            "examples": [
                "taskmd status 34042 in-progress",
                "taskmd status 34042 done",
                "taskmd status 34042 blocked ./tasks",
            ],
        },
        "validate": {
            "description": "Check all task filenames for consistency (pattern + duplicate IDs)",
            "args": {"tasks_dir": {"type": "path", "default": "./tasks or ./taskmds"}},
            "output": "ValidationResult with errors[] and file_count",
        },
        "fix": {
            "description": "Auto-repair fixable issues (legacy ID formats, duplicate task IDs). On first run after upgrading from a frontmatter-bearing version, fix will refuse and prompt for --migrate (strip frontmatter, destructive) or --no-migrate (skip the check).",
            "args": {
                "tasks_dir": {"type": "path", "default": "./tasks or ./taskmds"},
                "--migrate": {"type": "flag", "description": "Strip legacy YAML frontmatter from every task file that has it (destructive — commit first)."},
                "--no-migrate": {"type": "flag", "description": "Skip the frontmatter migration check entirely."},
            },
            "output": "FixResult with renames[], migrated count, renumbered[] (old_id/new_id/old_filename/new_filename for each duplicate-ID loser — cross-references NOT auto-patched), frontmatter_stripped[] (filenames whose YAML frontmatter was removed), errors[]. When neither --migrate nor --no-migrate is passed and frontmatter is detected, fix returns an error envelope whose data.frontmatter_pending lists the affected files.",
        },
        "next": {
            "description": "Print the next available task ID (prefix derived from hostname + directory path). DISCOURAGED: this is a read-only advisory that doesn't claim the ID — two concurrent callers can receive the same ID. Prefer 'taskmd new' for creation; use 'next' only for integrations that must do their own write path.",
            "args": {"tasks_dir": {"type": "path", "default": "./tasks or ./taskmds"}},
            "output": "Task ID string (5-digit numeric DDNNN format)",
            "prefer_instead": "new",
        },
        "list": {
            "description": "List all task files with metadata",
            "args": {
                "tasks_dir": {"type": "path", "default": "./tasks or ./taskmds"},
                "--status": {"type": "string", "description": "Filter by status"},
                "--priority": {"type": "string", "description": "Filter by priority"},
            },
            "output": "Array of TaskFile objects",
        },
    }

    s: dict[str, Any] = {
        "name": "taskmd",
        "description": "Markdown-native task management. Each task is a file. The filename encodes all metadata; the body is free-form markdown with no frontmatter. No database, no config — the filesystem is the data store, git is the audit trail.",
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
            "body": "Free-form markdown. No frontmatter — all task metadata lives in the filename.",
        },
        "valid_statuses": sorted(VALID_STATUSES),
        "valid_priorities": sorted(VALID_PRIORITIES),
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

    s["guidance"] = [
        "Use 'taskmd new' to create tasks. Do NOT hand-craft filenames or pattern-match ID prefixes you see on disk — 'new' allocates the ID, formats the filename, and writes the file atomically. Mimicking an on-disk ID is the #1 cause of duplicate-ID bugs.",
        "Tasks are markdown files with no frontmatter. The filename encodes id, priority, status, and slug. After 'new' creates them, edit the body directly — that's the primary interface.",
        "A task tracks work blocked by something: user input, a different environment, passage of time, or an unmade decision. If nothing blocks you from doing it now, just do it instead of creating a task.",
        "To change a task's status, use 'taskmd status <id> <new-status>' — it renames the file atomically.",
    ]

    if not compact:
        s["guidance"] += [
            "Filenames use double-dash before the slug: 'status--slug', not 'status-slug'. Slugs are kebab-case, 3-5 words. 'taskmd new' handles this for you — you only supply --slug.",
            "'taskmd next' returns an ID without claiming it and is discouraged — it's kept only for integrations that do their own write path. Two concurrent 'next' callers can get the same ID.",
            "Run 'taskmd validate' after editing task files to catch duplicate IDs or malformed filenames early.",
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
                    "echo 'Fix the login redirect loop when JWT is expired.' | taskmd new --slug fix-login",
                    "# Or, with a prewritten body file and non-default priority:",
                    "#   cat body.md | taskmd new --slug fix-login --priority p1",
                    "# Body on stdin is REQUIRED — a task with no description is a placeholder.",
                    "taskmd validate  # confirm it's valid",
                ],
            },
            {
                "name": "Change task status",
                "steps": [
                    "taskmd status 34042 in-progress  # renames the file in one step",
                    "taskmd status 34042 done         # same path when finishing a task",
                    "# 'status' refuses to clobber an existing file and rejects invalid statuses up front.",
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
    return json.dumps(schema(compact), separators=(",", ":"), sort_keys=True)
