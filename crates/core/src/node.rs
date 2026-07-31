//! A single entry in a scanned tree.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Directory,
    File,
    Symlink,
    Other,
}

/// One filesystem entry.
///
/// Nodes store only their own name, never a full path. Paths are reconstructed
/// by walking up the tree ([`crate::Tree::path_of`]), which keeps memory flat on
/// scans that reach millions of entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// File or directory name, not a path.
    pub name: String,

    pub kind: NodeKind,

    /// Bytes attributable to this entry alone, excluding children.
    pub own_bytes: u64,

    /// Bytes for this entry plus its whole subtree. Computed by
    /// [`crate::Tree::rollup`]; zero until then.
    pub total_bytes: u64,

    /// The backend could not fully read this entry, so its size is a lower
    /// bound. Surfaced in the UI rather than silently swallowed — an
    /// unreadable directory that looks empty is a lie.
    pub read_error: bool,
}

impl Node {
    pub fn directory(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: NodeKind::Directory,
            own_bytes: 0,
            total_bytes: 0,
            read_error: false,
        }
    }

    pub fn file(name: impl Into<String>, bytes: u64) -> Self {
        Self {
            name: name.into(),
            kind: NodeKind::File,
            own_bytes: bytes,
            total_bytes: bytes,
            read_error: false,
        }
    }

    pub fn is_dir(&self) -> bool {
        matches!(self.kind, NodeKind::Directory)
    }
}
