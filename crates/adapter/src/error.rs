//! Adapter failures.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AdapterError {
    /// The backend binary could not be executed at all.
    #[error("could not run {binary}: {source}")]
    Spawn {
        binary: &'static str,
        #[source]
        source: std::io::Error,
    },

    /// The backend ran but its version output was unrecognisable. Treated as a
    /// hard failure rather than assumed-compatible.
    #[error("could not read a version from {binary} output: {output:?}")]
    UnreadableVersion {
        binary: &'static str,
        output: String,
    },

    /// The backend exited non-zero.
    #[error("{binary} exited with status {status}: {stderr}")]
    BackendFailed {
        binary: &'static str,
        status: i32,
        stderr: String,
    },

    /// A path failed validation at the adapter boundary. This is the last line
    /// before a path becomes a subprocess argument.
    #[error("refused path {path}: {reason}")]
    RefusedPath { path: String, reason: String },

    /// The caller asked for something this backend cannot do. Should be
    /// unreachable if the UI respects `Capabilities`.
    #[error("{backend} does not support {operation}")]
    Unsupported {
        backend: &'static str,
        operation: &'static str,
    },
}
