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
    EnsureResult,
    FixResult,
    InitResult,
    TaskFile,
    ValidationResult,
    ancillary_files_for,
    create_task,
    ensure_initialized,
    find_task_by_id,
    find_task_by_slug,
    fix,
    get_expected_filename,
    init,
    list_tasks,
    next_id,
    parse_task_file,
    update_task,
    validate,
)

__all__ = [
    "validate",
    "fix",
    "init",
    "ensure_initialized",
    "next_id",
    "create_task",
    "update_task",
    "list_tasks",
    "find_task_by_id",
    "find_task_by_slug",
    "ancillary_files_for",
    "parse_task_file",
    "get_expected_filename",
    "VALID_STATUSES",
    "VALID_PRIORITIES",
    "ValidationResult",
    "FixResult",
    "InitResult",
    "EnsureResult",
    "CreateResult",
    "TaskFile",
]
