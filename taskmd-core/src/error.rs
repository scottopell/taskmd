use std::fmt;
use std::path::PathBuf;

use crate::constants::{Priority, Status};

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),

    /// Priority string did not match any known variant.
    InvalidPriority { got: String },

    /// Status string did not match any known variant.
    InvalidStatus { got: String },

    /// Slug failed validation. `reason` is a short, human-readable description
    /// (e.g. "must contain at least one alphanumeric character").
    InvalidSlug { got: String, reason: String },

    /// Task body was empty / whitespace-only.
    EmptyBody,

    /// Tasks directory does not exist on disk.
    TasksDirNotFound { path: PathBuf },

    /// Lookup by id found nothing.
    TaskNotFound { id: String },

    /// Target filename already exists when trying to rename to it.
    TargetExists { path: PathBuf },

    /// `create_task` exhausted retry budget allocating a unique id.
    IdAllocationExhausted { tasks_dir: PathBuf, tries: u32 },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::InvalidPriority { got } => {
                let allowed: Vec<&'static str> =
                    Priority::ALL.iter().map(|p| p.as_str()).collect();
                write!(
                    f,
                    "invalid priority '{got}', expected one of: {}",
                    allowed.join(", ")
                )
            }
            Error::InvalidStatus { got } => {
                let allowed: Vec<&'static str> =
                    Status::ALL.iter().map(|s| s.as_str()).collect();
                write!(
                    f,
                    "invalid status '{got}', expected one of: {}",
                    allowed.join(", ")
                )
            }
            Error::InvalidSlug { got, reason } => {
                write!(f, "invalid slug {got:?}: {reason}")
            }
            Error::EmptyBody => write!(
                f,
                "body is required — pipe at least one line of description on stdin. \
                 A task with no body is a placeholder; if you cannot describe it, \
                 do not create it yet."
            ),
            Error::TasksDirNotFound { path } => write!(
                f,
                "tasks directory does not exist: {} (run 'taskmd init' first)",
                path.display()
            ),
            Error::TaskNotFound { id } => write!(f, "task {id} not found"),
            Error::TargetExists { path } => {
                write!(f, "target already exists: {}", path.display())
            }
            Error::IdAllocationExhausted { tasks_dir, tries } => write!(
                f,
                "failed to allocate a unique task ID in {} after {tries} attempts",
                tasks_dir.display()
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
