//! What kind of thing is using the disk.
//!
//! The scan tree sorts by size and knows nothing about kind: `/Applications`
//! and `~/Downloads` are two directories that differ only in name. The dashboard
//! asks a question the tree cannot answer — how much of this disk is
//! applications, how much is my own files, how much is build output — so this
//! module answers it from paths, which is the only evidence a scan carries.
//!
//! # Why bytes are attributed, not nodes
//!
//! A naive pass that classified every node and summed `total_bytes` would count
//! `~/Documents` once and everything inside it again. So each node contributes
//! its `own_bytes` exactly once, to the category claimed by the nearest
//! ancestor-or-self that matches a rule. The categories therefore partition the
//! scan: their totals sum to the scanned size, which is asserted in the tests
//! rather than hoped for.
//!
//! That also settles nesting the way a user would expect. `~/Documents/app`
//! is personal, and the `node_modules` inside it is build output — the deeper
//! claim wins because it is nearer.

use std::collections::HashMap;
use std::path::Path;

use nirmoka_core::{NodeId, Tree};

use crate::dto::{CategoryBreakdown, CategoryConsumer, CategorySummary, StorageCategory};
use crate::state::ScanId;

/// Consumers reported per category, ranked by the bytes they account for.
///
/// The dashboard shows a handful and the list is a summary, not a browser: the
/// tree view is what exists for looking at everything.
const MAX_CONSUMERS: usize = 8;

/// Application bundles carried past that cap.
///
/// A bundle's size on disk is a fraction of what the application costs — see
/// ADR 0028 — and the footprint that says so is measured later, on the rows
/// this list hands over. Cutting at eight by bundle size therefore drops
/// exactly the applications that belong at the top: a 300 MB bundle in
/// eleventh place can carry tens of gigabytes under `~/Library`.
///
/// So bundles ride along beyond the ranked cap. They cost nothing on screen —
/// the dashboard still shows six rows — and they only take a place once their
/// real number earns it. The count is bounded because each footprint is a
/// filesystem walk, not because eight was ever the right number of candidates.
const MAX_BUNDLE_CANDIDATES: usize = 16;

/// Directories a user's own files live in, directly under home.
const PERSONAL_DIRECTORIES: &[&str] = &[
    "Documents",
    "Desktop",
    "Downloads",
    "Movies",
    "Music",
    "Pictures",
    "Public",
];

/// Toolchain and package-manager stores, directly under home.
///
/// Each is build output or a re-downloadable cache rather than anything the
/// user wrote, which is the distinction the Development category draws.
const DEVELOPER_HOME_DIRECTORIES: &[&str] = &[
    ".cargo",
    ".rustup",
    ".gradle",
    ".m2",
    ".npm",
    ".bun",
    ".deno",
    ".pub-cache",
    ".pnpm-store",
    ".nuget",
    ".cocoapods",
    "go",
];

/// Absolute roots the operating system owns.
///
/// `/Library` is here and `~/Library` deliberately is not: one is the system's,
/// the other is where applications keep the user's data, which is what makes it
/// part of what an application costs.
const SYSTEM_ROOTS: &[&str] = &[
    "/System",
    "/Library",
    "/usr",
    "/bin",
    "/sbin",
    "/opt",
    "/private",
    "/cores",
    "/Windows",
    "/Program Files",
];

/// Which category a node claims, or `None` to inherit its parent's.
///
/// Order is precedence, and it is checked at every node rather than once at the
/// top, so a rule that matches deeper always wins. Development precedes
/// Applications so that `Xcode.app`'s DerivedData does not read as the
/// application, and precedes Personal Files so that a repository checked out
/// into `~/Documents` reports its `node_modules` as build output.
fn claim(node_name: &str, path: &Path, home: &Path, is_dir: bool) -> Option<StorageCategory> {
    let parent = path.parent();
    let under_home = |name: &str| parent == Some(home) && node_name == name;

    // Development.
    if node_name == "node_modules" || node_name == ".git" {
        return Some(StorageCategory::Development);
    }
    if DEVELOPER_HOME_DIRECTORIES
        .iter()
        .any(|name| under_home(name))
    {
        return Some(StorageCategory::Development);
    }
    if is_developer_data(node_name, path) {
        return Some(StorageCategory::Development);
    }

    // Applications.
    if is_dir && node_name.to_ascii_lowercase().ends_with(".app") {
        return Some(StorageCategory::Apps);
    }
    if path == Path::new("/Applications") || path == home.join("Applications") {
        return Some(StorageCategory::Apps);
    }
    // Where applications keep the user's data. Named by the system, owned by
    // whatever installed into it — see the module note on `/Library`.
    if path == home.join("Library") {
        return Some(StorageCategory::Apps);
    }

    // Personal files.
    if PERSONAL_DIRECTORIES.iter().any(|name| under_home(name)) {
        return Some(StorageCategory::PersonalFiles);
    }

    // System.
    if SYSTEM_ROOTS.iter().any(|root| path == Path::new(root)) {
        return Some(StorageCategory::System);
    }

    None
}

/// Xcode and simulator working data, which is evidence-based rather than a
/// fixed location: `Developer` appears under both `~/Library` and `/Library`.
fn is_developer_data(node_name: &str, path: &Path) -> bool {
    let segments: Vec<_> = path
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect();
    let under_developer = segments.iter().any(|part| part == "Developer");

    match node_name {
        "DerivedData" | "Archives" => segments
            .windows(2)
            .any(|pair| pair[0] == "Developer" && pair[1] == "Xcode"),
        "CoreSimulator" => under_developer,
        "Caches" | "Logs" => under_developer,
        _ => false,
    }
}

/// Classify the whole scan, and name the biggest thing in each category.
///
/// One pass, carrying each node's claimed category to its children. `home` is
/// passed in rather than looked up so the rules are testable against paths this
/// process never has to own.
pub fn breakdown(
    scan_id: ScanId,
    tree: &Tree,
    home: &Path,
    volume: Option<crate::dto::VolumeInfo>,
) -> CategoryBreakdown {
    let mut totals: HashMap<StorageCategory, u64> = HashMap::new();
    // Bytes attributed to the node that claimed them, which is what makes a
    // consumer's size the part it actually accounts for: `~/Documents` reports
    // its personal bytes, not the `node_modules` that were counted elsewhere.
    //
    // The flag beside them is whether every one of those bytes was read in
    // full. It is not the claiming node's own `size_is_partial`, which says
    // only that its directory entry read cleanly, and it is not its whole
    // subtree's either — the `node_modules` below it belongs to another
    // consumer, and its errors are that consumer's to report. What is
    // accumulated here is exactly what is attributed here.
    let mut claimed: HashMap<NodeId, (u64, bool)> = HashMap::new();

    let Some(root) = tree.root() else {
        return empty(scan_id, tree, volume);
    };

    // (node, inherited category, the node that claimed it)
    let mut pending = vec![(root, StorageCategory::Other, None::<NodeId>)];
    while let Some((id, inherited, claimer)) = pending.pop() {
        let Ok(node) = tree.get(id) else { continue };
        let Ok(path) = tree.path_of(id) else { continue };

        let claimed_here = claim(&node.name, &path, home, node.is_dir());
        let category = claimed_here.unwrap_or(inherited);
        // A direct child of the root is a consumer even when it claims nothing,
        // or the default category would have no rows to show at all.
        let owner = if claimed_here.is_some() || tree.parent_of(id) == Some(root) {
            Some(id)
        } else {
            claimer
        };

        *totals.entry(category).or_default() += node.own_bytes;
        if let Some(owner) = owner {
            let entry = claimed.entry(owner).or_insert((0, true));
            entry.0 += node.own_bytes;
            entry.1 &= !node.size_is_partial();
        }

        pending.extend(
            tree.children_of(id)
                .iter()
                .map(|child| (*child, category, owner)),
        );
    }

    // A consumer is reported under the category it claimed, so the same pass
    // that totalled the bytes decides where the row belongs.
    let mut consumers: HashMap<StorageCategory, Vec<CategoryConsumer>> = HashMap::new();
    for (id, (bytes, complete)) in claimed {
        let (Ok(node), Ok(path)) = (tree.get(id), tree.path_of(id)) else {
            continue;
        };
        let category = claim(&node.name, &path, home, node.is_dir())
            .or_else(|| inherited_category(tree, id, home))
            .unwrap_or(StorageCategory::Other);
        consumers
            .entry(category)
            .or_default()
            .push(CategoryConsumer {
                id: id.raw(),
                name: node.name.clone(),
                path: path.display().to_string(),
                is_dir: node.is_dir(),
                // The window addresses the scan root as `null`, so the root's
                // own id would name a location it does not recognise.
                parent_id: tree
                    .parent_of(id)
                    .filter(|parent| Some(*parent) != tree.root())
                    .map(|parent| parent.raw()),
                total_bytes: bytes,
                size_is_partial: !complete,
            });
    }

    let scanned_bytes = totals.values().copied().fold(0u64, u64::saturating_add);
    let categories = StorageCategory::ALL
        .iter()
        .map(|category| {
            let total_bytes = totals.get(category).copied().unwrap_or(0);
            let mut rows = consumers.remove(category).unwrap_or_default();
            rows.sort_unstable_by(|a, b| {
                b.total_bytes
                    .cmp(&a.total_bytes)
                    .then_with(|| a.path.cmp(&b.path))
            });
            trim_consumers(&mut rows);
            CategorySummary {
                category: *category,
                total_bytes,
                share: share_of(total_bytes, scanned_bytes),
                consumers: rows,
            }
        })
        .collect();

    CategoryBreakdown {
        scan_id,
        root_path: tree.root_path().display().to_string(),
        scanned_bytes,
        volume,
        categories,
    }
}

/// Cut a category's sorted consumers down to what the dashboard needs.
///
/// Two rules, not one: the largest [`MAX_CONSUMERS`] by attributed bytes, plus
/// application bundles up to [`MAX_BUNDLE_CANDIDATES`] whatever their rank.
/// Order is untouched, so the caller's sort still holds.
fn trim_consumers(rows: &mut Vec<CategoryConsumer>) {
    let mut ranked = 0usize;
    let mut bundles = 0usize;
    rows.retain(|row| {
        let bundle = is_app_bundle(row);
        let keep = ranked < MAX_CONSUMERS || (bundle && bundles < MAX_BUNDLE_CANDIDATES);
        if keep {
            ranked += 1;
            if bundle {
                bundles += 1;
            }
        }
        keep
    });
}

/// A `.app` directory: the same test [`claim`] uses, and the same one the
/// frontend uses to decide a row is measured as an application.
fn is_app_bundle(row: &CategoryConsumer) -> bool {
    row.is_dir && row.name.to_ascii_lowercase().ends_with(".app")
}

/// The category a node inherits, for a consumer that claimed nothing itself.
fn inherited_category(tree: &Tree, id: NodeId, home: &Path) -> Option<StorageCategory> {
    for ancestor in tree.ancestors_of(id).into_iter().rev() {
        let (Ok(node), Ok(path)) = (tree.get(ancestor), tree.path_of(ancestor)) else {
            continue;
        };
        if let Some(category) = claim(&node.name, &path, home, node.is_dir()) {
            return Some(category);
        }
    }
    None
}

fn share_of(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        return 0.0;
    }
    part as f64 / whole as f64
}

fn empty(
    scan_id: ScanId,
    tree: &Tree,
    volume: Option<crate::dto::VolumeInfo>,
) -> CategoryBreakdown {
    CategoryBreakdown {
        scan_id,
        root_path: tree.root_path().display().to_string(),
        scanned_bytes: 0,
        volume,
        categories: StorageCategory::ALL
            .iter()
            .map(|category| CategorySummary {
                category: *category,
                total_bytes: 0,
                share: 0.0,
                consumers: Vec::new(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use nirmoka_core::Node;
    use std::path::PathBuf;

    use super::*;

    fn home() -> PathBuf {
        PathBuf::from("/users/example")
    }

    fn classify(path: &str, is_dir: bool) -> Option<StorageCategory> {
        let path = PathBuf::from(path);
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        claim(&name, &path, &home(), is_dir)
    }

    #[test]
    fn the_rules_are_deterministic_against_path_strings() {
        assert_eq!(classify("/Applications", true), Some(StorageCategory::Apps));
        assert_eq!(
            classify("/Applications/Example.app", true),
            Some(StorageCategory::Apps)
        );
        assert_eq!(
            classify("/users/example/Library", true),
            Some(StorageCategory::Apps)
        );
        assert_eq!(
            classify("/users/example/Downloads", true),
            Some(StorageCategory::PersonalFiles)
        );
        assert_eq!(
            classify("/users/example/.cargo", true),
            Some(StorageCategory::Development)
        );
        assert_eq!(classify("/System", true), Some(StorageCategory::System));
        assert_eq!(classify("/Library", true), Some(StorageCategory::System));
        assert_eq!(classify("/users/example/scratch", true), None);
    }

    /// `/Library` is the system's and `~/Library` is where applications keep
    /// the user's data. Same name, different owner, and the rule has to tell
    /// them apart by path rather than by name.
    #[test]
    fn the_two_library_directories_are_not_the_same_thing() {
        assert_eq!(classify("/Library", true), Some(StorageCategory::System));
        assert_eq!(
            classify("/users/example/Library", true),
            Some(StorageCategory::Apps)
        );
    }

    /// A directory named for a personal folder that is not directly under home
    /// is just a directory. Matching on the name alone would file every
    /// `Downloads` on the disk as the user's own.
    #[test]
    fn a_personal_name_deeper_down_claims_nothing() {
        assert_eq!(classify("/users/example/code/Downloads", true), None);
        assert_eq!(classify("/opt/Documents", true), None);
    }

    /// The rule that stops an app bundle swallowing its own build output.
    #[test]
    fn development_outranks_the_directory_it_sits_in() {
        assert_eq!(
            classify("/users/example/Documents/app/node_modules", true),
            Some(StorageCategory::Development)
        );
        assert_eq!(
            classify("/users/example/Library/Developer/Xcode/DerivedData", true),
            Some(StorageCategory::Development)
        );
    }

    /// A file called `something.app` is not an application bundle.
    #[test]
    fn only_a_directory_can_be_an_app_bundle() {
        assert_eq!(classify("/Downloads/Example.app", false), None);
    }

    fn sample_tree() -> Tree {
        let mut tree = Tree::new("/users/example");
        let root = tree.push(None, Node::directory("example"));

        let downloads = tree.push(Some(root), Node::directory("Downloads"));
        tree.push(Some(downloads), Node::file("installer.dmg", 4_000));

        let documents = tree.push(Some(root), Node::directory("Documents"));
        tree.push(Some(documents), Node::file("notes.md", 1_000));
        let project = tree.push(Some(documents), Node::directory("project"));
        let modules = tree.push(Some(project), Node::directory("node_modules"));
        tree.push(Some(modules), Node::file("dep.js", 8_000));

        let library = tree.push(Some(root), Node::directory("Library"));
        tree.push(Some(library), Node::file("state.plist", 2_000));

        tree.push(Some(root), Node::file("scratch.txt", 500));
        tree.rollup();
        tree
    }

    /// The property the whole design rests on: every byte lands in exactly one
    /// category. Without it the stacked bar shows more than the disk holds.
    #[test]
    fn the_categories_partition_the_scan() {
        let tree = sample_tree();
        let breakdown = breakdown(1, &tree, &home(), None);

        let summed: u64 = breakdown
            .categories
            .iter()
            .map(|summary| summary.total_bytes)
            .sum();
        let root = tree
            .get(tree.root().expect("a root"))
            .expect("the root node");

        assert_eq!(summed, root.total_bytes);
        assert_eq!(breakdown.scanned_bytes, root.total_bytes);
    }

    #[test]
    fn nested_build_output_leaves_the_personal_total() {
        let tree = sample_tree();
        let breakdown = breakdown(1, &tree, &home(), None);
        let of = |category: StorageCategory| {
            breakdown
                .categories
                .iter()
                .find(|summary| summary.category == category)
                .expect("every category is reported")
        };

        // Documents holds notes.md; the node_modules under it counted elsewhere.
        assert_eq!(of(StorageCategory::PersonalFiles).total_bytes, 5_000);
        assert_eq!(of(StorageCategory::Development).total_bytes, 8_000);
        assert_eq!(of(StorageCategory::Apps).total_bytes, 2_000);
        assert_eq!(of(StorageCategory::Other).total_bytes, 500);
    }

    /// A consumer's size is the part it accounts for in its own category, not
    /// its size on disk — otherwise Documents would report the build output it
    /// does not own.
    #[test]
    fn a_consumer_reports_only_what_it_accounts_for() {
        let tree = sample_tree();
        let breakdown = breakdown(1, &tree, &home(), None);
        let personal = breakdown
            .categories
            .iter()
            .find(|summary| summary.category == StorageCategory::PersonalFiles)
            .expect("personal files are reported");

        let documents = personal
            .consumers
            .iter()
            .find(|consumer| consumer.name == "Documents")
            .expect("Documents is a consumer");
        assert_eq!(documents.total_bytes, 1_000);

        let development = breakdown
            .categories
            .iter()
            .find(|summary| summary.category == StorageCategory::Development)
            .expect("development is reported");
        assert_eq!(development.consumers[0].name, "node_modules");
        assert_eq!(development.consumers[0].total_bytes, 8_000);
    }

    /// A consumer's bytes come from its whole run of non-claiming descendants,
    /// so its completeness has to come from the same set. Reading the flag off
    /// the claiming node alone reports an understated total as exact, which is
    /// the one direction a size on this screen must not be wrong in quietly.
    #[test]
    fn an_unreadable_descendant_makes_its_consumer_a_lower_bound() {
        let mut tree = Tree::new("/users/example");
        let root = tree.push(None, Node::directory("example"));
        let downloads = tree.push(Some(root), Node::directory("Downloads"));
        tree.push(Some(downloads), Node::file("installer.dmg", 4_000));
        let mut locked = Node::directory("locked");
        locked.read_error = true;
        tree.push(Some(downloads), locked);
        tree.rollup();

        // The claiming node itself read cleanly, so its own flag says nothing.
        assert!(!tree
            .get(downloads)
            .expect("the node exists")
            .size_is_partial());

        let breakdown = breakdown(1, &tree, &home(), None);
        let consumer = breakdown
            .categories
            .iter()
            .find(|summary| summary.category == StorageCategory::PersonalFiles)
            .expect("personal files are reported")
            .consumers
            .iter()
            .find(|consumer| consumer.name == "Downloads")
            .expect("Downloads is a consumer");

        assert!(consumer.size_is_partial, "the total is a lower bound");
        assert_eq!(consumer.total_bytes, 4_000);
    }

    /// The other half of the same rule: an error under a subtree that claimed a
    /// different category belongs to that category's row, not to this one.
    #[test]
    fn an_error_under_another_categorys_claim_stays_with_that_category() {
        let mut tree = Tree::new("/users/example");
        let root = tree.push(None, Node::directory("example"));
        let documents = tree.push(Some(root), Node::directory("Documents"));
        tree.push(Some(documents), Node::file("notes.md", 1_000));
        let modules = tree.push(Some(documents), Node::directory("node_modules"));
        let mut locked = Node::file("dep.js", 8_000);
        locked.read_error = true;
        tree.push(Some(modules), locked);
        tree.rollup();

        let breakdown = breakdown(1, &tree, &home(), None);
        let consumer_of = |category: StorageCategory, name: &str| {
            breakdown
                .categories
                .iter()
                .find(|summary| summary.category == category)
                .expect("the category is reported")
                .consumers
                .iter()
                .find(|consumer| consumer.name == name)
                .cloned()
                .expect("the consumer is reported")
        };

        assert!(consumer_of(StorageCategory::Development, "node_modules").size_is_partial);
        assert!(!consumer_of(StorageCategory::PersonalFiles, "Documents").size_is_partial);
    }

    /// A file can be the biggest thing in a category, and browsing "into" it
    /// would show an empty directory. Each consumer therefore carries where to
    /// open, and a direct child of the root opens the root, which the window
    /// addresses as null rather than by id.
    #[test]
    fn a_file_consumer_reports_the_directory_that_holds_it() {
        let mut tree = Tree::new("/users/example");
        let root = tree.push(None, Node::directory("example"));
        tree.push(Some(root), Node::file("scratch.iso", 9_000));
        let downloads = tree.push(Some(root), Node::directory("Downloads"));
        tree.push(Some(downloads), Node::file("installer.dmg", 4_000));
        tree.rollup();

        let breakdown = breakdown(1, &tree, &home(), None);
        let other = breakdown
            .categories
            .iter()
            .find(|summary| summary.category == StorageCategory::Other)
            .expect("other is reported");

        let file = &other.consumers[0];
        assert_eq!(file.name, "scratch.iso");
        assert!(!file.is_dir);
        assert_eq!(file.parent_id, None, "its parent is the scan root");

        let personal = breakdown
            .categories
            .iter()
            .find(|summary| summary.category == StorageCategory::PersonalFiles)
            .expect("personal files are reported");
        assert!(personal.consumers[0].is_dir);
    }

    /// A bundle's size is not what the application costs, and the footprint that
    /// says so is measured after this cut. So a small bundle ranked below the
    /// size cap has to survive it — otherwise the biggest application on the
    /// disk can never reach the list at all.
    #[test]
    fn a_small_bundle_survives_the_size_cap() {
        let mut tree = Tree::new("/Applications");
        let root = tree.push(None, Node::directory("Applications"));
        // Twelve bundles, descending, so the smallest three rank past the cap.
        for index in 0..12u64 {
            let bundle = tree.push(Some(root), Node::directory(format!("App{index:02}.app")));
            tree.push(
                Some(bundle),
                Node::file("binary", 1_000_000 - index * 1_000),
            );
        }
        tree.rollup();

        let breakdown = breakdown(1, &tree, &home(), None);
        let apps = breakdown
            .categories
            .iter()
            .find(|summary| summary.category == StorageCategory::Apps)
            .expect("apps are reported");

        assert!(
            apps.consumers.len() > MAX_CONSUMERS,
            "bundles ride past the ranked cap: {}",
            apps.consumers.len()
        );
        assert!(
            apps.consumers.iter().any(|row| row.name == "App11.app"),
            "the smallest bundle is still a footprint candidate"
        );
        assert!(apps.consumers.len() <= MAX_BUNDLE_CANDIDATES);
        // Order is untouched: the ranked rows still come first.
        assert_eq!(apps.consumers[0].name, "App00.app");
    }

    /// Only bundles get that exemption. Anything else is measured by the size
    /// already reported, so the cap is the whole truth about it.
    #[test]
    fn an_ordinary_directory_does_not_survive_the_size_cap() {
        let mut tree = Tree::new("/users/example");
        let root = tree.push(None, Node::directory("example"));
        for index in 0..12u64 {
            let dir = tree.push(Some(root), Node::directory(format!("dir{index:02}")));
            tree.push(Some(dir), Node::file("blob", 1_000_000 - index * 1_000));
        }
        tree.rollup();

        let breakdown = breakdown(1, &tree, &home(), None);
        let other = breakdown
            .categories
            .iter()
            .find(|summary| summary.category == StorageCategory::Other)
            .expect("other is reported");

        assert_eq!(other.consumers.len(), MAX_CONSUMERS);
        assert!(!other.consumers.iter().any(|row| row.name == "dir11"));
    }

    /// Every category is always reported, so the dashboard's cards do not move
    /// about as a scan finds or misses one of them.
    #[test]
    fn all_five_categories_are_always_present() {
        let mut tree = Tree::new("/tmp/empty");
        tree.push(None, Node::directory("empty"));
        tree.rollup();

        let breakdown = breakdown(2, &tree, &home(), None);
        let reported: Vec<_> = breakdown
            .categories
            .iter()
            .map(|summary| summary.category)
            .collect();

        assert_eq!(reported, StorageCategory::ALL.to_vec());
        assert_eq!(breakdown.scanned_bytes, 0);
        assert!(breakdown.categories.iter().all(|s| s.share == 0.0));
    }
}
