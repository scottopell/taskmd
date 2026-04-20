"""taskmd CLI — thin wrapper over the library.

Usage:
    taskmd [--agent] [--output json|text] <command> [options] [tasks_dir]
    taskmd new --slug S --artifact A [--priority P] [--status ST] [tasks/] < body.md
    taskmd validate [tasks/]
    taskmd fix [tasks/]
    taskmd next [tasks/]
    taskmd list [--status STATUS] [--priority PRIORITY] [tasks/]
"""

from __future__ import annotations

import sys
from importlib.metadata import version as _pkg_version
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
    create_task,
    fix,
    init,
    list_tasks,
    next_id,
    validate,
)

_DEFAULT_DIRS = ("tasks", "tasksmd")

_HELP_TEXT = """\
Usage: taskmd [--agent] [--output json|text] <command> [options] [tasks_dir]

Commands:
  init       Create a new tasks directory with a template file
  new        Create a new task atomically (ID + filename + frontmatter + write)
  validate   Check all task files for consistency
  fix        Auto-repair fixable issues (missing dates, mismatched filenames, legacy naming)
  next       Print the next available task ID (advisory; prefer 'new' for creation)
  list       List all task files with metadata

Options:
  --version, -V     Print version and exit
  --agent           Force agent mode (JSON output, structured --help)
  --output FMT      Output format: json or text (default: text, json in agent mode)
  --compact         With --help in agent mode: minimal schema (fewer tokens)
  --slug S          (new) URL-safe slug; dirty input is normalized. Required.
  --artifact A      (new) Concrete output the task produces (file path, commit, etc.). Required.
  --priority P      (new, list) Priority (default: p2 for 'new')
  --status S        (new, list) Status (default: ready for 'new')

Arguments:
  tasks_dir         Path to tasks directory (default: ./tasks or ./tasksmd)

Creating a task:
  echo "what this task is about" | taskmd new --slug fix-login --artifact src/auth.py
  cat body.md                    | taskmd new --slug fix-login --artifact src/auth.py --priority p1

  A task body is REQUIRED on stdin. A task without a description is a
  placeholder, and placeholders inflate the triage surface area.

'new' vs 'next':
  'new' is the recommended path — it allocates the ID, formats the filename,
  synthesizes the frontmatter, and writes the file in one atomic step.
  'next' returns just an ID string; callers are responsible for writing the
  file themselves, which is a sharp edge (two concurrent 'next' callers can
  receive the same ID)."""


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
        "version": False,
        "command": None,
        "tasks_dir": None,
        "status": None,
        "priority": None,
        "slug": None,
        "artifact": None,
    }

    positional: list[str] = []
    i = 0
    while i < len(argv):
        arg = argv[i]
        if arg in ("-h", "--help"):
            opts["help"] = True
        elif arg in ("-V", "--version"):
            opts["version"] = True
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
        elif arg == "--slug" and i + 1 < len(argv):
            i += 1
            opts["slug"] = argv[i]
        elif arg.startswith("--slug="):
            opts["slug"] = arg.split("=", 1)[1]
        elif arg == "--artifact" and i + 1 < len(argv):
            i += 1
            opts["artifact"] = argv[i]
        elif arg.startswith("--artifact="):
            opts["artifact"] = arg.split("=", 1)[1]
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
        "id": task.id,
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

    # --version
    if opts["version"]:
        print(f"taskmd {_pkg_version('taskmd')}")
        sys.exit(0)

    # --help
    if opts["help"] or opts["command"] is None:
        if is_agent_mode(opts["agent"]):
            print(schema_json(compact=opts["compact"]))
        else:
            print(_HELP_TEXT)
        sys.exit(0)

    command = opts["command"]

    if command == "init":
        tasks_dir = opts["tasks_dir"] or Path(_DEFAULT_DIRS[0])
        result = init(tasks_dir)
        if use_json:
            if result.ok:
                print(success_envelope(
                    "init",
                    {
                        "tasks_dir": str(result.tasks_dir),
                        "created": result.created,
                        "template_fields": result.template_fields,
                    },
                ))
            else:
                assert result.error is not None  # ok is False → error is set
                print(error_envelope(
                    "init",
                    [result.error],
                    suggestions=["Use 'taskmd validate' to check existing tasks"],
                ))
                sys.exit(1)
        else:
            if result.ok:
                for path in result.created:
                    print(f"  created {path}")
                print(f"Created {tasks_dir}/ with _TEMPLATE.md")
            else:
                print(f"Error: {result.error}", file=sys.stderr)
                sys.exit(1)
        return

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
                print(error_envelope(
                    "fix",
                    result.errors,
                    suggestions=["Run 'taskmd validate' to see all issues", "Check that task files have valid frontmatter (status, priority, created, artifact)"],
                ))
                sys.exit(1)
            else:
                print(success_envelope(
                    "fix",
                    {
                        "patched": result.patched,
                        "renamed": result.renamed,
                        "migrated": result.migrated,
                        "patches": [{"file": f, "date": d} for f, d in result.patches],
                        "renames": [{"old": o, "new": n} for o, n in result.renames],
                    },
                    patched=result.patched,
                    renamed=result.renamed,
                    migrated=result.migrated,
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
                if result.migrated:
                    print(f"  Note: {result.migrated} file(s) migrated to numeric ID format")
                print(f"\u2713 {result.summary()}")

    elif command == "next":
        # 'next' is the sharp edge — two callers in the same (hostname, dir)
        # partition can receive the same ID because allocation isn't tied to
        # a write. Nudge callers toward 'taskmd new', which is atomic.
        print(
            "warning: 'taskmd next' returns an ID without claiming it; "
            "two concurrent callers can receive the same ID. "
            "Prefer 'taskmd new' (stdin for body) unless you have a reason "
            "to write the file yourself.",
            file=sys.stderr,
        )
        n = next_id(tasks_dir)
        if use_json:
            print(success_envelope("next", {"next_id": n}))
        else:
            print(n)

    elif command == "new":
        slug = opts["slug"]
        artifact = opts["artifact"]
        priority = opts["priority"] or "p2"
        status = opts["status"] or "ready"

        missing = []
        if not slug:
            missing.append("--slug")
        if not artifact:
            missing.append("--artifact")
        if missing:
            msg = f"'new' requires {' and '.join(missing)}"
            if use_json:
                print(error_envelope(
                    "new",
                    [msg],
                    suggestions=[
                        "taskmd new --slug my-task --artifact src/foo.py [--priority p2] [--status ready] < body.md",
                        "Body is read from stdin; omit stdin for a template skeleton.",
                    ],
                ))
            else:
                print(f"Error: {msg}", file=sys.stderr)
                print("Run 'taskmd --help' for usage.", file=sys.stderr)
            sys.exit(1)

        if priority not in VALID_PRIORITIES:
            msg = f"invalid priority '{priority}' (valid: {', '.join(sorted(VALID_PRIORITIES))})"
            if use_json:
                print(error_envelope("new", [msg]))
            else:
                print(f"Error: {msg}", file=sys.stderr)
            sys.exit(1)
        if status not in VALID_STATUSES:
            msg = f"invalid status '{status}' (valid: {', '.join(sorted(VALID_STATUSES))})"
            if use_json:
                print(error_envelope("new", [msg]))
            else:
                print(f"Error: {msg}", file=sys.stderr)
            sys.exit(1)

        # Body is required. Detect the interactive-tty case up front so the
        # caller gets a specific, actionable error instead of the generic
        # "body is required" from core. Empty piped stdin still falls
        # through to core's validation.
        if sys.stdin.isatty():
            msg = "'new' requires a task body on stdin"
            if use_json:
                print(error_envelope(
                    "new",
                    [msg],
                    suggestions=[
                        "echo 'what this task is about' | taskmd new --slug ... --artifact ...",
                        "cat body.md | taskmd new --slug ... --artifact ...",
                        "A task with no body is a placeholder — if you cannot describe it, do not create it yet.",
                    ],
                ))
            else:
                print(f"Error: {msg} (pipe a description on stdin)", file=sys.stderr)
                print("  echo 'what this task is about' | taskmd new --slug ... --artifact ...", file=sys.stderr)
            sys.exit(1)

        body = sys.stdin.read()

        try:
            result = create_task(
                tasks_dir,
                slug=slug,
                artifact=artifact,
                priority=priority,
                status=status,
                body=body,
            )
        except RuntimeError as e:
            msg = str(e)
            # Only offer suggestions that actually match the underlying error,
            # otherwise the output misleads (e.g. "Run taskmd init" shown for
            # a body-missing failure).
            suggestions: list[str] = []
            if "tasks directory does not exist" in msg:
                suggestions.append("Run 'taskmd init' first")
            if "body is required" in msg:
                suggestions.append(
                    "Pipe a description on stdin: echo 'desc' | taskmd new --slug ... --artifact ..."
                )
            if use_json:
                print(error_envelope("new", [msg], suggestions=suggestions or None))
            else:
                print(f"Error: {msg}", file=sys.stderr)
            sys.exit(1)

        if use_json:
            print(success_envelope(
                "new",
                {
                    "id": result.id,
                    "path": str(result.path),
                    "filename": result.filename,
                },
            ))
        else:
            print(f"created {result.path}")

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
                    print(f"{t.id:5s}  {t.priority}  {t.status:14s}  {t.slug}")

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
