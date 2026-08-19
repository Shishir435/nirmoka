//! Product-specific views over a completed scan tree.
//!
//! No filesystem walk happens here. These queries inspect the ncdu-produced
//! tree already held in Rust and return a capped, sorted window.

use std::collections::BTreeSet;

use nirmoka_core::{NodeId, Tree};

use crate::dto::{
    ApplicationInventory, ApplicationItem, DeveloperCategory, DeveloperInventory, DeveloperItem,
};
use crate::state::ScanId;

pub const MAX_INVENTORY_ROWS: usize = 500;

pub fn applications(scan_id: ScanId, tree: &Tree) -> ApplicationInventory {
    let mut rows = ids(tree)
        .filter_map(|id| {
            let node = tree.get(id).ok()?;
            if !node.is_dir() || !is_bundle_name(&node.name) {
                return None;
            }
            Some(ApplicationItem {
                id: id.raw(),
                name: node.name.trim_end_matches(".app").to_string(),
                path: tree.path_of(id).ok()?.display().to_string(),
                total_bytes: node.total_bytes,
                size_is_partial: node.size_is_partial(),
            })
        })
        .collect::<Vec<_>>();
    rows.sort_unstable_by(|a, b| {
        b.total_bytes
            .cmp(&a.total_bytes)
            .then_with(|| a.path.cmp(&b.path))
    });
    let total = rows.len().min(u32::MAX as usize) as u32;
    rows.truncate(MAX_INVENTORY_ROWS);
    ApplicationInventory {
        scan_id,
        total,
        rows,
    }
}

pub fn developer(scan_id: ScanId, tree: &Tree) -> DeveloperInventory {
    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    for id in ids(tree) {
        let Ok(node) = tree.get(id) else { continue };
        if !node.is_dir() || !is_developer_name(&node.name) {
            continue;
        }
        // Only now. `path_of` walks to the root and allocates the whole path,
        // and the segment list allocates again; doing that for every directory
        // in a scan of a home folder is millions of reconstructed paths to
        // answer a question the name alone already rules out.
        let path = match tree.path_of(id) {
            Ok(path) => path,
            Err(_) => continue,
        };
        let path_text = path.display().to_string();
        let segments: Vec<_> = path
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect();
        let category = if node.name == "DerivedData"
            && contains_pair(&segments, "Developer", "Xcode")
        {
            Some(DeveloperCategory::XcodeDerivedData)
        } else if node.name == "CoreSimulator" && segments.iter().any(|part| part == "Developer") {
            Some(DeveloperCategory::SimulatorData)
        } else if node.name == "Archives" && contains_pair(&segments, "Developer", "Xcode") {
            Some(DeveloperCategory::XcodeArchives)
        } else if matches!(node.name.as_str(), "Caches" | "Logs")
            && segments.iter().any(|part| part == "Developer")
        {
            Some(DeveloperCategory::DeveloperCaches)
        } else if node.name == "node_modules" {
            Some(DeveloperCategory::NodeModules)
        } else if node.name == ".git" {
            Some(DeveloperCategory::GitRepository)
        } else {
            None
        };
        let Some(category) = category else { continue };

        // A `.git` directory is evidence for its parent repository; report the
        // repository's actual scanned footprint rather than only metadata.
        let reported = if category == DeveloperCategory::GitRepository {
            tree.parent_of(id).unwrap_or(id)
        } else {
            id
        };
        if !seen.insert((reported.raw(), category as u8)) {
            continue;
        }
        let Ok(reported_node) = tree.get(reported) else {
            continue;
        };
        let Ok(reported_path) = tree.path_of(reported) else {
            continue;
        };
        rows.push(DeveloperItem {
            id: reported.raw(),
            category,
            name: reported_node.name.clone(),
            path: if reported == id {
                path_text
            } else {
                reported_path.display().to_string()
            },
            total_bytes: reported_node.total_bytes,
            modified_at_ms: None,
            size_is_partial: reported_node.size_is_partial(),
        });
    }
    rows.sort_unstable_by(|a, b| {
        b.total_bytes
            .cmp(&a.total_bytes)
            .then_with(|| a.path.cmp(&b.path))
    });
    let total = rows.len().min(u32::MAX as usize) as u32;
    rows.truncate(MAX_INVENTORY_ROWS);
    DeveloperInventory {
        scan_id,
        total,
        rows,
    }
}

/// Whether a name ends in `.app`, without allocating to find out.
///
/// `to_ascii_lowercase().ends_with(..)` allocates a `String` per node, and this
/// runs once for every entry in the scan — millions of allocations to answer a
/// question about the last four bytes.
fn is_bundle_name(name: &str) -> bool {
    name.len() > 4 && name[name.len() - 4..].eq_ignore_ascii_case(".app")
}

/// The names any developer rule could match.
///
/// A cheap gate in front of the expensive tests below, which need a
/// reconstructed path. Every rule in `developer` keys off one of these names,
/// so a node that is none of them cannot match whatever its location.
fn is_developer_name(name: &str) -> bool {
    matches!(
        name,
        "DerivedData" | "Archives" | "CoreSimulator" | "Caches" | "Logs" | "node_modules" | ".git"
    )
}

fn ids(tree: &Tree) -> impl Iterator<Item = NodeId> + '_ {
    (0..tree.len()).filter_map(|raw| tree.node_id(raw as u32).ok())
}

fn contains_pair(segments: &[std::borrow::Cow<'_, str>], first: &str, second: &str) -> bool {
    segments
        .windows(2)
        .any(|pair| pair[0] == first && pair[1] == second)
}

#[cfg(test)]
mod tests {
    use nirmoka_core::Node;

    use super::*;

    #[test]
    fn application_inventory_uses_scanned_bundle_sizes() {
        let mut tree = Tree::new("/Applications");
        let root = tree.push(None, Node::directory("Applications"));
        let app = tree.push(Some(root), Node::directory("Example.app"));
        tree.push(Some(app), Node::file("binary", 4096));
        tree.rollup();

        let inventory = applications(9, &tree);
        assert_eq!(inventory.scan_id, 9);
        assert_eq!(inventory.total, 1);
        assert_eq!(inventory.rows[0].name, "Example");
        assert_eq!(inventory.rows[0].path, "/Applications/Example.app");
        assert_eq!(inventory.rows[0].total_bytes, 4096);
    }

    /// The gate in front of the expensive tests. Getting it wrong silently
    /// drops rows, so every name the rules below key off is listed here.
    #[test]
    fn the_name_gate_admits_exactly_what_the_rules_can_match() {
        for name in [
            "DerivedData",
            "Archives",
            "CoreSimulator",
            "Caches",
            "Logs",
            "node_modules",
            ".git",
        ] {
            assert!(is_developer_name(name), "{name} is a rule's own name");
        }
        for name in ["Documents", "derivedData", "node_modules2", ""] {
            assert!(!is_developer_name(name), "{name} matches no rule");
        }
    }

    #[test]
    fn a_bundle_name_is_matched_without_allocating() {
        assert!(is_bundle_name("Docker.app"));
        // A case-preserving filesystem means the suffix is not always lowercase.
        assert!(is_bundle_name("Docker.APP"));
        assert!(
            !is_bundle_name(".app"),
            "a bundle has a name before the suffix"
        );
        assert!(!is_bundle_name("notes.txt"));
        assert!(!is_bundle_name("app"));
        assert!(!is_bundle_name(""));
    }

    #[test]
    fn git_evidence_reports_the_repository_not_only_dot_git() {
        let mut tree = Tree::new("/projects");
        let root = tree.push(None, Node::directory("projects"));
        let repo = tree.push(Some(root), Node::directory("nirmoka"));
        tree.push(Some(repo), Node::file("Cargo.toml", 100));
        let git = tree.push(Some(repo), Node::directory(".git"));
        tree.push(Some(git), Node::file("index", 20));
        tree.rollup();

        let inventory = developer(3, &tree);
        let item = inventory
            .rows
            .iter()
            .find(|item| item.category == DeveloperCategory::GitRepository)
            .unwrap();
        assert_eq!(item.path, "/projects/nirmoka");
        assert_eq!(item.total_bytes, 120);
        assert_eq!(item.modified_at_ms, None);
    }
}
