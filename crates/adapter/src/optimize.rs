//! Backend-produced preview and outcome for system maintenance.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizePreview {
    /// Exact backend version that produced this review.
    pub backend_version: String,
    /// Count from Mole's own dry-run summary, when present.
    pub action_count: u64,
    /// Grouped using headings and rows from Mole's output; no cleanup targets
    /// or recommendations are added by Nirmoka.
    pub sections: Vec<OptimizeSection>,
    /// The backend's own dry-run output with terminal styling removed.
    pub transcript: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizeSection {
    pub name: String,
    pub items: Vec<OptimizeItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizeItem {
    pub kind: OptimizeItemKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OptimizeItemKind {
    Action,
    Complete,
    Review,
    Note,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OptimizeCompletion {
    Finished,
    Cancelled,
    Failed,
}

/// Outcome of one confirmed system-maintenance run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizeExecution {
    pub completion: OptimizeCompletion,
    /// Output as reported by the backend, including any partial-run detail.
    pub transcript: String,
}
