"""taskmd CLI — thin wrapper over the library.

Usage:
    taskmd [--agent] [--output json|text] <command> [options] [tasks_dir]
    taskmd validate [tasks/]
    taskmd fix [tasks/]
    taskmd next [tasks/]
    taskmd list [--status STATUS] [--priority PRIORITY] [tasks/]
"""

from __future__ import annotations

import json
import sys
from dataclasses import asdict
from pathlib import Path

from taskmd.agent import (
    error_envelope,
    is_agent_mode,
    schema_json,
    success_envelope,
)
from taskmd.core import (
    VALID_PRIORITIES,
    VALID_STATUSES,
    fix,
    list_tasks,
    next_number,
    validate,
)

_DEFAULT_DIRS = ("tasks", "tasksmd")

_HELP_TEXT = """\
Usage: taskmd [--agent] [--output json|text] <command> [options] [tasks_dir]

Commands:
  validate   Check all task files for consistency
  fix        Auto-repair fixable issues (missing dates, mismatched filenames)
  next       Print the next available task number
  list       List all task files with metadata

Options:
  --agent         Force agent mode (JSON output, structured --help)
  --output FMT    Output format: json or text (default: text, json in agent mode)
  --compact       With --help in agent mode: minimal schema (fewer tokens)
  --status S      (list) Filter by status
  --priority P    (list) Filter by priority

Arguments:
  tasks_dir       Path to tasks directory (default: ./tasks or ./tasksmd)"""


def _resolve_tasks_dir() -> Path:
    """Return the first existing candidate directory, or 'tasks' as fallback."""
    for name in _DEFAULT_DIRS:
        p = Path(name)
        if p.is_dir():
            return p
    return Path(_DEFAULT_DIRS[0])


def _parse_args(argv: list[str]) -> dict:
    """Hand-rolled arg parser. Extracts global flags, command, and command args."""
    opts: dict = {
        "agent": False,
        "output": None,  # None means "auto" — text for humans, json for agents
        "compact": False,
        "help": False,
        "command": None,
        "tasks_dir": None,
        "status": None,
        "priority": None,
    }

    positional: list[str] = []
    i = 0
    while i < len(argv):
        arg = argv[i]
        if arg in ("-h", "--help"):
            opts["help"] = True
        elif arg == "--agent":
            opts["agent"] = True
        elif arg == "--compact":
            opts["compact"] = True
        elif arg == "--output" and i + 1 < len(argv):
            i += 1
            opts["output"] = argv[i]
        elif arg.startswith("--output="):
            opts["output"] = arg.split("=", 1)[1]
        elif arg == "--status" and i + 1 < len(argv):
            i += 1
            opts["status"] = argv[i]
        elif arg.startswith("--status="):
            opts["status"] = arg.split("=", 1)[1]
        elif arg == "--priority" and i + 1 < len(argv):
            i += 1
            opts["priority"] = argv[i]
        elif arg.startswith("--priority="):
            opts["priority"] = arg.split("=", 1)[1]
        elif arg.startswith("-"):
            print(f"Unknown flag: {arg}", file=sys.stderr)
            print("Run 'taskmd --help' for usage.", file=sys.stderr)
            sys.exit(1)
        else:
            positional.append(arg)
        i += 1

    if positional:
        opts["command"] = positional[0]
    if len(positional) > 1:
        opts["tasks_dir"] = Path(positional[1])

    return opts


def _use_json(opts: dict) -> bool:
    """Determine whether to output JSON."""
    if opts["output"] == "json":
        return True
    if opts["output"] == "text":
        return False
    # Auto: JSON in agent mode, text otherwise
    return is_agent_mode(opts["agent"])


def _task_to_dict(task) -> dict:
    """Convert a TaskFile to a JSON-friendly dict."""
    return {
        "number": task.number,
        "priority": task.priority,
        "status": task.status,
        "slug": task.slug,
        "path": str(task.path),
        "fields": task.fields,
    }


def main(argv: list[str] | None = None) -> None:
    args = argv if argv is not None else sys.argv[1:]
    opts = _parse_args(args)
    use_json = _use_json(opts)

    # --help
    if opts["help"] or opts["command"] is None:
        if is_agent_mode(opts["agent"]):
            print(schema_json(compact=opts["compact"]))
        else:
            print(_HELP_TEXT)
        sys.exit(0)

    command = opts["command"]
    tasks_dir = opts["tasks_dir"] or _resolve_tasks_dir()

    if command == "validate":
        result = validate(tasks_dir)
        if use_json:
            if result.errors:
                print(error_envelope(
                    "validate",
                    result.errors,
                    suggestions=["Run 'taskmd fix' to auto-repair fixable issues"],
                ))
                sys.exit(1)
            else:
                print(success_envelope(
                    "validate",
                    {"file_count": result.file_count, "errors": []},
                    file_count=result.file_count,
                ))
        else:
            if result.errors:
                print(f"\u2717 {len(result.errors)} task validation error(s):")
                for err in result.errors:
                    print(f"  - {err}")
                print()
                print("Run 'taskmd fix' to auto-fix (injects missing 'created', renames files).")
                sys.exit(1)
            else:
                print(f"\u2713 {result.file_count} task files validated")

    elif command == "fix":
        result = fix(tasks_dir)
        if use_json:
            if result.errors:
                print(error_envelope("fix", result.errors))
                sys.exit(1)
            else:
                print(success_envelope(
                    "fix",
                    {
                        "patched": result.patched,
                        "renamed": result.renamed,
                        "patches": [{"file": f, "date": d} for f, d in result.patches],
                        "renames": [{"old": o, "new": n} for o, n in result.renames],
                    },
                    patched=result.patched,
                    renamed=result.renamed,
                ))
        else:
            if result.errors:
                print(f"\u2717 {len(result.errors)} error(s):")
                for err in result.errors:
                    print(f"  - {err}")
                sys.exit(1)
            else:
                for old, new in result.renames:
                    print(f"  {old} -> {new}")
                print(f"\u2713 {result.summary()}")

    elif command == "next":
        n = next_number(tasks_dir)
        if use_json:
            print(success_envelope("next", {"next_number": n}))
        else:
            print(f"{n:04d}")

    elif command == "list":
        tasks = list_tasks(tasks_dir)
        # Apply filters
        if opts["status"]:
            if opts["status"] not in VALID_STATUSES:
                msg = f"invalid status '{opts['status']}' (valid: {', '.join(sorted(VALID_STATUSES))})"
                if use_json:
                    print(error_envelope("list", [msg]))
                else:
                    print(f"Error: {msg}", file=sys.stderr)
                sys.exit(1)
            tasks = [t for t in tasks if t.status == opts["status"]]
        if opts["priority"]:
            if opts["priority"] not in VALID_PRIORITIES:
                msg = f"invalid priority '{opts['priority']}' (valid: {', '.join(sorted(VALID_PRIORITIES))})"
                if use_json:
                    print(error_envelope("list", [msg]))
                else:
                    print(f"Error: {msg}", file=sys.stderr)
                sys.exit(1)
            tasks = [t for t in tasks if t.priority == opts["priority"]]

        if use_json:
            print(success_envelope(
                "list",
                [_task_to_dict(t) for t in tasks],
                count=len(tasks),
            ))
        else:
            if not tasks:
                print("No tasks found.")
            else:
                for t in tasks:
                    print(f"{t.number:04d}  {t.priority}  {t.status:14s}  {t.slug}")

    else:
        msg = f"Unknown command: {command}"
        if use_json:
            print(error_envelope(
                command,
                [msg],
                suggestions=["Run 'taskmd --help' for available commands"],
            ))
        else:
            print(msg, file=sys.stderr)
            print("Run 'taskmd --help' for usage.", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
