//! Arena-backed scan tree.
//!
//! Nodes live in a flat `Vec` and reference each other by index. This is chosen
//! deliberately over `Rc<RefCell<Node>>`: a home-directory scan can produce
//! millions of entries, and an arena keeps allocation count and cache behaviour
//! predictable.
//!
//! **The tree stays in Rust.** The frontend never receives a whole tree — only
//! the visible window plus aggregates. Shipping millions of nodes into a webview
//! is the mistake that gets misattributed to the GUI framework.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::node::Node;

/// Index into a [`Tree`]. Only meaningful for the tree that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeId(u32);

/// How a directory's children are ordered.
///
/// Ordering happens here rather than in the frontend, and that is invariant 5
/// rather than a preference: the window the UI holds is a few dozen rows out of
/// a directory that may have a hundred thousand, so sorting client-side would
/// only ever sort the visible slice. Re-sorting means asking for the window
/// again.
///
/// Each variant names both the key and the direction. A separate `ascending`
/// flag would have to answer what ascending means for a size, where the useful
/// default is largest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Sort {
    /// Biggest consumer at the top. What the tool is for.
    #[default]
    LargestFirst,
    SmallestFirst,
    NameAscending,
    NameDescending,
}

impl NodeId {
    pub fn index(self) -> usize {
        self.0 as usize
    }

    pub fn raw(self) -> u32 {
        self.0
    }
}

/// A scanned directory tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tree {
    root_path: PathBuf,
    nodes: Vec<Node>,
    children: Vec<Vec<NodeId>>,
    parents: Vec<Option<NodeId>>,
}

impl Tree {
    pub fn new(root_path: impl Into<PathBuf>) -> Self {
        Self {
            root_path: root_path.into(),
            nodes: Vec::new(),
            children: Vec::new(),
            parents: Vec::new(),
        }
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// The first node pushed, by construction.
    pub fn root(&self) -> Option<NodeId> {
        (!self.nodes.is_empty()).then_some(NodeId(0))
    }

    /// Append a node. Parents must be pushed before their children — the
    /// bottom-up pass in [`Tree::rollup`] depends on it.
    pub fn push(&mut self, parent: Option<NodeId>, node: Node) -> NodeId {
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(node);
        self.children.push(Vec::new());
        self.parents.push(parent);

        if let Some(parent) = parent {
            self.children[parent.index()].push(id);
        }

        id
    }

    pub fn get(&self, id: NodeId) -> Result<&Node> {
        self.nodes
            .get(id.index())
            .ok_or(CoreError::UnknownNode(id.raw()))
    }

    /// Turn a raw id from outside this process into one that indexes this tree.
    ///
    /// Ids handed to a frontend come back as plain integers, so this is where an
    /// out-of-range one becomes an error rather than a panic.
    ///
    /// **It is a bounds check, not a staleness check.** Every tree numbers its
    /// nodes from zero, so an id left over from a scan that has since been
    /// replaced resolves here whenever the new tree is long enough — and names a
    /// different file. A `Tree` cannot tell: it has no idea another one ever
    /// existed. Whoever hands ids out has to pair them with the scan that issued
    /// them, which is what `nirmoka_app::state::ScanId` does.
    pub fn node_id(&self, raw: u32) -> Result<NodeId> {
        let id = NodeId(raw);
        if id.index() < self.nodes.len() {
            Ok(id)
        } else {
            Err(CoreError::UnknownNode(raw))
        }
    }

    pub fn children_of(&self, id: NodeId) -> &[NodeId] {
        self.children
            .get(id.index())
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn parent_of(&self, id: NodeId) -> Option<NodeId> {
        self.parents.get(id.index()).copied().flatten()
    }

    /// Every node between the root and `id`, root first, `id` excluded.
    ///
    /// This is what a breadcrumb is made of, and it is also the only way back
    /// out of a directory: the frontend holds one node id at a time, so without
    /// the chain it would have no way to name the parent it came from.
    ///
    /// Bounded by depth, not by size — a path is dozens of entries where a
    /// directory is millions, so returning the whole chain does not put a tree
    /// on the wire.
    pub fn ancestors_of(&self, id: NodeId) -> Vec<NodeId> {
        let mut chain = Vec::new();
        let mut cursor = self.parent_of(id);

        while let Some(current) = cursor {
            chain.push(current);
            cursor = self.parent_of(current);
        }

        chain.reverse();
        chain
    }

    /// Reconstruct the absolute path of a node by walking to the root.
    pub fn path_of(&self, id: NodeId) -> Result<PathBuf> {
        let mut segments = Vec::new();
        let mut cursor = Some(id);

        while let Some(current) = cursor {
            segments.push(self.get(current)?.name.clone());
            cursor = self.parent_of(current);
        }

        // The root node's name is the scan root itself, so drop it and join the
        // rest onto root_path. Using PathBuf::push throughout keeps this correct
        // on Windows separators.
        segments.pop();

        let mut path = self.root_path.clone();
        for segment in segments.iter().rev() {
            path.push(segment);
        }

        Ok(path)
    }

    /// Fill in `total_bytes` for every node, bottom-up.
    ///
    /// Relies on parents having lower indices than their children, which
    /// [`Tree::push`] guarantees. Saturating arithmetic so a pathological tree
    /// cannot panic in release or wrap in debug.
    pub fn rollup(&mut self) {
        for index in (0..self.nodes.len()).rev() {
            let children_total: u64 = self.children[index]
                .iter()
                .map(|child| self.nodes[child.index()].total_bytes)
                .fold(0u64, u64::saturating_add);

            self.nodes[index].total_bytes =
                self.nodes[index].own_bytes.saturating_add(children_total);
        }
    }

    /// Children of `id`, largest first. Used by every list view.
    pub fn children_by_size(&self, id: NodeId) -> Vec<NodeId> {
        self.children_sorted(id, Sort::LargestFirst)
    }

    /// Children of `id` in the requested order.
    ///
    /// Every comparison falls through to the id, so the result is a total order
    /// even when two siblings share a size and a name — which a case-preserving
    /// filesystem can produce. Without the fallback, `sort_unstable_by` would be
    /// free to return either arrangement, and a window scrolled twice could show
    /// the same row at two different offsets.
    pub fn children_sorted(&self, id: NodeId, sort: Sort) -> Vec<NodeId> {
        let mut sorted = self.children_of(id).to_vec();

        let size_of = |id: &NodeId| self.nodes[id.index()].total_bytes;
        let name_of = |id: &NodeId| self.nodes[id.index()].name.as_str();

        sorted.sort_unstable_by(|a, b| {
            match sort {
                Sort::LargestFirst => size_of(b).cmp(&size_of(a)),
                Sort::SmallestFirst => size_of(a).cmp(&size_of(b)),
                Sort::NameAscending => name_of(a).cmp(name_of(b)),
                Sort::NameDescending => name_of(b).cmp(name_of(a)),
            }
            .then_with(|| name_of(a).cmp(name_of(b)))
            .then_with(|| a.cmp(b))
        });

        sorted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::Node;

    #[test]
    fn a_raw_id_from_outside_is_checked_against_this_tree() {
        let tree = sample();

        let root = tree.node_id(0).expect("index 0 exists");
        assert_eq!(tree.get(root).unwrap().name, "root");

        let out_of_range = tree.node_id(9_999);
        assert!(
            matches!(out_of_range, Err(CoreError::UnknownNode(9_999))),
            "an id past the end must be refused rather than indexed"
        );
    }

    fn sample() -> Tree {
        // /root
        //   a/        (dir)
        //     big     3000
        //     small     10
        //   b           500
        let mut tree = Tree::new("/root");
        let root = tree.push(None, Node::directory("root"));
        let a = tree.push(Some(root), Node::directory("a"));
        tree.push(Some(a), Node::file("big", 3000));
        tree.push(Some(a), Node::file("small", 10));
        tree.push(Some(root), Node::file("b", 500));
        tree.rollup();
        tree
    }

    #[test]
    fn rolls_sizes_up_to_the_root() {
        let tree = sample();
        let root = tree.root().unwrap();
        assert_eq!(tree.get(root).unwrap().total_bytes, 3510);
    }

    #[test]
    fn rolls_intermediate_directories() {
        let tree = sample();
        let a = tree.children_of(tree.root().unwrap())[0];
        assert_eq!(tree.get(a).unwrap().total_bytes, 3010);
    }

    #[test]
    fn sorts_children_largest_first() {
        let tree = sample();
        let root = tree.root().unwrap();
        let ordered: Vec<&str> = tree
            .children_by_size(root)
            .iter()
            .map(|id| tree.get(*id).unwrap().name.as_str())
            .collect();
        assert_eq!(ordered, vec!["a", "b"]);
    }

    #[test]
    fn sorts_by_each_key_and_direction() {
        let tree = sample();
        let root = tree.root().unwrap();

        let names = |sort| -> Vec<&str> {
            tree.children_sorted(root, sort)
                .iter()
                .map(|id| tree.get(*id).unwrap().name.as_str())
                .collect()
        };

        // a/ holds 3010 bytes, b holds 500.
        assert_eq!(names(Sort::LargestFirst), vec!["a", "b"]);
        assert_eq!(names(Sort::SmallestFirst), vec!["b", "a"]);
        assert_eq!(names(Sort::NameAscending), vec!["a", "b"]);
        assert_eq!(names(Sort::NameDescending), vec!["b", "a"]);
    }

    /// Two windows of one directory have to agree on where a row sits, or
    /// scrolling past a tie shows the same entry twice and skips another.
    #[test]
    fn siblings_that_tie_on_every_key_still_have_one_order() {
        let mut tree = Tree::new("/root");
        let root = tree.push(None, Node::directory("root"));
        for _ in 0..8 {
            tree.push(Some(root), Node::file("same", 100));
        }
        tree.rollup();

        let once = tree.children_sorted(root, Sort::LargestFirst);
        let again = tree.children_sorted(root, Sort::LargestFirst);
        assert_eq!(once, again, "a tie must not be resolved differently twice");
    }

    #[test]
    fn ancestors_run_from_the_root_down_to_the_parent() {
        let tree = sample();
        let root = tree.root().unwrap();
        let a = tree.children_of(root)[0];
        let big = tree.children_of(a)[0];

        assert_eq!(tree.ancestors_of(big), vec![root, a]);
        assert_eq!(tree.ancestors_of(a), vec![root]);
        assert!(
            tree.ancestors_of(root).is_empty(),
            "the root has nowhere further out to go"
        );
    }

    #[test]
    fn reconstructs_paths_without_storing_them() {
        let tree = sample();
        let a = tree.children_of(tree.root().unwrap())[0];
        let big = tree.children_of(a)[0];
        assert_eq!(tree.path_of(big).unwrap(), PathBuf::from("/root/a/big"));
    }

    #[test]
    fn root_path_is_the_scan_root() {
        let tree = sample();
        let root = tree.root().unwrap();
        assert_eq!(tree.path_of(root).unwrap(), PathBuf::from("/root"));
    }

    #[test]
    fn unknown_node_is_an_error_not_a_panic() {
        let tree = sample();
        assert!(tree.get(NodeId(999)).is_err());
    }

    #[test]
    fn empty_tree_has_no_root() {
        let tree = Tree::new("/root");
        assert!(tree.root().is_none());
        assert!(tree.is_empty());
    }
}
