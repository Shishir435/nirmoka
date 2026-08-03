//! Adapter failures.

use thiserror::Error;

use crate::wire::WireError;

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

    /// An adapter could run its backend but could not safely complete the
    /// surrounding operation (for example, creating its recovery directory or
    /// finding the receipt the backend promised).
    #[error("{operation} on {backend} failed: {reason}")]
    OperationFailed {
        backend: &'static str,
        operation: &'static str,
        reason: String,
    },

    /// The backend ran and exited cleanly, but what it printed was not the wire
    /// format. Distinct from `BackendFailed` because the fix is different: a
    /// format drift needs a new version gate, not a working install.
    #[error("{binary} produced output this build cannot read: {source}")]
    MalformedOutput {
        binary: &'static str,
        #[source]
        source: WireError,
    },

    /// A capability-specific machine-readable response changed shape.
    #[error("{binary} produced unreadable {operation} output: {reason}")]
    MalformedBackendOutput {
        binary: &'static str,
        operation: &'static str,
        reason: String,
    },

    /// The backend is not on this machine. Not an internal failure — the UI
    /// says "install ncdu", not "something went wrong".
    #[error("{binary} is not installed")]
    NotInstalled { binary: &'static str },

    /// The backend is installed at a version this adapter has never been tested
    /// against. Every operation refuses rather than hoping the format held.
    #[error("{binary} {version} is not supported; this build understands {supported}")]
    UnsupportedVersion {
        binary: &'static str,
        version: String,
        supported: &'static str,
    },

    /// The backend changed after a destructive operation was reviewed. Even a
    /// separately supported version needs a new preview because its discovery
    /// and safety behavior may differ.
    #[error(
        "{binary} changed from reviewed version {reviewed} to {current}; generate a new preview"
    )]
    BackendVersionChanged {
        binary: &'static str,
        reviewed: String,
        current: String,
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

    /// The caller stopped the operation. Not a failure — but it must be
    /// distinguishable from one, because a cancelled scan produces a truncated
    /// export that would otherwise look like a corrupt backend.
    #[error("{operation} on {backend} was cancelled")]
    Cancelled {
        backend: &'static str,
        operation: &'static str,
    },
}

impl AdapterError {
    /// True when the user asked for this. Callers use it to stay quiet instead
    /// of showing an error dialog for a button the user pressed on purpose.
    pub fn is_cancellation(&self) -> bool {
        matches!(self, AdapterError::Cancelled { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn spawn_failures_name_the_binary_and_keep_the_cause() {
        let error = AdapterError::Spawn {
            binary: "ncdu",
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        };
        assert_eq!(error.to_string(), "could not run ncdu: denied");
        assert!(error.source().is_some());
    }

    #[test]
    fn unreadable_version_quotes_what_the_backend_actually_printed() {
        // Quoted on purpose: the interesting case is output that looks empty
        // or is pure whitespace, which an unquoted message would hide.
        let error = AdapterError::UnreadableVersion {
            binary: "ncdu",
            output: "  ".to_string(),
        };
        assert_eq!(
            error.to_string(),
            r#"could not read a version from ncdu output: "  ""#
        );
    }

    #[test]
    fn backend_failures_carry_the_status_and_stderr() {
        let error = AdapterError::BackendFailed {
            binary: "ncdu",
            status: 2,
            stderr: "no such directory".to_string(),
        };
        assert_eq!(
            error.to_string(),
            "ncdu exited with status 2: no such directory"
        );
    }

    #[test]
    fn operation_failures_explain_the_safety_check_that_failed() {
        let error = AdapterError::OperationFailed {
            backend: "rip",
            operation: "delete",
            reason: "recovery receipt was not created".to_string(),
        };
        assert_eq!(
            error.to_string(),
            "delete on rip failed: recovery receipt was not created"
        );
    }

    #[test]
    fn malformed_output_keeps_the_parse_error_as_its_cause() {
        let error = AdapterError::MalformedOutput {
            binary: "ncdu",
            source: WireError::UnsupportedFormat {
                major: 2,
                minor: 0,
                supported: 1,
            },
        };
        assert!(error.to_string().starts_with("ncdu produced output"));
        assert!(error.source().unwrap().to_string().contains("2.0"));
    }

    #[test]
    fn a_missing_backend_reads_as_missing_not_as_broken() {
        let error = AdapterError::NotInstalled { binary: "ncdu" };
        assert_eq!(error.to_string(), "ncdu is not installed");
        assert!(error.source().is_none());
    }

    #[test]
    fn an_untested_version_says_what_it_found_and_what_it_wants() {
        // Both halves matter: "1.19" tells the user what they have, ">=2.0"
        // tells them what to install.
        let error = AdapterError::UnsupportedVersion {
            binary: "ncdu",
            version: "1.19".to_string(),
            supported: ">=2.0, <3.0",
        };
        assert_eq!(
            error.to_string(),
            "ncdu 1.19 is not supported; this build understands >=2.0, <3.0"
        );
    }

    #[test]
    fn a_backend_change_requires_a_new_review() {
        let error = AdapterError::BackendVersionChanged {
            binary: "mo",
            reviewed: "1.48.1".to_string(),
            current: "1.49.0".to_string(),
        };
        assert_eq!(
            error.to_string(),
            "mo changed from reviewed version 1.48.1 to 1.49.0; generate a new preview"
        );
    }

    #[test]
    fn refused_paths_say_which_path_and_why() {
        let error = AdapterError::RefusedPath {
            path: "/etc".to_string(),
            reason: "outside the scan root".to_string(),
        };
        assert_eq!(
            error.to_string(),
            "refused path /etc: outside the scan root"
        );
    }

    #[test]
    fn unsupported_names_the_operation_the_ui_should_have_hidden() {
        let error = AdapterError::Unsupported {
            backend: "ncdu",
            operation: "dry run",
        };
        assert_eq!(error.to_string(), "ncdu does not support dry run");
        assert!(!error.is_cancellation());
    }

    #[test]
    fn cancellation_is_reported_as_itself_not_as_a_failure() {
        let error = AdapterError::Cancelled {
            backend: "ncdu",
            operation: "scan",
        };
        assert_eq!(error.to_string(), "scan on ncdu was cancelled");
        assert!(error.is_cancellation());
    }
}
