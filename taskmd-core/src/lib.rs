//! taskmd-core: pure Rust implementation of taskmd logic.
//!
//! This crate is the single source of truth for:
//!   - Task ID generation (numeric DDNNN format with hostname+directory prefix)
//!   - Filename parsing, formatting, and pattern constant
//!   - Slug derivation
//!   - Frontmatter parsing
//!   - Task file listing, searching, and renaming
//!   - Corpus validation and auto-fix
//!   - Tasks directory initialisation
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
pub mod date;
pub mod error;
pub mod filename;
pub mod fix;
pub mod frontmatter;
pub mod ids;
pub mod init;
pub mod tasks;
pub mod util;
pub mod validate;
