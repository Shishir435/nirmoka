//! Backend-produced application uninstall plan and outcome.
//!
//! An uninstall plan is *evidence read back from the backend*, not a list of
//! arguments. Nothing in this module is ever handed to a subprocess: the backend
//! rediscovers what to remove when it runs, and it applies its own protected-path
//! rules while doing so. See [ADR 0027](../../../docs/adr/0027-uninstall-is-a-relayed-confirmation.md).

use serde::{Deserialize, Serialize};

/// One backend-produced preview of removing one or more applications.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UninstallPreview {
    /// Exact backend version that produced this review. Execution must reject a
    /// different version, even when both are otherwise supported.
    pub backend_version: String,
    /// The identifiers this preview was asked about, verbatim — the backend's
    /// own `uninstall_name` values, never display names.
    pub requested: Vec<String>,
    pub apps: Vec<UninstallApp>,
    /// The backend's rounded total, e.g. `"83.4MB"`. Text because that is what
    /// was published; see [`UninstallItem::reported_size`].
    pub reported_total: Option<String>,
    pub warnings: Vec<String>,
    /// The backend's own review notes — things it says it will *not* do, such as
    /// resetting Local Network permissions. Surfaced verbatim because a note
    /// about what survives an uninstall is exactly what a user needs to read.
    pub notes: Vec<String>,
    /// The backend's output for this preview, ANSI-stripped and otherwise
    /// untouched.
    ///
    /// Kept beside the parsed form on purpose. The parse is for rendering; this
    /// is the thing the user is actually approving, and it is what makes a
    /// parser bug visible instead of silently narrowing a delete plan.
    pub transcript: String,
}

impl UninstallPreview {
    /// Total paths across every app. Zero means the backend found nothing, which
    /// must not be preparable for execution.
    pub fn total_items(&self) -> usize {
        self.apps.iter().map(|app| app.items.len()).sum()
    }

    /// Any path the backend classified as needing review rather than removal.
    pub fn has_review_only_items(&self) -> bool {
        self.apps.iter().flat_map(|app| &app.items).any(|item| {
            matches!(
                item.scope,
                UninstallItemScope::System | UninstallItemScope::ReviewOnly
            )
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallApp {
    pub name: String,
    /// The backend tagged this one as a Homebrew cask, which changes what it
    /// does: a cask is removed through `brew` with configs and data included.
    pub homebrew_cask: bool,
    pub reported_size: Option<String>,
    pub items: Vec<UninstallItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallItem {
    /// The path as the backend displayed it, which is tilde-abbreviated.
    ///
    /// A `String` rather than a `PathBuf` deliberately. `~/Library/Caches` is not
    /// a path any API can resolve, and wrapping it in a `PathBuf` would produce
    /// a type that looks resolvable, invites being passed to a file operation,
    /// and resolves to a directory named `~` in the working directory if it ever
    /// were. This is display text, so it is typed as display text.
    pub display_path: String,
    /// The backend's own rounded label, e.g. `"225KB"`, absent when it did not
    /// report one.
    ///
    /// Text for the same reason as `InstalledApplication::reported_size`: parsing
    /// it back into bytes would invent precision and then let the UI add the
    /// inventions together.
    pub reported_size: Option<String>,
    pub scope: UninstallItemScope,
}

/// What the backend said it would do with one path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UninstallItemScope {
    /// The backend will remove it.
    Removed,
    /// A system location the backend will remove, flagged because it is outside
    /// the user's own library.
    System,
    /// The backend will **not** touch it and is reporting it for the user to
    /// deal with. Rendering this the same as `Removed` would claim a removal
    /// that never happens.
    ReviewOnly,
}

/// Backend-reported outcome of one confirmed uninstall run.
///
/// Every started run produces one of these, including a cancelled one. Same rule
/// as [`crate::CleanupExecution`]: once the subprocess is alive it may already
/// have moved files, so an interruption is an outcome to record rather than an
/// error to propagate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallExecution {
    pub completion: UninstallCompletion,
    /// Application names the backend reported as removed.
    pub removed: Vec<String>,
    /// Application names the backend reported it could not remove, with its own
    /// stated reason.
    pub failed: Vec<String>,
    pub reported_freed: Option<String>,
    pub warnings: Vec<String>,
    /// The run's output, ANSI-stripped. The record of what actually happened.
    pub transcript: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UninstallCompletion {
    /// The backend exited successfully and reported no failures.
    Finished,
    /// The backend removed some applications and reported others as failed.
    Partial,
    /// The subprocess was killed on request. Whatever it had already moved stays
    /// moved, which is why this is not `AdapterError::Cancelled`.
    Cancelled,
    /// The backend died part way through, or refused after the run began — an
    /// administrator prompt the user dismissed, most often. The run happened and
    /// how far it got is not knowable from outside.
    Failed,
}
