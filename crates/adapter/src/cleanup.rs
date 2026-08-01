//! Backend-produced cleanup preview.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupPreview {
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
