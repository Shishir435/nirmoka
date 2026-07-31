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
///
/// # Two sizes, deliberately
///
/// `own_bytes` is disk usage and `apparent_bytes` is the length the file claims.
/// They diverge on sparse files (2 GB apparent, near-zero on disk) and on
/// filesystems that pack small files inline. A cleanup tool that shows only
/// apparent size promises space it cannot reclaim, so disk usage is what
/// everything sorts and rolls up by, and apparent size is kept alongside it so
/// the difference can be explained rather than hidden.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// File or directory name, not a path.
    pub name: String,

    pub kind: NodeKind,

    /// Disk usage attributable to this entry alone, excluding children.
    pub own_bytes: u64,

    /// Apparent size of this entry alone. Larger than `own_bytes` for sparse
    /// files, smaller for entries whose metadata costs more than their content.
    pub apparent_bytes: u64,

    /// Disk usage for this entry plus its whole subtree. Computed by
    /// [`crate::Tree::rollup`]; zero until then.
    pub total_bytes: u64,

    /// The backend could not fully read this entry, so its size is a lower
    /// bound. Surfaced in the UI rather than silently swallowed — an
    /// unreadable directory that looks empty is a lie.
    pub read_error: bool,

    /// Another entry in this scan shares the same inode, and that one was
    /// counted. This node's `own_bytes` is zero to keep the total honest, so
    /// the UI must be able to say "counted once" rather than "empty".
    pub hardlink: bool,

    /// The backend was told to skip this entry, so its size is unknown rather
    /// than zero. Same reasoning as `read_error`: a skipped directory that
    /// renders as empty is a lie.
    pub excluded: bool,
}

impl Node {
    pub fn directory(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: NodeKind::Directory,
            own_bytes: 0,
            apparent_bytes: 0,
            total_bytes: 0,
            read_error: false,
            hardlink: false,
            excluded: false,
        }
    }

    pub fn file(name: impl Into<String>, bytes: u64) -> Self {
        Self {
            name: name.into(),
            kind: NodeKind::File,
            own_bytes: bytes,
            apparent_bytes: bytes,
            total_bytes: bytes,
            read_error: false,
            hardlink: false,
            excluded: false,
        }
    }

    pub fn is_dir(&self) -> bool {
        matches!(self.kind, NodeKind::Directory)
    }

    /// True when this node's size is known to be incomplete, for any reason.
    /// The UI uses one marker for all of them; the specific flag explains why.
    pub fn size_is_partial(&self) -> bool {
        self.read_error || self.excluded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_file_has_equal_disk_and_apparent_size() {
        let node = Node::file("f", 4096);
        assert_eq!(node.own_bytes, node.apparent_bytes);
        assert!(!node.size_is_partial());
    }

    #[test]
    fn read_errors_and_exclusions_both_mark_the_size_partial() {
        let mut node = Node::directory("d");
        assert!(!node.size_is_partial());

        node.read_error = true;
        assert!(node.size_is_partial());

        node.read_error = false;
        node.excluded = true;
        assert!(node.size_is_partial());
    }

    #[test]
    fn a_hardlink_is_not_a_partial_size() {
        // A deduplicated hardlink has a *correct* zero, unlike an unreadable
        // directory. Conflating the two would put a warning marker on entries
        // whose numbers are fine.
        let mut node = Node::file("f", 0);
        node.hardlink = true;
        assert!(!node.size_is_partial());
    }
}
