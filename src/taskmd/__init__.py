"""taskmd — Markdown-native task management.

Library interface:

    from taskmd import validate, fix, next_id, parse_task_file

CLI interface:

    taskmd validate [tasks/]
    taskmd fix [tasks/]
    taskmd next [tasks/]
"""

from taskmd.core import (
    VALID_PRIORITIES,
    VALID_STATUSES,
    CreateResult,
    FixResult,
    TaskFile,
    ValidationResult,
    create_task,
    fix,
    get_expected_filename,
    list_tasks,
    next_id,
    parse_task_file,
    validate,
)

__all__ = [
    "validate",
    "fix",
    "next_id",
    "create_task",
    "list_tasks",
    "parse_task_file",
    "get_expected_filename",
    "VALID_STATUSES",
    "VALID_PRIORITIES",
    "ValidationResult",
    "FixResult",
    "CreateResult",
    "TaskFile",
]
