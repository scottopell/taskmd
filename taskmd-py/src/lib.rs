//! PyO3 Python extension module for taskmd.
//!
//! Compiled by maturin as `taskmd._core`. All logic lives in taskmd-core;
//! this crate is a thin binding layer.

use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::path::Path;
use std::str::FromStr;
use taskmd_core::constants::{Priority, Status};
use taskmd_core::{constants, create, discover, filename, fix, ids, init, tasks, validate as vld};

// ── Task dict helper ──────────────────────────────────────────────────────────

fn task_to_dict<'py>(
    py: Python<'py>,
    task: tasks::TaskFile,
) -> PyResult<pyo3::Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("path", task.path.to_string_lossy().as_ref())?;
    dict.set_item("id", &task.id)?;
    dict.set_item("priority", task.priority.as_str())?;
    dict.set_item("status", task.status.as_str())?;
    dict.set_item("slug", &task.slug)?;
    Ok(dict)
}

// ── Internal helpers exposed for the test suite ───────────────────────────────

#[pyfunction]
fn task_files(tasks_dir: &str) -> PyResult<Vec<String>> {
    taskmd_core::tasks::task_files(Path::new(tasks_dir))
        .map(|v| v.into_iter().map(|p| p.to_string_lossy().into_owned()).collect())
        .map_err(|e| pyo3::exceptions::PyOSError::new_err(e.to_string()))
}

#[pyfunction]
fn is_legacy_id(task_id: &str) -> bool {
    ids::is_legacy_id(task_id)
}

#[pyfunction]
fn needs_migration(task_id: &str, expected_prefix: &str) -> bool {
    ids::needs_migration(task_id, expected_prefix)
}

#[pyfunction]
fn parse_id_parts(task_id: &str) -> (String, u32) {
    ids::parse_id_parts(task_id)
}

#[pyfunction]
fn prefix_for(tasks_dir: &str) -> String {
    ids::prefix_for(Path::new(tasks_dir))
}

// ── ID / filename / slug ─────────────────────────────────────────────────────

#[pyfunction]
fn next_id(tasks_dir: &str) -> String {
    ids::next_id(Path::new(tasks_dir))
}

#[pyfunction]
fn get_expected_filename(
    id: &str,
    priority: &str,
    status: &str,
    slug: &str,
) -> PyResult<String> {
    let prio = Priority::from_str(priority)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    let stat = Status::from_str(status)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    Ok(filename::format_filename(id, prio, stat, slug))
}

#[pyfunction]
fn derive_slug(title: &str) -> String {
    filename::derive_slug(title)
}

// ── Task file operations ──────────────────────────────────────────────────────

#[pyfunction]
fn parse_task_file(py: Python<'_>, path: &str) -> PyResult<Option<Py<PyAny>>> {
    match tasks::parse_task_file(Path::new(path)) {
        None => Ok(None),
        Some(task) => Ok(Some(task_to_dict(py, task)?.into_any().unbind())),
    }
}

#[pyfunction]
fn list_tasks(py: Python<'_>, tasks_dir: &str) -> PyResult<Vec<Py<PyAny>>> {
    taskmd_core::tasks::list_tasks(Path::new(tasks_dir))
        .into_iter()
        .map(|t| task_to_dict(py, t).map(|d| d.into_any().unbind()))
        .collect()
}

#[pyfunction]
fn find_task_by_id(py: Python<'_>, tasks_dir: &str, id: &str) -> PyResult<Option<Py<PyAny>>> {
    match tasks::find_task_by_id(Path::new(tasks_dir), id) {
        None => Ok(None),
        Some(task) => Ok(Some(task_to_dict(py, task)?.into_any().unbind())),
    }
}

#[pyfunction]
fn find_task_by_slug(
    py: Python<'_>,
    tasks_dir: &str,
    slug: &str,
) -> PyResult<Vec<Py<PyAny>>> {
    tasks::find_task_by_slug(Path::new(tasks_dir), slug)
        .into_iter()
        .map(|t| task_to_dict(py, t).map(|d| d.into_any().unbind()))
        .collect()
}

#[pyfunction]
fn ancillary_files_for(tasks_dir: &str, id: &str) -> PyResult<Vec<String>> {
    Ok(tasks::ancillary_files_for(Path::new(tasks_dir), id)
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect())
}

#[pyfunction]
#[pyo3(signature = (tasks_dir, id, priority=None, status=None, slug=None))]
fn update_task(
    tasks_dir: &str,
    id: &str,
    priority: Option<&str>,
    status: Option<&str>,
    slug: Option<String>,
) -> PyResult<(String, String)> {
    let priority = priority
        .map(Priority::from_str)
        .transpose()
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    let status = status
        .map(Status::from_str)
        .transpose()
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    let update = tasks::TaskUpdate {
        priority,
        status,
        slug,
    };
    let r = tasks::update_task(Path::new(tasks_dir), id, update)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
    Ok((r.old_filename, r.new_filename))
}

// ── Validate ─────────────────────────────────────────────────────────────────

#[pyfunction]
fn validate(py: Python<'_>, tasks_dir: &str) -> PyResult<Py<PyAny>> {
    let r = vld::validate(Path::new(tasks_dir));
    let dict = PyDict::new(py);
    dict.set_item("errors", r.errors)?;
    dict.set_item("file_count", r.file_count)?;
    Ok(dict.into_any().unbind())
}

// ── Fix ───────────────────────────────────────────────────────────────────────

#[pyfunction]
fn fix_summary(renamed: usize, migrated: usize, renumbered: usize, frontmatter_stripped: usize) -> String {
    fix::fix_summary(renamed, migrated, renumbered, frontmatter_stripped)
}

/// `migrate`: None (default) prompts, Some(true) strips frontmatter,
/// Some(false) skips the frontmatter check.
#[pyfunction]
#[pyo3(signature = (tasks_dir, migrate=None))]
fn do_fix(py: Python<'_>, tasks_dir: &str, migrate: Option<bool>) -> PyResult<Py<PyAny>> {
    let mode = match migrate {
        None => fix::MigrateMode::Prompt,
        Some(true) => fix::MigrateMode::Migrate,
        Some(false) => fix::MigrateMode::Skip,
    };
    let r = fix::fix(Path::new(tasks_dir), mode);
    let dict = PyDict::new(py);
    dict.set_item("renamed", r.renamed)?;
    dict.set_item("migrated", r.migrated)?;
    dict.set_item("renames", r.renames)?;
    dict.set_item("renumbered", r.renumbered)?;
    dict.set_item("frontmatter_stripped", r.frontmatter_stripped)?;
    dict.set_item("frontmatter_pending", r.frontmatter_pending)?;
    dict.set_item("errors", r.errors)?;
    Ok(dict.into_any().unbind())
}

// ── Create (atomic new-task) ──────────────────────────────────────────────────

#[pyfunction]
#[pyo3(signature = (tasks_dir, priority, status, slug, body))]
fn do_create(
    py: Python<'_>,
    tasks_dir: &str,
    priority: &str,
    status: &str,
    slug: &str,
    body: &str,
) -> PyResult<Py<PyAny>> {
    let prio = Priority::from_str(priority)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    let stat = Status::from_str(status)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

    let created = create::create_task(Path::new(tasks_dir), prio, stat, slug, body)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

    let dict = PyDict::new(py);
    dict.set_item("id", &created.id)?;
    dict.set_item("path", created.path.to_string_lossy().as_ref())?;
    dict.set_item("filename", &created.filename)?;
    Ok(dict.into_any().unbind())
}

// ── Init ──────────────────────────────────────────────────────────────────────

#[pyfunction]
fn do_init(py: Python<'_>, tasks_dir: &str) -> PyResult<Py<PyAny>> {
    let r = init::init(Path::new(tasks_dir));
    let dict = PyDict::new(py);
    dict.set_item("tasks_dir", r.tasks_dir.to_string_lossy().as_ref())?;
    dict.set_item("created", r.created)?;
    dict.set_item("error", r.error)?;
    Ok(dict.into_any().unbind())
}

#[pyfunction]
fn do_ensure_initialized(py: Python<'_>, tasks_dir: &str) -> PyResult<Py<PyAny>> {
    let r = init::ensure_initialized(Path::new(tasks_dir));
    let dict = PyDict::new(py);
    dict.set_item("tasks_dir", r.tasks_dir.to_string_lossy().as_ref())?;
    dict.set_item("created", r.created)?;
    dict.set_item("error", r.error)?;
    Ok(dict.into_any().unbind())
}

// ── Discovery ─────────────────────────────────────────────────────────────────

/// Scan `dir` for a taskmd tasks directory (a subdir holding `_TEMPLATE.md`).
///
/// Returns `(name, candidates)`:
///   - `(Some(name), [name])` — exactly one match.
///   - `(None, [])` — no match.
///   - `(None, [n1, n2, ...])` — 2+ matches, names sorted; caller disambiguates.
///
/// Names are relative (a single path component); the absolute path is
/// `os.path.join(dir, name)`.
#[pyfunction]
fn discover_tasks_dir(dir: &str) -> (Option<String>, Vec<String>) {
    let found = discover::candidates(Path::new(dir));
    let one = if found.len() == 1 {
        Some(found[0].clone())
    } else {
        None
    };
    (one, found)
}

/// Never-fails variant: prefer the conventional `tasks` name, else the
/// lexically-first candidate, else fall back to the bare name `tasks`.
/// Always returns a relative name.
#[pyfunction]
fn discover_tasks_dir_or_default(dir: &str) -> String {
    discover::discover_or_default(Path::new(dir))
        .to_string_lossy()
        .into_owned()
}

// ── Module ────────────────────────────────────────────────────────────────────

#[pymodule]
fn _core(m: &pyo3::Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(task_files, m)?)?;
    m.add_function(wrap_pyfunction!(is_legacy_id, m)?)?;
    m.add_function(wrap_pyfunction!(needs_migration, m)?)?;
    m.add_function(wrap_pyfunction!(parse_id_parts, m)?)?;
    m.add_function(wrap_pyfunction!(prefix_for, m)?)?;

    m.add_function(wrap_pyfunction!(next_id, m)?)?;
    m.add_function(wrap_pyfunction!(get_expected_filename, m)?)?;
    m.add_function(wrap_pyfunction!(derive_slug, m)?)?;

    m.add_function(wrap_pyfunction!(parse_task_file, m)?)?;
    m.add_function(wrap_pyfunction!(list_tasks, m)?)?;
    m.add_function(wrap_pyfunction!(find_task_by_id, m)?)?;
    m.add_function(wrap_pyfunction!(find_task_by_slug, m)?)?;
    m.add_function(wrap_pyfunction!(ancillary_files_for, m)?)?;
    m.add_function(wrap_pyfunction!(update_task, m)?)?;

    m.add_function(wrap_pyfunction!(validate, m)?)?;
    m.add_function(wrap_pyfunction!(fix_summary, m)?)?;
    m.add_function(wrap_pyfunction!(do_fix, m)?)?;
    m.add_function(wrap_pyfunction!(do_init, m)?)?;
    m.add_function(wrap_pyfunction!(do_ensure_initialized, m)?)?;
    m.add_function(wrap_pyfunction!(do_create, m)?)?;
    m.add_function(wrap_pyfunction!(discover_tasks_dir, m)?)?;
    m.add_function(wrap_pyfunction!(discover_tasks_dir_or_default, m)?)?;

    m.add("FILENAME_PATTERN", filename::FILENAME_PATTERN.as_str())?;
    m.add("TEMPLATE_FILENAME", constants::TEMPLATE_FILENAME)?;
    m.add("DEFAULT_TASKS_DIR_NAME", constants::DEFAULT_TASKS_DIR_NAME)?;
    m.add("VALID_STATUSES", constants::VALID_STATUSES.to_vec())?;
    m.add("VALID_PRIORITIES", constants::VALID_PRIORITIES.to_vec())?;

    Ok(())
}
