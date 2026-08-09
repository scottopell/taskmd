//! taskmd-core: pure Rust implementation of taskmd logic.
//!
//! This crate is the single source of truth for:
//!   - Task ID generation (numeric DDNNN format; DD = hash of machine identity + tasks directory path, mod 100)
//!   - Filename parsing, formatting, and pattern constant
//!   - Slug derivation
//!   - Task file listing, searching, and renaming
//!   - Corpus validation and auto-fix
//!   - Tasks directory discovery (`_TEMPLATE.md` marker scan) and initialisation
//!
//! The filename is the sole source of truth for task metadata (id, priority,
//! status, slug). Bodies are free-form markdown.
//!
//! # Usage (Rust)
//!
//! ```toml
//! [dependencies]
//! taskmd-core = { git = "…", path = "taskmd-core" }
//! ```
//!
//! The Python extension (`taskmd._core`) lives in `taskmd-py/` and re-exports
//! everything here via PyO3.

pub mod constants;
pub mod create;
pub mod discover;
pub mod error;
pub mod filename;
pub mod fix;
mod git_history;
pub mod ids;
pub mod init;
pub mod tasks;
pub mod util;
pub mod validate;
