"""taskmd CLI — thin wrapper over the library.

Usage:
    taskmd validate [tasks/]
    taskmd fix [tasks/]
    taskmd next [tasks/]
"""

from __future__ import annotations

import sys
from pathlib import Path

from taskmd.core import fix, next_number, validate


def main(argv: list[str] | None = None) -> None:
    args = argv if argv is not None else sys.argv[1:]

    if not args or args[0] in ("-h", "--help"):
        print("Usage: taskmd <command> [tasks_dir]")
        print()
        print("Commands:")
        print("  validate   Check all task files for consistency")
        print("  fix        Auto-repair fixable issues (missing dates, mismatched filenames)")
        print("  next       Print the next available task number")
        print()
        print("Arguments:")
        print("  tasks_dir  Path to tasks directory (default: ./tasks)")
        sys.exit(0)

    command = args[0]
    tasks_dir = Path(args[1]) if len(args) > 1 else Path("tasks")

    if command == "validate":
        result = validate(tasks_dir)
        if result.errors:
            print(f"✗ {len(result.errors)} task validation error(s):")
            for err in result.errors:
                print(f"  - {err}")
            print()
            print("Run 'taskmd fix' to auto-fix (injects missing 'created', renames files).")
            sys.exit(1)
        else:
            print(f"✓ {result.file_count} task files validated")

    elif command == "fix":
        result = fix(tasks_dir)
        if result.errors:
            print(f"✗ {len(result.errors)} error(s):")
            for err in result.errors:
                print(f"  - {err}")
            sys.exit(1)
        else:
            for old, new in result.renames:
                print(f"  {old} -> {new}")
            print(f"✓ {result.summary()}")

    elif command == "next":
        n = next_number(tasks_dir)
        print(f"{n:04d}")

    else:
        print(f"Unknown command: {command}", file=sys.stderr)
        print("Run 'taskmd --help' for usage.", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
