//! PyO3 Python extension module for taskmd.
//!
//! Compiled by maturin as `taskmd._core`. All logic lives in taskmd-core;
//! this crate is a thin binding layer.

use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::path::Path;
use taskmd_core::{constants, filename, fix, ids, init, tasks, validate as vld};

// ── Task dict helper ──────────────────────────────────────────────────────────

fn task_to_dict<'py>(
    py: Python<'py>,
    task: tasks::TaskFile,
) -> PyResult<pyo3::Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("path", task.path.to_string_lossy().as_ref())?;
    dict.set_item("id", &task.id)?;
    dict.set_item("priority", &task.priority)?;
    dict.set_item("status", &task.status)?;
    dict.set_item("slug", &task.slug)?;

    let fields_dict = PyDict::new(py);
    for (k, v) in &task.fields {
        fields_dict.set_item(k, v)?;
    }
    dict.set_item("fields", fields_dict)?;
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
fn get_expected_filename(id: &str, priority: &str, status: &str, slug: &str) -> String {
    filename::format_filename(id, priority, status, slug)
}

#[pyfunction]
fn derive_slug(title: &str) -> String {
    filename::derive_slug(title)
}

// ── Frontmatter ───────────────────────────────────────────────────────────────

#[pyfunction]
fn parse_frontmatter(content: &str) -> std::collections::HashMap<String, String> {
    taskmd_core::frontmatter::parse_frontmatter_str(content)
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
fn rename_status(tasks_dir: &str, id: &str, new_status: &str) -> PyResult<(String, String)> {
    tasks::rename_status(Path::new(tasks_dir), id, new_status)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
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

/// Canonical summary string for a fix result — single Rust implementation.
/// Python's `FixResult.summary()` calls this rather than reimplementing.
#[pyfunction]
fn fix_summary(patched: usize, renamed: usize, migrated: usize) -> String {
    fix::fix_summary(patched, renamed, migrated)
}

#[pyfunction]
fn do_fix(py: Python<'_>, tasks_dir: &str) -> PyResult<Py<PyAny>> {
    let r = fix::fix(Path::new(tasks_dir));
    let dict = PyDict::new(py);
    dict.set_item("patched", r.patched)?;
    dict.set_item("renamed", r.renamed)?;
    dict.set_item("migrated", r.migrated)?;
    dict.set_item("patches", r.patches)?;
    dict.set_item("renames", r.renames)?;
    dict.set_item("errors", r.errors)?;
    Ok(dict.into_any().unbind())
}

// ── Init ──────────────────────────────────────────────────────────────────────

#[pyfunction]
fn do_init(py: Python<'_>, tasks_dir: &str) -> PyResult<Py<PyAny>> {
    let r = init::init(Path::new(tasks_dir));
    let dict = PyDict::new(py);
    dict.set_item("tasks_dir", r.tasks_dir.to_string_lossy().as_ref())?;
    dict.set_item("created", r.created)?;
    dict.set_item("template_fields", r.template_fields)?;
    dict.set_item("error", r.error)?;
    Ok(dict.into_any().unbind())
}

// ── Module ────────────────────────────────────────────────────────────────────

#[pymodule]
fn _core(m: &pyo3::Bound<'_, PyModule>) -> PyResult<()> {
    // Internal helpers (test suite)
    m.add_function(wrap_pyfunction!(task_files, m)?)?;
    m.add_function(wrap_pyfunction!(is_legacy_id, m)?)?;
    m.add_function(wrap_pyfunction!(needs_migration, m)?)?;
    m.add_function(wrap_pyfunction!(parse_id_parts, m)?)?;
    m.add_function(wrap_pyfunction!(prefix_for, m)?)?;

    // ID / filename / slug
    m.add_function(wrap_pyfunction!(next_id, m)?)?;
    m.add_function(wrap_pyfunction!(get_expected_filename, m)?)?;
    m.add_function(wrap_pyfunction!(derive_slug, m)?)?;

    // Frontmatter
    m.add_function(wrap_pyfunction!(parse_frontmatter, m)?)?;

    // Task file operations
    m.add_function(wrap_pyfunction!(parse_task_file, m)?)?;
    m.add_function(wrap_pyfunction!(list_tasks, m)?)?;
    m.add_function(wrap_pyfunction!(find_task_by_id, m)?)?;
    m.add_function(wrap_pyfunction!(rename_status, m)?)?;

    // Higher-level operations
    m.add_function(wrap_pyfunction!(validate, m)?)?;
    m.add_function(wrap_pyfunction!(fix_summary, m)?)?;
    m.add_function(wrap_pyfunction!(do_fix, m)?)?;
    m.add_function(wrap_pyfunction!(do_init, m)?)?;

    // Constants — sourced from taskmd_core::constants (single definition)
    m.add("FILENAME_PATTERN", filename::FILENAME_PATTERN.as_str())?;
    m.add("VALID_STATUSES", constants::VALID_STATUSES.to_vec())?;
    m.add("VALID_PRIORITIES", constants::VALID_PRIORITIES.to_vec())?;
    m.add("VALID_FIELDS", constants::VALID_FIELDS.to_vec())?;

    Ok(())
}
