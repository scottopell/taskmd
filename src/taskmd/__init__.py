"""taskmd — Markdown-native task management.

Library interface:

    from taskmd import validate, fix, next_id, parse_task_file

CLI interface:

    taskmd validate [tasks/]
    taskmd fix [tasks/]
    taskmd next [tasks/]
"""

from taskmd.core import (
    VALID_FIELDS,
    VALID_PRIORITIES,
    VALID_STATUSES,
    FixResult,
    TaskFile,
    ValidationResult,
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
    "list_tasks",
    "parse_task_file",
    "get_expected_filename",
    "VALID_STATUSES",
    "VALID_PRIORITIES",
    "VALID_FIELDS",
    "ValidationResult",
    "FixResult",
    "TaskFile",
]
