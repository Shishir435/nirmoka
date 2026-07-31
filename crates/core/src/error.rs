//! Core error type. Backend-specific failures live in `nirmoka-adapter`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    /// A path failed validation before any operation was attempted.
    #[error("invalid path: {0}")]
    InvalidPath(String),

    /// Backend output could not be understood.
    #[error("could not parse backend output: {0}")]
    Parse(String),

    /// A tree operation referenced a node that does not exist.
    #[error("unknown node id: {0}")]
    UnknownNode(u32),
}

pub type Result<T> = std::result::Result<T, CoreError>;
