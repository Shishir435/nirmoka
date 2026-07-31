//! The types that cross into TypeScript.
//!
//! These are deliberately separate from the domain types in `nirmoka-core` and
//! `nirmoka-adapter`, even where they look identical today.
//!
//! Two reasons. First, invariant 1: `core` may depend on the standard library,
//! serde, and thiserror, so it cannot carry a `#[derive(TS)]`. Second, the shape
//! the UI needs and the shape the domain uses drift apart — a `Row` is a `Node`
//! plus its position in a window and its share of the parent, which is a fact
//! about a viewport, not about a file. Keeping the boundary explicit means that
//! drift shows up as a conversion, not as a domain type quietly growing a field
//! that only the frontend wanted.
//!
//! The TypeScript mirrors are generated from this file by `cargo test -p
//! nirmoka-app --test export_bindings` and committed to
//! `packages/transport/src/generated/`. CI regenerates and fails on a diff.

use nirmoka_adapter::registry::RegistryEntry;
use nirmoka_adapter::wire::TreeStats;
use nirmoka_adapter::{Capabilities as AdapterCapabilities, Detection as AdapterDetection};
use nirmoka_core::{Node, NodeKind as CoreNodeKind, Tree};
use serde::Serialize;
use ts_rs::TS;

// Byte counts carry `#[ts(type = "number")]` throughout.
//
// ts-rs maps `u64` to `bigint`, which is right for a general Rust type and wrong
// here: Tauri's IPC is JSON, so these arrive as ordinary JavaScript numbers and
// a `bigint` annotation would describe a value that never appears. The precision
// ceiling is 2^53 bytes — 8 petabytes for a single entry — which is past the
// size of any disk this will run on.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub enum NodeKind {
    Directory,
    File,
    Symlink,
    Other,
}

impl From<CoreNodeKind> for NodeKind {
    fn from(kind: CoreNodeKind) -> Self {
        match kind {
            CoreNodeKind::Directory => Self::Directory,
            CoreNodeKind::File => Self::File,
            CoreNodeKind::Symlink => Self::Symlink,
            CoreNodeKind::Other => Self::Other,
        }
    }
}

/// Whether a backend is installed, and whether this build understands it.
///
/// `unsupportedVersion` stays a distinct state all the way to the UI. Collapsing
/// it into "not installed" would tell a user to install what they already have.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub enum Detection {
    #[serde(rename_all = "camelCase")]
    Found {
        path: String,
        version: String,
    },
    #[serde(rename_all = "camelCase")]
    UnsupportedVersion {
        path: String,
        version: String,
        supported: String,
    },
    NotInstalled,
}

impl From<&AdapterDetection> for Detection {
    fn from(detection: &AdapterDetection) -> Self {
        match detection {
            AdapterDetection::Found { path, version } => Self::Found {
                path: path.display().to_string(),
                version: version.clone(),
            },
            AdapterDetection::UnsupportedVersion {
                path,
                version,
                supported,
            } => Self::UnsupportedVersion {
                path: path.display().to_string(),
                version: version.clone(),
                supported: supported.clone(),
            },
            AdapterDetection::NotInstalled => Self::NotInstalled,
        }
    }
}

/// One backend as the picker sees it.
///
/// `detection` and `error` are both optional because detection itself can fail —
/// a backend that exists but whose version output could not be read is neither
/// "found" nor "not installed", and saying so is more useful than picking one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct Backend {
    pub id: String,
    pub display_name: String,
    pub supported_versions: String,
    pub detection: Option<Detection>,
    pub error: Option<String>,
    /// Usable right now: installed, and at a version this build was tested
    /// against. The UI enables the scan button on this and nothing else.
    pub usable: bool,
}

impl From<&RegistryEntry> for Backend {
    fn from(entry: &RegistryEntry) -> Self {
        let (detection, error) = match &entry.detection {
            Ok(detection) => (Some(Detection::from(detection)), None),
            Err(error) => (None, Some(error.to_string())),
        };

        Self {
            id: entry.id.to_string(),
            display_name: entry.display_name.to_string(),
            supported_versions: entry.supported_versions.to_string(),
            usable: matches!(&detection, Some(Detection::Found { .. })),
            detection,
            error,
        }
    }
}

/// What the active backend can do, so the UI can hide controls it cannot honour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct Capabilities {
    pub scan: bool,
    pub delete: bool,
    pub trash: bool,
    pub dry_run: bool,
    pub cleanup_categories: bool,
    pub uninstall_apps: bool,
    pub system_status: bool,
}

impl From<AdapterCapabilities> for Capabilities {
    fn from(caps: AdapterCapabilities) -> Self {
        Self {
            scan: caps.scan,
            delete: caps.delete,
            trash: caps.trash,
            dry_run: caps.dry_run,
            cleanup_categories: caps.cleanup_categories,
            uninstall_apps: caps.uninstall_apps,
            system_status: caps.system_status,
        }
    }
}

/// One rendered line. The frontend never receives anything else about the tree.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct Row {
    /// Opaque handle into the Rust-side tree. Pass it back to descend.
    pub id: u32,
    pub name: String,
    pub kind: NodeKind,
    /// Disk usage for this entry alone.
    #[ts(type = "number")]
    pub own_bytes: u64,
    /// What the entry claims to be. Differs from `ownBytes` for sparse files.
    #[ts(type = "number")]
    pub apparent_bytes: u64,
    /// Disk usage for this entry plus everything under it.
    #[ts(type = "number")]
    pub total_bytes: u64,
    /// The size is a lower bound: the backend could not read this entry.
    pub read_error: bool,
    /// Counted once already, under another name. Not empty — shared.
    pub hardlink: bool,
    /// Skipped by request, so its size is unknown rather than zero.
    pub excluded: bool,
    /// Directories with children can be descended into.
    pub child_count: u32,
    /// Fraction of the parent's total, 0..1, for bar rendering.
    pub share: f64,
}

impl Row {
    fn from_node(id: u32, node: &Node, child_count: u32, parent_total: u64) -> Self {
        Self {
            id,
            name: node.name.clone(),
            kind: node.kind.into(),
            own_bytes: node.own_bytes,
            apparent_bytes: node.apparent_bytes,
            total_bytes: node.total_bytes,
            read_error: node.read_error,
            hardlink: node.hardlink,
            excluded: node.excluded,
            child_count,
            // A zero-byte parent makes every share meaningless rather than
            // infinite; report nothing rather than dividing by zero.
            share: if parent_total == 0 {
                0.0
            } else {
                node.total_bytes as f64 / parent_total as f64
            },
        }
    }
}

/// One window of one directory's children.
///
/// `total` is how many children exist, not how many are in `rows` — the caller
/// needs it to size a scrollbar without asking for the whole directory, which is
/// invariant 5 in one field.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct RowPage {
    pub parent_id: u32,
    /// Absolute path of the parent, for the breadcrumb.
    pub path: String,
    pub offset: u32,
    pub total: u32,
    pub rows: Vec<Row>,
}

/// Progress while a scan is running.
///
/// Emitted periodically rather than per entry: a home directory produces
/// millions of entries and an event per entry would spend more time in IPC than
/// in the scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct ScanProgress {
    #[ts(type = "number")]
    pub scanned: u64,
    /// Where the backend is now, for something truthful to put on screen.
    pub current_path: String,
}

/// A finished scan. Everything here is a fact about a completed walk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct ScanSummary {
    pub root_id: u32,
    pub root_path: String,
    #[ts(type = "number")]
    pub total_bytes: u64,
    #[ts(type = "number")]
    pub entries: u64,
    #[ts(type = "number")]
    pub directories: u64,
    pub backend_id: String,
    pub backend_version: Option<String>,
    /// Counts of what the totals could not account for. A total that quietly
    /// omits twelve unreadable directories is a lie by omission.
    #[ts(type = "number")]
    pub read_errors: u64,
    #[ts(type = "number")]
    pub excluded: u64,
    #[ts(type = "number")]
    pub hardlinks_deduplicated: u64,
    #[ts(type = "number")]
    pub hardlink_bytes_saved: u64,
}

impl ScanSummary {
    pub(crate) fn new(
        tree: &Tree,
        summary: &nirmoka_adapter::ScanSummary,
        stats: TreeStats,
        backend_id: &str,
    ) -> Self {
        let root = tree.root();
        let total_bytes = root
            .and_then(|id| tree.get(id).ok())
            .map(|node| node.total_bytes)
            .unwrap_or(0);

        Self {
            root_id: root.map(|id| id.raw()).unwrap_or(0),
            root_path: summary.root.display().to_string(),
            total_bytes,
            entries: summary.items,
            directories: summary.directories,
            backend_id: backend_id.to_string(),
            backend_version: summary.backend_version.clone(),
            read_errors: stats.read_errors,
            excluded: stats.excluded,
            hardlinks_deduplicated: stats.hardlinks_deduplicated,
            hardlink_bytes_saved: stats.hardlink_bytes_saved,
        }
    }
}

/// A scan that ended without a result.
///
/// `cancelled` is separate from the message because a cancelled scan is not an
/// error the user needs to read — they are the one who stopped it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct ScanFailure {
    pub message: String,
    pub cancelled: bool,
}

/// Build a page of rows from a parent's children, largest first.
pub(crate) fn page(tree: &Tree, parent: nirmoka_core::NodeId, offset: u32, limit: u32) -> RowPage {
    let parent_total = tree.get(parent).map(|node| node.total_bytes).unwrap_or(0);
    let children = tree.children_by_size(parent);

    let rows = children
        .iter()
        .skip(offset as usize)
        .take(limit as usize)
        .filter_map(|&id| {
            let node = tree.get(id).ok()?;
            Some(Row::from_node(
                id.raw(),
                node,
                tree.children_of(id).len() as u32,
                parent_total,
            ))
        })
        .collect();

    RowPage {
        parent_id: parent.raw(),
        path: tree
            .path_of(parent)
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        offset,
        total: children.len() as u32,
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nirmoka_core::Node;

    fn tree_with_two_children() -> Tree {
        let mut tree = Tree::new("/fixtures/root");
        let root = tree.push(None, Node::directory("root"));
        tree.push(Some(root), Node::file("big", 300));
        tree.push(Some(root), Node::file("small", 100));
        tree.rollup();
        tree
    }

    #[test]
    fn a_row_reports_its_share_of_the_parent() {
        let tree = tree_with_two_children();
        let page = page(&tree, tree.root().unwrap(), 0, 10);

        assert_eq!(page.rows.len(), 2);
        assert_eq!(page.rows[0].name, "big");
        assert!((page.rows[0].share - 0.75).abs() < f64::EPSILON);
        assert!((page.rows[1].share - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn an_empty_parent_reports_no_share_rather_than_dividing_by_zero() {
        let mut tree = Tree::new("/fixtures/root");
        let root = tree.push(None, Node::directory("root"));
        tree.push(Some(root), Node::file("empty", 0));
        tree.rollup();

        let page = page(&tree, root, 0, 10);
        assert_eq!(page.rows[0].share, 0.0);
    }

    #[test]
    fn a_window_reports_the_full_child_count_not_the_window_size() {
        let tree = tree_with_two_children();
        let page = page(&tree, tree.root().unwrap(), 1, 1);

        assert_eq!(page.rows.len(), 1, "asked for one row");
        assert_eq!(page.rows[0].name, "small", "offset skipped the largest");
        assert_eq!(page.total, 2, "but the scrollbar needs both");
    }

    #[test]
    fn an_offset_past_the_end_is_an_empty_page_not_an_error() {
        let tree = tree_with_two_children();
        let page = page(&tree, tree.root().unwrap(), 99, 10);

        assert!(page.rows.is_empty());
        assert_eq!(page.total, 2);
    }

    #[test]
    fn an_unsupported_version_survives_the_conversion_as_itself() {
        let detection = AdapterDetection::UnsupportedVersion {
            path: "/usr/bin/ncdu".into(),
            version: "1.19".into(),
            supported: ">=2.0, <3.0".into(),
        };

        match Detection::from(&detection) {
            Detection::UnsupportedVersion {
                path,
                version,
                supported,
            } => {
                assert_eq!(path, "/usr/bin/ncdu");
                assert_eq!(version, "1.19");
                assert_eq!(supported, ">=2.0, <3.0");
            }
            other => panic!("unsupported version collapsed into {other:?}"),
        }
    }
}
