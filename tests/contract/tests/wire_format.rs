//! The wire format, checked against recorded backend output.
//!
//! Every file under `fixtures/` came out of a real backend, so these tests are
//! the difference between "the parser handles what the documentation says" and
//! "the parser handles what the tool emits". They need no backend installed,
//! which is what lets them run on Windows CI where ncdu does not exist.

use nirmoka_adapter::wire::{self, TreeStats, WireStats};
use nirmoka_contract_tests::{all_fixtures, Fixture};
use nirmoka_core::{NodeId, NodeKind, Tree};

fn parse(fixture: &Fixture) -> (Tree, WireStats, TreeStats) {
    wire::parse_tree(fixture.open()).unwrap_or_else(|error| panic!("{}: {error}", fixture.label()))
}

fn by_name(tree: &Tree, parent: NodeId, name: &str) -> NodeId {
    *tree
        .children_of(parent)
        .iter()
        .find(|id| tree.get(**id).unwrap().name == name)
        .unwrap_or_else(|| panic!("no child named {name}"))
}

fn walk(tree: &Tree, mut visit: impl FnMut(NodeId)) {
    let Some(root) = tree.root() else { return };
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        visit(id);
        pending.extend_from_slice(tree.children_of(id));
    }
}

#[test]
fn every_recorded_export_parses() {
    for fixture in all_fixtures() {
        let (tree, stats, _) = parse(&fixture);

        assert_eq!(
            stats.header.format_major,
            wire::SUPPORTED_FORMAT_MAJOR,
            "{}: recorded from an unsupported format",
            fixture.label()
        );
        assert!(
            stats.header.program.is_some(),
            "{}: no producer recorded",
            fixture.label()
        );
        assert!(tree.root().is_some(), "{}: no root", fixture.label());
        assert_eq!(
            tree.len() as u64,
            stats.items,
            "{}: tree size disagrees with the parsed entry count",
            fixture.label()
        );
    }
}

#[test]
fn every_tree_rolls_up_consistently() {
    for fixture in all_fixtures() {
        let (tree, _, _) = parse(&fixture);

        walk(&tree, |id| {
            let node = tree.get(id).unwrap();
            let children: u64 = tree
                .children_of(id)
                .iter()
                .map(|child| tree.get(*child).unwrap().total_bytes)
                .sum();

            assert_eq!(
                node.total_bytes,
                node.own_bytes + children,
                "{}: {} does not equal its own size plus its children",
                fixture.label(),
                node.name
            );
        });
    }
}

#[test]
fn every_path_reconstructs_under_the_scan_root() {
    for fixture in all_fixtures() {
        let (tree, _, _) = parse(&fixture);
        let root_path = tree.root_path().to_path_buf();

        walk(&tree, |id| {
            let path = tree
                .path_of(id)
                .unwrap_or_else(|error| panic!("{}: {error}", fixture.label()));
            assert!(
                path.starts_with(&root_path),
                "{}: {} escaped the scan root",
                fixture.label(),
                path.display()
            );
        });
    }
}

#[test]
fn only_directories_have_children() {
    for fixture in all_fixtures() {
        let (tree, _, _) = parse(&fixture);

        walk(&tree, |id| {
            let node = tree.get(id).unwrap();
            if !tree.children_of(id).is_empty() {
                assert!(
                    node.is_dir(),
                    "{}: {} has children but is {:?}",
                    fixture.label(),
                    node.name,
                    node.kind
                );
            }
        });
    }
}

#[test]
fn parsing_is_deterministic() {
    // Two parses of the same bytes must agree. Anything that depends on hash
    // iteration order — inode deduplication does — would show up here.
    for fixture in all_fixtures() {
        let (first, first_stats, first_tree_stats) = parse(&fixture);
        let (second, second_stats, second_tree_stats) = parse(&fixture);

        assert_eq!(first_stats, second_stats, "{}", fixture.label());
        assert_eq!(first_tree_stats, second_tree_stats, "{}", fixture.label());
        assert_eq!(
            first.get(first.root().unwrap()).unwrap().total_bytes,
            second.get(second.root().unwrap()).unwrap().total_bytes,
            "{}",
            fixture.label()
        );
    }
}

// ---------------------------------------------------------------------------
// Specific expectations about what the recorded trees contain.
//
// These name the exact awkward cases the fixtures were built to hold. If a
// re-recording changes them, that is a real change in backend behaviour and
// belongs in a commit message, not in a silently updated number.
// ---------------------------------------------------------------------------

fn fixture_named(backend: &str, name: &str) -> Fixture {
    all_fixtures()
        .into_iter()
        .find(|fixture| fixture.backend == backend && fixture.name == name)
        .unwrap_or_else(|| panic!("no fixture named {backend}/{name}"))
}

#[test]
fn the_simple_tree_holds_every_awkward_case() {
    let (tree, stats, tree_stats) = parse(&fixture_named("ncdu", "simple"));
    let root = tree.root().unwrap();

    assert_eq!(stats.directories, 5); // root, empty, unreadable, nested, deeper

    // Sparse: 4 MB apparent, nothing on disk. Reporting the apparent size would
    // promise space that deleting it does not free.
    let sparse = tree.get(by_name(&tree, root, "sparse.img")).unwrap();
    assert_eq!(sparse.apparent_bytes, 4 * 1024 * 1024);
    assert_eq!(sparse.own_bytes, 0);

    // Unreadable: flagged, kept in the tree, and counted as a warning.
    let unreadable = tree.get(by_name(&tree, root, "unreadable")).unwrap();
    assert!(unreadable.read_error);
    assert!(unreadable.size_is_partial());
    assert_eq!(tree_stats.read_errors, 1);

    // Hardlink: one inode, two names, counted once.
    assert_eq!(tree_stats.hardlinks_deduplicated, 1);
    assert!(tree_stats.hardlink_bytes_saved > 0);

    let nested = by_name(&tree, root, "nested");
    let deeper = by_name(&tree, nested, "deeper");
    let hardlink = tree.get(by_name(&tree, deeper, "hardlink.txt")).unwrap();
    let original = tree.get(by_name(&tree, nested, "regular.txt")).unwrap();

    assert!(hardlink.hardlink);
    assert_eq!(hardlink.own_bytes, 0);
    assert!(!original.hardlink);
    assert!(original.own_bytes > 0);
    // A deduplicated hardlink is a correct zero, not an incomplete number.
    assert!(!hardlink.size_is_partial());

    // Symlink: not a regular file. Without extended mode the format cannot say
    // more than that, and the parser must not invent it.
    let link = tree.get(by_name(&tree, deeper, "link")).unwrap();
    assert_eq!(link.kind, NodeKind::Other);

    // Empty directory: a directory with nothing in it, not a file.
    let empty = by_name(&tree, root, "empty");
    assert!(tree.get(empty).unwrap().is_dir());
    assert!(tree.children_of(empty).is_empty());
}

#[test]
fn the_excluded_tree_keeps_what_it_skipped() {
    let (tree, _, tree_stats) = parse(&fixture_named("ncdu", "excluded"));

    assert!(tree_stats.excluded > 0);

    let mut found = 0;
    walk(&tree, |id| {
        let node = tree.get(id).unwrap();
        if node.excluded {
            found += 1;
            // An excluded entry has an unknown size, which the flag says and
            // the number cannot.
            assert_eq!(node.own_bytes, 0);
            assert!(node.size_is_partial());
        }
    });

    assert_eq!(found, tree_stats.excluded);
}

#[test]
fn an_extended_export_identifies_what_a_plain_one_cannot() {
    // `ncdu -e` records mode, which is the only way to tell a symlink from a
    // socket, or an excluded directory from an excluded file.
    let (tree, _, _) = parse(&fixture_named("ncdu", "extended"));
    let root = tree.root().unwrap();
    let nested = by_name(&tree, root, "nested");
    let deeper = by_name(&tree, nested, "deeper");

    assert_eq!(
        tree.get(by_name(&tree, deeper, "link")).unwrap().kind,
        NodeKind::Symlink
    );
    assert_eq!(
        tree.get(by_name(&tree, deeper, "blocks.bin")).unwrap().kind,
        NodeKind::File
    );
}

#[test]
fn a_root_with_no_children_is_a_tree_of_one() {
    // The degenerate export, and the one a parser that assumes at least one
    // child gets wrong.
    for fixture in all_fixtures()
        .into_iter()
        .filter(|fixture| fixture.name == "empty-root")
    {
        let (tree, stats, _) = parse(&fixture);

        assert_eq!(stats.items, 1, "{}", fixture.label());
        assert_eq!(tree.len(), 1, "{}", fixture.label());

        let root = tree.root().unwrap();
        assert!(tree.children_of(root).is_empty(), "{}", fixture.label());
        assert!(tree.get(root).unwrap().is_dir(), "{}", fixture.label());
        assert_eq!(tree.path_of(root).unwrap(), tree.root_path());
    }
}

#[test]
fn gdu_records_the_shared_wire_format_without_translation() {
    let fixture = fixture_named("gdu", "simple");
    let (tree, stats, tree_stats) = parse(&fixture);

    assert_eq!(stats.header.program.as_deref(), Some("gdu"));
    assert_eq!(stats.header.program_version.as_deref(), Some("v5.32.0"));
    assert_eq!(stats.header.format_major, 1);
    assert_eq!(stats.header.format_minor, 2);
    assert_eq!(tree_stats.hardlinks_deduplicated, 1);
    // Fixtures use a neutral `/fixtures/...` root. It has a filesystem root on
    // every platform, but Windows reserves `is_absolute` for drive-qualified
    // paths such as `C:\\fixtures`.
    assert!(tree.path_of(tree.root().unwrap()).unwrap().has_root());
}
