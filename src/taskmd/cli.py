"""taskmd CLI — thin wrapper over the library.

Usage:
    taskmd [--agent] [--output json|text] <command> [options] [tasks_dir]
    taskmd new --slug S [--priority P] [--status ST] [tasks/] < body.md
    taskmd status <id> <new-status> [tasks/]
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
    find_task_by_id,
    fix,
    init,
    list_tasks,
    next_id,
    rename_status,
    validate,
)

_DEFAULT_DIRS = ("tasks", "taskmds")

_HELP_TEXT = """\
Usage: taskmd [--agent] [--output json|text] <command> [options] [tasks_dir]

Commands:
  init       Create a new tasks directory with a template file
  new        Create a new task atomically (ID + filename + body write)
  status     Change a task's status atomically (renames the file)
  validate   Check all task filenames for consistency
  fix        Auto-repair fixable issues (legacy ID formats, duplicate IDs)
  next       Print the next available task ID (advisory; prefer 'new' for creation)
  list       List all task files with metadata

Options:
  --version, -V     Print version and exit
  --agent           Force agent mode (JSON output, structured --help)
  --output FMT      Output format: json or text (default: text, json in agent mode)
  --compact         With --help in agent mode: minimal schema (fewer tokens)
  --slug S          (new) URL-safe slug; dirty input is normalized. Required.
  --priority P      (new, list) Priority (default: p2 for 'new')
  --status S        (new, list) Status (default: ready for 'new')
  --migrate         (fix) Strip legacy YAML frontmatter from task files
  --no-migrate      (fix) Skip the frontmatter migration check

Arguments:
  tasks_dir         Path to tasks directory (default: ./tasks or ./taskmds)

Creating a task:
  echo "what this task is about" | taskmd new --slug fix-login
  cat body.md                    | taskmd new --slug fix-login --priority p1

  A task body is REQUIRED on stdin. The body is written verbatim; all
  metadata lives in the filename.

Changing status:
  taskmd status 34042 in-progress
  taskmd status 34042 done

  Renames the file to reflect the new status.

'new' vs 'next':
  'new' is the recommended path — it allocates the ID, formats the filename,
  and writes the file in one atomic step.
  'next' returns just an ID string; callers are responsible for writing the
  file themselves, which is a sharp edge (two concurrent 'next' callers can
  receive the same ID)."""


def _resolve_tasks_dir() -> Path:
    """Return the first existing candidate directory, or 'tasks' as fallback."""
    for name in reversed(_DEFAULT_DIRS):
        p = Path(name)
        if p.is_dir():
            return p
    return Path(_DEFAULT_DIRS[0])


def _resolve_init_dir() -> Path:
    """Return the default dir to create on `init`."""
    primary = Path(_DEFAULT_DIRS[0])
    if primary.exists():
        return Path(_DEFAULT_DIRS[1])
    return primary


def _parse_args(argv: list[str]) -> dict:
    """Hand-rolled arg parser. Extracts global flags, command, and command args."""
    opts: dict = {
        "agent": False,
        "output": None,
        "compact": False,
        "help": False,
        "version": False,
        "command": None,
        "tasks_dir": None,
        "status": None,
        "priority": None,
        "slug": None,
        "migrate": None,  # None = prompt, True = migrate, False = skip
        "positional": [],
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
        elif arg == "--migrate":
            opts["migrate"] = True
        elif arg == "--no-migrate":
            opts["migrate"] = False
        elif arg.startswith("-"):
            print(f"Unknown flag: {arg}", file=sys.stderr)
            print("Run 'taskmd --help' for usage.", file=sys.stderr)
            sys.exit(1)
        else:
            positional.append(arg)
        i += 1

    if positional:
        opts["command"] = positional[0]
    if len(positional) > 1 and positional[0] != "status":
        opts["tasks_dir"] = Path(positional[1])
    opts["positional"] = positional

    return opts


def _use_json(opts: dict) -> bool:
    if opts["output"] == "json":
        return True
    if opts["output"] == "text":
        return False
    return is_agent_mode(opts["agent"])


def _task_to_dict(task) -> dict:
    return {
        "id": task.id,
        "priority": task.priority,
        "status": task.status,
        "slug": task.slug,
        "path": str(task.path),
    }


def main(argv: list[str] | None = None) -> None:
    args = argv if argv is not None else sys.argv[1:]
    opts = _parse_args(args)
    use_json = _use_json(opts)

    if opts["version"]:
        print(f"taskmd {_pkg_version('taskmd')}")
        sys.exit(0)

    if opts["help"] or opts["command"] is None:
        if is_agent_mode(opts["agent"]):
            print(schema_json(compact=opts["compact"]))
        else:
            print(_HELP_TEXT)
        sys.exit(0)

    command = opts["command"]

    if command == "init":
        tasks_dir = opts["tasks_dir"] or _resolve_init_dir()
        result = init(tasks_dir)
        if use_json:
            if result.ok:
                print(success_envelope(
                    "init",
                    {
                        "tasks_dir": str(result.tasks_dir),
                        "created": result.created,
                    },
                ))
            else:
                assert result.error is not None
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
                # Tailor suggestions to the kinds of errors present. `fix`
                # only handles duplicate IDs and legacy ID migration; it
                # cannot repair a filename that doesn't match the pattern.
                has_duplicate = any("duplicate task id" in e for e in result.errors)
                has_pattern = any("doesn't match pattern" in e for e in result.errors)
                suggestions: list[str] = []
                if has_duplicate:
                    suggestions.append(
                        "Run 'taskmd fix' to auto-renumber duplicate IDs"
                    )
                if has_pattern:
                    suggestions.append(
                        "Rename non-conforming files to match DDNNN-pX-status--slug.md "
                        "(taskmd fix cannot repair these)"
                    )
                print(error_envelope(
                    "validate",
                    result.errors,
                    suggestions=suggestions or None,
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
                print(f"✗ {len(result.errors)} task validation error(s):")
                for err in result.errors:
                    print(f"  - {err}")
                print()
                if any("duplicate task id" in e for e in result.errors):
                    print("Run 'taskmd fix' to auto-renumber duplicate IDs.")
                if any("doesn't match pattern" in e for e in result.errors):
                    print(
                        "Rename non-conforming files to match "
                        "DDNNN-pX-status--slug.md (taskmd fix cannot repair these)."
                    )
                sys.exit(1)
            else:
                print(f"✓ {result.file_count} task files validated")

    elif command == "fix":
        result = fix(tasks_dir, migrate=opts["migrate"])
        if use_json:
            if result.errors:
                # If the migration prompt fired, surface the pending file
                # list so agents can act on it.
                data: dict | None = None
                suggestions: list[str]
                if result.frontmatter_pending:
                    data = {"frontmatter_pending": list(result.frontmatter_pending)}
                    suggestions = [
                        "Run 'taskmd fix --migrate' to strip the frontmatter (destructive — commit first)",
                        "Run 'taskmd fix --no-migrate' to skip the check this run",
                    ]
                else:
                    suggestions = ["Run 'taskmd validate' to see all issues"]
                print(error_envelope(
                    "fix",
                    result.errors,
                    suggestions=suggestions,
                    data=data,
                ))
                sys.exit(1)
            else:
                print(success_envelope(
                    "fix",
                    {
                        "renamed": result.renamed,
                        "migrated": result.migrated,
                        "renames": [{"old": o, "new": n} for o, n in result.renames],
                        "renumbered": [
                            {
                                "old_id": oid,
                                "new_id": nid,
                                "old_filename": old,
                                "new_filename": new,
                            }
                            for oid, nid, old, new in result.renumbered
                        ],
                        "frontmatter_stripped": list(result.frontmatter_stripped),
                    },
                    renamed=result.renamed,
                    migrated=result.migrated,
                    renumbered=len(result.renumbered),
                    frontmatter_stripped=len(result.frontmatter_stripped),
                ))
        else:
            if result.errors:
                # The migration-prompt error has its own dedicated layout.
                if result.frontmatter_pending:
                    n = len(result.frontmatter_pending)
                    print(
                        f"✗ {n} task file(s) have legacy YAML frontmatter "
                        "that must be removed:"
                    )
                    for name in result.frontmatter_pending:
                        print(f"  - {name}")
                    print()
                    print(
                        "Frontmatter is no longer used; the filename is the sole "
                        "source of truth."
                    )
                    print()
                    print("  taskmd fix --migrate     "
                          "# strip the frontmatter (destructive — commit first)")
                    print("  taskmd fix --no-migrate  "
                          "# skip the migration check this run")
                else:
                    print(f"✗ {len(result.errors)} error(s):")
                    for err in result.errors:
                        print(f"  - {err}")
                sys.exit(1)
            else:
                for name in result.frontmatter_stripped:
                    print(f"  stripped frontmatter: {name}")
                for old, new in result.renames:
                    print(f"  {old} -> {new}")
                if result.migrated:
                    print(f"  Note: {result.migrated} file(s) migrated to numeric ID format")
                for old_id, new_id, old_name, new_name in result.renumbered:
                    print(f"  renumbered: {old_id} -> {new_id} ({old_name} -> {new_name})")
                if result.renumbered:
                    print(
                        "  Note: cross-references to old IDs are NOT rewritten; "
                        "grep the mapping above."
                    )
                print(f"✓ {result.summary()}")

    elif command == "next":
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
        priority = opts["priority"] or "p2"
        status = opts["status"] or "ready"

        if not slug:
            msg = "'new' requires --slug"
            if use_json:
                print(error_envelope(
                    "new",
                    [msg],
                    suggestions=[
                        "taskmd new --slug my-task [--priority p2] [--status ready] < body.md",
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

        if sys.stdin.isatty():
            msg = "'new' requires a task body on stdin"
            if use_json:
                print(error_envelope(
                    "new",
                    [msg],
                    suggestions=[
                        "echo 'what this task is about' | taskmd new --slug ...",
                        "cat body.md | taskmd new --slug ...",
                    ],
                ))
            else:
                print(f"Error: {msg} (pipe a description on stdin)", file=sys.stderr)
                print("  echo 'what this task is about' | taskmd new --slug ...", file=sys.stderr)
            sys.exit(1)

        body = sys.stdin.read()

        try:
            result = create_task(
                tasks_dir,
                slug=slug,
                priority=priority,
                status=status,
                body=body,
            )
        except RuntimeError as e:
            msg = str(e)
            suggestions: list[str] = []
            if "tasks directory does not exist" in msg:
                suggestions.append("Run 'taskmd init' first")
            if "body is required" in msg:
                suggestions.append(
                    "Pipe a description on stdin: echo 'desc' | taskmd new --slug ..."
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

    elif command == "status":
        positional = opts["positional"]
        task_id = positional[1] if len(positional) > 1 else None
        new_status = positional[2] if len(positional) > 2 else None
        status_tasks_dir = (
            Path(positional[3]) if len(positional) > 3 else _resolve_tasks_dir()
        )

        if not task_id or not new_status:
            msg = "'status' requires <id> and <new-status>"
            if use_json:
                print(error_envelope(
                    "status",
                    [msg],
                    suggestions=[
                        "taskmd status <id> <new-status> [tasks_dir]",
                        "Example: taskmd status 34042 in-progress",
                        f"Valid statuses: {', '.join(sorted(VALID_STATUSES))}",
                    ],
                ))
            else:
                print(f"Error: {msg}", file=sys.stderr)
                print("  taskmd status <id> <new-status> [tasks_dir]", file=sys.stderr)
            sys.exit(1)

        if new_status not in VALID_STATUSES:
            msg = f"invalid status '{new_status}' (valid: {', '.join(sorted(VALID_STATUSES))})"
            if use_json:
                print(error_envelope("status", [msg]))
            else:
                print(f"Error: {msg}", file=sys.stderr)
            sys.exit(1)

        existing = find_task_by_id(status_tasks_dir, task_id)
        if existing is None:
            msg = f"task {task_id} not found in {status_tasks_dir}"
            if use_json:
                print(error_envelope(
                    "status",
                    [msg],
                    suggestions=[
                        "Run 'taskmd list' to see available task IDs",
                    ],
                ))
            else:
                print(f"Error: {msg}", file=sys.stderr)
            sys.exit(1)

        try:
            old_filename, new_filename = rename_status(
                status_tasks_dir, task_id, new_status
            )
        except RuntimeError as e:
            msg = str(e)
            suggestions: list[str] = []
            if "target already exists" in msg:
                suggestions.append(
                    "Resolve the filename collision (the target name is already taken — likely a duplicate slug)."
                )
            if use_json:
                print(error_envelope("status", [msg], suggestions=suggestions or None))
            else:
                print(f"Error: {msg}", file=sys.stderr)
            sys.exit(1)

        if use_json:
            print(success_envelope(
                "status",
                {
                    "id": task_id,
                    "old_filename": old_filename,
                    "new_filename": new_filename,
                    "old_status": existing.status,
                    "new_status": new_status,
                },
            ))
        else:
            print(f"  {old_filename} -> {new_filename}")

    elif command == "list":
        tasks = list_tasks(tasks_dir)
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
