//! Backend-produced cleanup preview.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// What a cleanup preview reports while it is still running.
///
/// A dry run walks the disk and takes minutes, and the backend narrates that
/// walk. These are the parts of the narration worth showing: enough for a
/// window to say what is happening now, in the backend's own words, without
/// inventing a percentage nobody can compute.
///
/// Deliberately borrowed rather than owned. The adapter reads a line, hands it
/// over, and moves on; nothing here is retained, and a caller that wants to
/// keep a value copies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupProgress<'a> {
    /// A new heading: the group of things the backend has started on.
    Category(&'a str),
    /// One entry within the current category, as the backend described it.
    Item(&'a str),
    /// What the category just finished came to. The backend's own text — see
    /// ADR 0030, and `CleanupItem::reported_size` for why it stays text.
    CategoryTotal(&'a str),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupPreview {
    /// Exact backend version that produced this review. Execution must reject
    /// a different version, even when both versions are otherwise supported.
    pub backend_version: String,
    pub generated_at: String,
    pub categories: Vec<CleanupCategory>,
    pub potential_cleanup: Option<String>,
    pub total_items: u64,
    pub system_scope: CleanupSystemScope,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupCategory {
    pub name: String,
    pub items: Vec<CleanupItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupItem {
    pub path: PathBuf,
    /// Human-readable size published by the backend. Mole rounds this value,
    /// so it must not be presented as an exact byte count.
    pub reported_size: Option<String>,
    pub item_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CleanupSystemScope {
    Included,
    UserOnly,
    Unknown,
}

/// Backend-reported outcome of one confirmed cleanup run.
///
/// Every started run produces one of these, including a run that was cancelled
/// or that the backend failed part way through. Once the cleanup subprocess is
/// alive it may have removed files, so an interruption is an outcome to record
/// rather than an error to propagate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupExecution {
    pub system_scope: CleanupSystemScope,
    pub completion: CleanupCompletion,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CleanupCompletion {
    /// Backend exited successfully and published no known partial warning.
    /// Exact per-path results still come from the operation journal.
    Finished,
    Partial,
    /// The subprocess was killed on request. Whatever it had already removed
    /// stays removed, which is why this is not `AdapterError::Cancelled`.
    Cancelled,
    /// The backend died part way through. Same reasoning as `Cancelled`: the
    /// run happened, and how far it got is not knowable from outside.
    Failed,
}
