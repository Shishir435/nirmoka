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
    /// Ids handed to a frontend come back as plain integers, and nothing stops
    /// one arriving after a rescan replaced the tree it referred to. This is
    /// where that becomes an error instead of a panic or, worse, a row from an
    /// unrelated node.
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
        let mut sorted = self.children_of(id).to_vec();
        sorted.sort_unstable_by(|a, b| {
            self.nodes[b.index()]
                .total_bytes
                .cmp(&self.nodes[a.index()].total_bytes)
                .then_with(|| self.nodes[a.index()].name.cmp(&self.nodes[b.index()].name))
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

        let stale = tree.node_id(9_999);
        assert!(
            matches!(stale, Err(CoreError::UnknownNode(9_999))),
            "an id from a tree that no longer exists must not resolve"
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
