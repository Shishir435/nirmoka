//! What an application costs.
//!
//! The scan tree knows that `Docker.app` is 1.8 GB, because that is the
//! directory it is. A user who is out of disk space means something larger by
//! "Docker": the bundle plus everything macOS lets the application keep
//! elsewhere, which is `~/Library` and is filed under a bundle identifier
//! rather than under a name.
//!
//! So a footprint is assembled from one key. `CFBundleIdentifier` comes out of
//! `Contents/Info.plist`; Mole already publishes it for applications it can
//! address, and this closes the gap for everything reached by a scan instead.
//! Nothing here matches on an application's *name* — two apps called Preview
//! is a naming collision, and `com.apple.Preview` is an identity.
//!
//! Components are named by where they live, never by what the application keeps
//! there. See [ADR 0028]. `Docker.raw` is 22 GB of something, and this module
//! reports 22 GB and its path rather than guessing at the shape inside.
//!
//! [ADR 0028]: ../../../docs/adr/0028-an-applications-footprint-is-what-the-filesystem-says.md

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use nirmoka_core::{NodeId, Tree};

use crate::dto::{AppFootprint, FootprintPath, FootprintSource, StorageComponent};
use crate::state::ScanId;

/// Entries a single filesystem walk will visit before it gives up.
///
/// A footprint is drawn on screen while the user waits. An application support
/// directory holding a million files is rare and real, and the honest answer
/// there is a partial size with the fact stated, not a frozen window.
const WALK_BUDGET: u32 = 400_000;

/// Where macOS lets an application keep things, and what to call each one.
///
/// The `label` is the component a path is reported under, so several locations
/// can share one — caches, web storage, and WebKit's own store are all cache,
/// and splitting them on screen would be filing detail presented as insight.
/// Every individual path survives into [`StorageComponent::paths`] regardless,
/// because the component total is a summary and the paths are the evidence.
const LOCATIONS: &[Location] = &[
    Location {
        label: "Containers",
        directory: "Containers",
        suffix: "",
    },
    Location {
        label: "Containers",
        directory: "Application Scripts",
        suffix: "",
    },
    Location {
        label: "Application Support",
        directory: "Application Support",
        suffix: "",
    },
    Location {
        label: "Caches",
        directory: "Caches",
        suffix: "",
    },
    Location {
        label: "Caches",
        directory: "HTTPStorages",
        suffix: "",
    },
    Location {
        label: "Caches",
        directory: "WebKit",
        suffix: "",
    },
    Location {
        label: "Logs",
        directory: "Logs",
        suffix: "",
    },
    Location {
        label: "Preferences",
        directory: "Preferences",
        suffix: ".plist",
    },
    Location {
        label: "Preferences",
        directory: "Saved Application State",
        suffix: ".savedState",
    },
];

struct Location {
    label: &'static str,
    directory: &'static str,
    suffix: &'static str,
}

/// Read `CFBundleIdentifier` out of an application bundle.
///
/// Returns `None` for anything that is not a readable bundle: a directory that
/// merely ends in `.app`, a plist with no identifier, a plist this process
/// cannot open. A missing identifier is a footprint of one component, which is
/// a worse answer than Mole's and a true one.
pub fn bundle_id(app_path: &Path) -> Option<String> {
    let plist = plist::Value::from_file(app_path.join("Contents").join("Info.plist")).ok()?;
    let identifier = plist
        .as_dictionary()?
        .get("CFBundleIdentifier")?
        .as_string()?
        .trim()
        .to_string();
    (!identifier.is_empty()).then_some(identifier)
}

/// Every path under `~/Library` that carries this bundle identifier and exists.
///
/// Existence is checked here rather than reported as a zero later, so a
/// component that never appears on screen is one the application does not have
/// rather than one that happens to be empty.
pub fn library_paths(home: &Path, bundle_id: &str) -> Vec<(&'static str, PathBuf)> {
    let library = home.join("Library");
    LOCATIONS
        .iter()
        .map(|location| {
            let leaf = format!("{bundle_id}{}", location.suffix);
            (location.label, library.join(location.directory).join(leaf))
        })
        .filter(|(_, path)| path.symlink_metadata().is_ok())
        .collect()
}

/// The size of `path` according to a scan that already walked it.
///
/// Free where it works: if the user scanned `~`, every Library path is already
/// in the tree and this is a descent by name. Returns `None` when the path was
/// outside the scanned set, which is the common case for a scan of
/// `/Applications` and the reason [`walk`] exists.
///
/// The second value is whether the subtree was read in full. It is not
/// `Node::size_is_partial`, which is a fact about one node: `rollup` sums
/// `total_bytes` up the tree and propagates nothing else, so a directory whose
/// own entry read cleanly reports a clean flag over an unreadable descendant.
/// A footprint built from that would understate an application's storage while
/// claiming to be complete, which is the one direction the number must not be
/// wrong in silently.
pub fn size_from_tree(tree: &Tree, path: &Path) -> Option<(u64, bool)> {
    let relative = path.strip_prefix(tree.root_path()).ok()?;
    let mut current = tree.root()?;

    for part in relative.components() {
        let Component::Normal(name) = part else {
            continue;
        };
        let name = name.to_str()?;
        current = tree
            .children_of(current)
            .iter()
            .copied()
            .find(|child| tree.get(*child).is_ok_and(|node| node.name == name))?;
    }

    let bytes = tree.get(current).ok()?.total_bytes;
    Some((bytes, subtree_is_complete(tree, current)))
}

/// Whether every node under `id`, and `id` itself, was read in full.
///
/// An in-memory descent over an arena that is already resident: no filesystem
/// access, and the subtree of one `~/Library` entry rather than of the scan.
fn subtree_is_complete(tree: &Tree, id: NodeId) -> bool {
    let mut pending = vec![id];
    while let Some(current) = pending.pop() {
        let Ok(node) = tree.get(current) else {
            return false;
        };
        if node.size_is_partial() {
            return false;
        }
        pending.extend(tree.children_of(current).iter().copied());
    }
    true
}

/// Disk usage of `path`, by walking it.
///
/// Blocks rather than apparent bytes, matching ADR 0009 and the scanners: a
/// sparse 64 GB disk image that occupies 22 GB is 22 GB in every other number
/// this product shows. Symlinks are counted as the links they are and never
/// followed, so a link into `/Applications` cannot make a cache look enormous
/// or send the walk in a circle.
///
/// How much of a subtree a walk actually reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkOutcome {
    /// Everything under the path was counted.
    Complete,
    /// A locked or vanished entry was skipped. The total is a lower bound, and
    /// a useful one: the rest of the subtree was counted normally.
    Partial,
    /// The entry budget ran out. An unknown fraction was counted, so the total
    /// is not a bound in either direction and is not offered as a number.
    Exhausted,
}

/// Returns the bytes found and how much of the subtree they represent.
///
/// An unreadable subdirectory lowers the total rather than failing the call —
/// the same treatment a scan gives a permission error — and downgrades the
/// outcome, because a total missing a locked directory has to say so.
///
/// A path that does not exist is `(0, Complete)`: nothing there is nothing
/// missed.
pub fn walk(path: &Path) -> (u64, WalkOutcome) {
    if path.symlink_metadata().is_err() {
        return (0, WalkOutcome::Complete);
    }

    let mut total = 0;
    let mut complete = true;
    let mut budget = WALK_BUDGET;
    // An explicit stack: `~/Library` nests deeply enough that recursion here
    // would be a stack depth that depends on the user's disk.
    let mut pending = vec![path.to_path_buf()];

    while let Some(current) = pending.pop() {
        let Ok(metadata) = current.symlink_metadata() else {
            complete = false;
            continue;
        };
        if budget == 0 {
            return (total, WalkOutcome::Exhausted);
        }
        budget -= 1;
        total += disk_bytes(&metadata);

        if metadata.is_dir() {
            let Ok(entries) = std::fs::read_dir(&current) else {
                // Permission denied on a cache directory is the common case, and
                // the bytes behind it are real whether or not they can be read.
                complete = false;
                continue;
            };
            pending.extend(entries.flatten().map(|entry| entry.path()));
        }
    }

    (
        total,
        if complete {
            WalkOutcome::Complete
        } else {
            WalkOutcome::Partial
        },
    )
}

#[cfg(unix)]
fn disk_bytes(metadata: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    // 512 is the unit `st_blocks` is defined in, and is not the block size of
    // the filesystem.
    metadata.blocks() * 512
}

#[cfg(not(unix))]
fn disk_bytes(metadata: &std::fs::Metadata) -> u64 {
    metadata.len()
}

/// When macOS last recorded the application being opened.
///
/// Spotlight keeps this and nothing else does, so there is no cross-platform
/// answer and no answer at all on a machine with indexing off — `None` in both
/// cases, which the window renders as an absent line rather than as "never".
///
/// `mdls` is a subprocess, so this is called once for the application being
/// inspected and never in a loop over a list.
pub fn last_used_ms(path: &Path) -> Option<i64> {
    if std::env::consts::OS != "macos" {
        return None;
    }
    let output = std::process::Command::new("mdls")
        .arg("-raw")
        .arg("-name")
        .arg("kMDItemLastUsedDate")
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_spotlight_date(String::from_utf8_lossy(&output.stdout).trim())
}

/// `2026-08-19 10:22:33 +0000`, which is the only shape `mdls -raw` prints for
/// a date, and `(null)` when the attribute is unset.
///
/// Parsed by hand rather than by adding a date library for one field. The
/// offset is read but the values are already UTC in practice; anything that
/// does not match this shape is `None` instead of a guess.
fn parse_spotlight_date(raw: &str) -> Option<i64> {
    let (date, rest) = raw.split_once(' ')?;
    let time = rest.split(' ').next()?;
    let mut date = date.split('-');
    let year: i64 = date.next()?.parse().ok()?;
    let month: i64 = date.next()?.parse().ok()?;
    let day: i64 = date.next()?.parse().ok()?;
    let mut time = time.split(':');
    let hour: i64 = time.next()?.parse().ok()?;
    let minute: i64 = time.next()?.parse().ok()?;
    let second: i64 = time.next()?.split('.').next()?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 {
        return None;
    }

    let seconds = days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second;
    Some(seconds * 1000)
}

/// Days between 1970-01-01 and a civil date, by Howard Hinnant's algorithm.
///
/// Fifteen lines against a dependency that would arrive with a timezone
/// database for one timestamp that is already UTC.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Directories a vendor-named folder can appear in.
///
/// Preferences and Saved Application State are absent on purpose: both are
/// keyed by identifier by macOS itself, so a vendor-named entry there would be
/// a coincidence rather than a match.
const VENDOR_LOCATIONS: &[&str] = &[
    "Application Support",
    "Caches",
    "Logs",
    "HTTPStorages",
    "WebKit",
    "Containers",
];

/// Bundle identifier segments that name nobody.
///
/// `com.google.Chrome` yields a vendor and a product; the `com` yields a
/// directory that would match half the disk if such a folder existed.
const GENERIC_SEGMENTS: &[&str] = &[
    "com", "org", "net", "io", "dev", "co", "me", "app", "apps", "desktop", "mac", "macos", "osx",
    "inc", "ltd", "llc", "gmbh", "software", "labs", "team", "client", "www",
];

/// The label every vendor-matched path is reported under.
///
/// One label, never merged into the identifier-matched components, because the
/// two are known in different ways and the window styles them differently.
pub const RELATED_LABEL: &str = "Possibly related";

/// Names this application might have filed a directory under.
///
/// Non-sandboxed applications do not use their bundle identifier for storage.
/// Chrome's 6 GB is in `~/Library/Application Support/Google`, Firefox's in
/// `.../Firefox` — vendor and product names, which are exactly the words a
/// bundle identifier is made of. So the candidates are derived from the
/// identifier and the application's own name rather than read from a table
/// somebody has to maintain and nobody can relicense.
///
/// This is a guess, and the caller keeps it separate from what is known. See
/// [ADR 0028].
///
/// [ADR 0028]: ../../../docs/adr/0028-an-applications-footprint-is-what-the-filesystem-says.md
pub fn vendor_candidates(bundle_id: Option<&str>, name: &str) -> Vec<String> {
    let mut candidates: Vec<String> = Vec::new();
    let mut push = |candidate: &str| {
        let candidate = candidate.trim();
        // Two characters is an abbreviation that collides with everything.
        if candidate.len() < 3
            || GENERIC_SEGMENTS.contains(&candidate.to_ascii_lowercase().as_str())
        {
            return;
        }
        let lowered = candidate.to_ascii_lowercase();
        if !candidates.contains(&lowered) {
            candidates.push(lowered);
        }
    };

    for segment in bundle_id.unwrap_or_default().split('.') {
        push(segment);
    }
    push(name);
    // "Google Chrome" is also filed as "Google". The last word is the product
    // and is already a candidate via the identifier in the usual case.
    if let Some(first) = name.split_whitespace().next() {
        push(first);
    }

    candidates
}

/// Existing directories matching any candidate, excluding what is already known.
///
/// Matched case-insensitively against the directory listing rather than by
/// joining a guessed name, because the filesystem's capitalisation is the
/// authority: the identifier says `google` and the folder says `Google`.
pub fn related_paths(home: &Path, candidates: &[String], known: &[PathBuf]) -> Vec<PathBuf> {
    if candidates.is_empty() {
        return Vec::new();
    }
    let library = home.join("Library");
    let mut found = Vec::new();

    for location in VENDOR_LOCATIONS {
        let Ok(entries) = std::fs::read_dir(library.join(location)) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let lowered = name.to_ascii_lowercase();
            // A directory named for the identifier is already a known
            // component; reporting it twice would double the footprint.
            if !candidates.contains(&lowered) {
                continue;
            }
            let path = entry.path();
            if !known.contains(&path) {
                found.push(path);
            }
        }
    }

    found.sort();
    found
}

/// Everything about a footprint that the tree alone can answer.
///
/// The tree lives behind a lock and a walk of `~/Library` can take seconds, so
/// the two are deliberately separate: [`plan`] holds the lock and touches no
/// directory, [`resolve`] holds nothing and does the walking. Holding a scan of
/// two million nodes locked while a cache directory is counted would stall
/// every other command behind it, including the one that cancels this.
#[derive(Debug)]
pub struct FootprintPlan {
    scan_id: ScanId,
    node_id: u32,
    name: String,
    path: PathBuf,
    bundle_id: Option<String>,
    bundle_bytes: u64,
    bundle_complete: bool,
    pending: Vec<PlannedPath>,
}

#[derive(Debug)]
struct PlannedPath {
    label: &'static str,
    path: PathBuf,
    /// `Some(bytes, complete)` when the scan already walked this path, which is
    /// the free case. `complete` is about the whole subtree — see
    /// [`size_from_tree`].
    known: Option<(u64, bool)>,
}

/// Read what the tree knows about the application at `node_id`.
///
/// `home` is passed in rather than looked up so the assembly is testable
/// against a directory this process created.
pub fn plan(
    scan_id: ScanId,
    node_id: NodeId,
    tree: &Tree,
    home: &Path,
) -> Result<FootprintPlan, String> {
    let node = tree.get(node_id).map_err(|error| error.to_string())?;
    let path = tree.path_of(node_id).map_err(|error| error.to_string())?;
    let identifier = bundle_id(&path);

    let name = node.name.trim_end_matches(".app").to_string();
    let mut pending: Vec<PlannedPath> = identifier
        .as_deref()
        .map(|identifier| {
            library_paths(home, identifier)
                .into_iter()
                .map(|(label, path)| PlannedPath {
                    known: size_from_tree(tree, &path),
                    label,
                    path,
                })
                .collect()
        })
        .unwrap_or_default();

    // Vendor-named directories, which most non-sandboxed applications use and
    // no identifier finds. Kept apart from the paths above all the way to the
    // screen: this is a guess, and `known` stops it counting anything twice.
    let known: Vec<PathBuf> = pending.iter().map(|planned| planned.path.clone()).collect();
    let candidates = vendor_candidates(identifier.as_deref(), &name);
    pending.extend(
        related_paths(home, &candidates, &known)
            .into_iter()
            .map(|path| PlannedPath {
                known: size_from_tree(tree, &path),
                label: RELATED_LABEL,
                path,
            }),
    );

    Ok(FootprintPlan {
        scan_id,
        node_id: node_id.raw(),
        name,
        bundle_bytes: node.total_bytes,
        // The subtree, not the node. `size_is_partial` on a directory says its
        // own entry read cleanly, which it does while an unreadable file sits
        // inside it — see `size_from_tree`.
        bundle_complete: subtree_is_complete(tree, node_id),
        bundle_id: identifier,
        path,
        pending,
    })
}

/// Walk whatever the scan did not cover, and assemble the answer.
///
/// Runs off the main thread. Nothing here holds a lock, which is the point of
/// the split — see [`FootprintPlan`].
pub fn resolve(plan: FootprintPlan) -> AppFootprint {
    // The bundle is the one component that needs no identifier, and the scan
    // walked it by definition: this is the node the user clicked.
    let mut components = vec![StorageComponent {
        label: "Application".to_string(),
        total_bytes: plan.bundle_bytes,
        complete: plan.bundle_complete,
        certain: true,
        paths: vec![FootprintPath {
            path: plan.path.display().to_string(),
            total_bytes: Some(plan.bundle_bytes),
            complete: plan.bundle_complete,
            source: FootprintSource::Scan,
        }],
    }];

    // Grouped by label so the three cache directories arrive as one component,
    // and ordered by LOCATIONS rather than alphabetically so that Containers
    // precedes Preferences for the same reason it does in the declaration.
    let mut grouped: BTreeMap<&'static str, Vec<FootprintPath>> = BTreeMap::new();
    for planned in plan.pending {
        grouped
            .entry(planned.label)
            .or_default()
            .push(measure(planned));
    }

    for location in LOCATIONS {
        let Some(paths) = grouped.remove(location.label) else {
            continue;
        };
        components.push(StorageComponent {
            label: location.label.to_string(),
            total_bytes: paths.iter().filter_map(|path| path.total_bytes).sum(),
            complete: paths
                .iter()
                .all(|path| path.total_bytes.is_some() && path.complete),
            certain: true,
            paths,
        });
    }

    // Everything above is attributed by identifier and sums into the headline
    // number. This does not: it is matched by name, it can include a sibling
    // application's data, and a total that quietly absorbed it would be
    // confident about a guess.
    let related = grouped.remove(RELATED_LABEL).unwrap_or_default();
    let related_bytes = related.iter().filter_map(|path| path.total_bytes).sum();
    let total_bytes = components
        .iter()
        .map(|component| component.total_bytes)
        .sum();
    if !related.is_empty() {
        components.push(StorageComponent {
            label: RELATED_LABEL.to_string(),
            total_bytes: related_bytes,
            complete: related
                .iter()
                .all(|path| path.total_bytes.is_some() && path.complete),
            certain: false,
            paths: related,
        });
    }

    let unmeasured = components
        .iter()
        .flat_map(|component| &component.paths)
        .filter(|path| path.total_bytes.is_none())
        .count();

    AppFootprint {
        scan_id: plan.scan_id,
        node_id: plan.node_id,
        name: plan.name,
        bundle_id: plan.bundle_id,
        total_bytes,
        related_bytes,
        unmeasured_paths: unmeasured.min(u32::MAX as usize) as u32,
        last_used_ms: last_used_ms(&plan.path),
        components,
        path: plan.path.display().to_string(),
    }
}

/// Size one planned path, preferring the scan that already walked it.
fn measure(planned: PlannedPath) -> FootprintPath {
    if let Some((bytes, complete)) = planned.known {
        return FootprintPath {
            path: planned.path.display().to_string(),
            total_bytes: Some(bytes),
            complete,
            source: FootprintSource::Scan,
        };
    }

    // A walk that ran out of budget counted an unknown fraction, so it offers no
    // number. A walk that merely hit a locked directory counted everything else,
    // and that is a lower bound worth showing — flagged, not withheld.
    let (bytes, outcome) = walk(&planned.path);
    FootprintPath {
        path: planned.path.display().to_string(),
        total_bytes: (outcome != WalkOutcome::Exhausted).then_some(bytes),
        complete: outcome == WalkOutcome::Complete,
        source: if outcome == WalkOutcome::Exhausted {
            FootprintSource::Unavailable
        } else {
            FootprintSource::Filesystem
        },
    }
}

#[cfg(test)]
mod tests {
    use nirmoka_core::Node;

    use super::*;

    /// A directory this test owns, so `home` is a real one without being the
    /// machine's.
    fn scratch(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "nirmoka-attribution-{}-{label}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("the temp directory is writable");
        path
    }

    fn write(path: &Path, bytes: usize) {
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("created");
        std::fs::write(path, vec![b'x'; bytes]).expect("written");
    }

    #[test]
    fn a_bundle_identifier_is_read_from_an_xml_info_plist() {
        let home = scratch("plist");
        let app = home.join("Example.app");
        write(&app.join("Contents").join("Info.plist"), 0);
        std::fs::write(
            app.join("Contents").join("Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>com.example.desktop</string>
</dict></plist>"#,
        )
        .expect("written");

        assert_eq!(bundle_id(&app).as_deref(), Some("com.example.desktop"));
    }

    /// A directory named `.app` is not a bundle, and guessing an identifier
    /// from the name is what this module exists to avoid.
    #[test]
    fn a_directory_that_is_not_a_bundle_has_no_identifier() {
        let home = scratch("not-a-bundle");
        std::fs::create_dir_all(home.join("Example.app")).expect("created");

        assert_eq!(bundle_id(&home.join("Example.app")), None);
    }

    #[test]
    fn only_library_paths_that_exist_are_reported() {
        let home = scratch("library");
        write(
            &home
                .join("Library")
                .join("Caches")
                .join("com.example.desktop")
                .join("blob"),
            16,
        );
        std::fs::create_dir_all(
            home.join("Library")
                .join("Containers")
                .join("com.example.desktop"),
        )
        .expect("created");

        let found = library_paths(&home, "com.example.desktop");
        let labels: Vec<_> = found.iter().map(|(label, _)| *label).collect();

        assert_eq!(labels, vec!["Containers", "Caches"]);
        assert!(!found.iter().any(|(label, _)| *label == "Logs"));
    }

    /// The identifier is the key. An application support directory belonging to
    /// a different app must not be attributed by resembling the name.
    #[test]
    fn another_applications_directory_is_not_attributed() {
        let home = scratch("neighbour");
        std::fs::create_dir_all(
            home.join("Library")
                .join("Caches")
                .join("com.example.other"),
        )
        .expect("created");

        assert!(library_paths(&home, "com.example.desktop").is_empty());
    }

    #[test]
    fn vendor_candidates_come_from_the_identifier_and_the_name() {
        assert_eq!(
            vendor_candidates(Some("com.google.Chrome"), "Google Chrome"),
            vec!["google", "chrome", "google chrome"]
        );
        assert_eq!(
            vendor_candidates(Some("org.mozilla.firefox"), "Firefox"),
            vec!["mozilla", "firefox"]
        );
    }

    /// `com` would match a directory named for the top-level domain, and a
    /// two-letter segment collides with everything. Neither names a vendor.
    #[test]
    fn generic_and_tiny_segments_are_not_candidates() {
        let candidates = vendor_candidates(Some("com.io.ab.desktop"), "Ab");

        assert!(candidates.is_empty(), "{candidates:?}");
    }

    /// The case the whole vendor path exists for: Chrome keeps 6 GB in a
    /// folder no bundle identifier finds.
    #[test]
    fn a_vendor_named_directory_is_found_where_the_identifier_finds_nothing() {
        let home = scratch("vendor");
        write(
            &home
                .join("Library")
                .join("Application Support")
                .join("Google")
                .join("Default"),
            32,
        );

        let candidates = vendor_candidates(Some("com.google.Chrome"), "Google Chrome");
        let found = related_paths(&home, &candidates, &[]);

        assert_eq!(
            found,
            vec![home
                .join("Library")
                .join("Application Support")
                .join("Google")]
        );
    }

    /// A directory already attributed by identifier must not also arrive as a
    /// guess, or the footprint counts it twice.
    #[test]
    fn a_path_already_known_is_not_reported_again() {
        let home = scratch("vendor-dedupe");
        let known = home
            .join("Library")
            .join("Application Support")
            .join("Firefox");
        std::fs::create_dir_all(&known).expect("created");

        let candidates = vendor_candidates(Some("org.mozilla.firefox"), "Firefox");

        assert!(related_paths(&home, &candidates, &[known]).is_empty());
    }

    /// Vendor bytes are a guess, so they sit in their own component and stay
    /// out of the headline total. This is the ADR 0028 decision, under test.
    #[test]
    fn vendor_bytes_are_reported_apart_from_the_total() {
        let home = scratch("vendor-total");
        let apps = home.join("Applications");
        let app = apps.join("Example Browser.app");
        std::fs::create_dir_all(app.join("Contents")).expect("created");
        std::fs::write(
            app.join("Contents").join("Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>com.example.browser</string>
</dict></plist>"#,
        )
        .expect("written");
        // Vendor-named, which no identifier would find.
        write(
            &home
                .join("Library")
                .join("Application Support")
                .join("Example")
                .join("profile"),
            4096,
        );

        let mut tree = Tree::new(&apps);
        let root = tree.push(None, Node::directory("Applications"));
        let bundle = tree.push(Some(root), Node::directory("Example Browser.app"));
        tree.push(Some(bundle), Node::file("binary", 1024));
        tree.rollup();

        let footprint = resolve(plan(4, bundle, &tree, &home).expect("the node is in the tree"));

        let related = footprint
            .components
            .iter()
            .find(|component| component.label == RELATED_LABEL)
            .expect("the vendor directory was found");
        assert!(!related.certain);
        assert!(related.total_bytes > 0);

        // The headline is the bundle alone. The guess is beside it, not inside.
        assert_eq!(footprint.total_bytes, 1024);
        assert_eq!(footprint.related_bytes, related.total_bytes);
        assert!(footprint.components[0].certain);
        // And it sorts last, after everything that is actually known.
        assert_eq!(
            footprint.components.last().map(|c| c.label.as_str()),
            Some(RELATED_LABEL)
        );
    }

    #[test]
    fn a_scanned_path_is_sized_from_the_tree_without_touching_the_disk() {
        let mut tree = Tree::new("/scan");
        let root = tree.push(None, Node::directory("scan"));
        let library = tree.push(Some(root), Node::directory("Library"));
        let caches = tree.push(Some(library), Node::directory("Caches"));
        let app = tree.push(Some(caches), Node::directory("com.example.desktop"));
        tree.push(Some(app), Node::file("blob", 2048));
        tree.rollup();

        assert_eq!(
            size_from_tree(&tree, Path::new("/scan/Library/Caches/com.example.desktop")),
            Some((2048, true))
        );
        assert_eq!(size_from_tree(&tree, Path::new("/elsewhere")), None);
        assert_eq!(size_from_tree(&tree, Path::new("/scan/Library/Logs")), None);
    }

    #[test]
    fn a_spotlight_date_becomes_epoch_milliseconds() {
        // 2026-08-19T10:22:33Z, checked against `date -u -j -f`.
        assert_eq!(
            parse_spotlight_date("2026-08-19 10:22:33 +0000"),
            Some(1_787_134_953_000)
        );
        assert_eq!(parse_spotlight_date("1970-01-01 00:00:00 +0000"), Some(0));
    }

    /// Spotlight prints `(null)` for an application it has no record of
    /// opening, and that is an absent line rather than a date at the epoch.
    #[test]
    fn a_date_that_is_not_a_date_is_none() {
        for raw in ["(null)", "", "not a date", "2026-13-01 00:00:00 +0000"] {
            assert_eq!(parse_spotlight_date(raw), None, "{raw}");
        }
    }

    /// `rollup` sums bytes and propagates nothing else, so a directory whose own
    /// entry read cleanly reports a clean flag over an unreadable descendant.
    /// Reading the flag off the node alone would call an understated total
    /// complete, which is the one way this number must not be wrong quietly.
    #[test]
    fn an_unreadable_descendant_makes_the_scanned_size_a_lower_bound() {
        let mut tree = Tree::new("/scan");
        let root = tree.push(None, Node::directory("scan"));
        let library = tree.push(Some(root), Node::directory("Library"));
        let caches = tree.push(Some(library), Node::directory("Caches"));
        let app = tree.push(Some(caches), Node::directory("com.example.desktop"));
        tree.push(Some(app), Node::file("blob", 2048));
        let mut locked = Node::directory("locked");
        locked.read_error = true;
        tree.push(Some(app), locked);
        tree.rollup();

        let path = Path::new("/scan/Library/Caches/com.example.desktop");
        // The directory's own node is clean: the flag being read has to be the
        // subtree's, not this one's.
        assert!(!tree.get(app).expect("the node exists").size_is_partial());
        assert_eq!(size_from_tree(&tree, path), Some((2048, false)));
    }

    /// The same fault one level up: the bundle is the node the user clicked, and
    /// an unreadable file inside it does not touch the directory's own flag.
    #[test]
    fn an_unreadable_file_in_the_bundle_is_carried_into_the_footprint() {
        let home = scratch("partial-bundle");
        let apps = home.join("Applications");
        let mut tree = Tree::new(&apps);
        let root = tree.push(None, Node::directory("Applications"));
        let bundle = tree.push(Some(root), Node::directory("Example.app"));
        tree.push(Some(bundle), Node::file("binary", 1024));
        let mut unreadable = Node::file("secret", 0);
        unreadable.read_error = true;
        tree.push(Some(bundle), unreadable);
        tree.rollup();

        let footprint = resolve(plan(2, bundle, &tree, &home).expect("the node is in the tree"));
        let application = &footprint.components[0];

        assert!(!application.complete, "the bundle was not read in full");
        assert!(!application.paths[0].complete);
        // Still counted, and still reported: a lower bound beats no number.
        assert_eq!(application.total_bytes, 1024);
        assert_eq!(footprint.unmeasured_paths, 0);
    }

    #[test]
    fn a_walk_counts_a_directory_tree() {
        let home = scratch("walk");
        write(&home.join("a").join("one"), 4096);
        write(&home.join("a").join("nested").join("two"), 4096);

        let (bytes, outcome) = walk(&home.join("a"));

        assert_eq!(outcome, WalkOutcome::Complete);
        assert!(bytes >= 8192, "counted {bytes}");
    }

    #[test]
    fn a_walk_of_nothing_is_zero_rather_than_an_error() {
        let home = scratch("missing");

        assert_eq!(walk(&home.join("absent")), (0, WalkOutcome::Complete));
    }

    /// The whole assembly, against a home directory this test built: the bundle
    /// from the tree, the Library paths from disk, grouped and totalled.
    #[test]
    fn a_footprint_is_the_bundle_plus_what_carries_its_identifier() {
        let home = scratch("footprint");
        let apps = home.join("Applications");
        let app = apps.join("Example.app");
        std::fs::create_dir_all(app.join("Contents")).expect("created");
        std::fs::write(
            app.join("Contents").join("Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>com.example.desktop</string>
</dict></plist>"#,
        )
        .expect("written");
        write(
            &home
                .join("Library")
                .join("Caches")
                .join("com.example.desktop")
                .join("blob"),
            4096,
        );
        write(
            &home
                .join("Library")
                .join("Logs")
                .join("com.example.desktop")
                .join("log"),
            4096,
        );

        let mut tree = Tree::new(&apps);
        let root = tree.push(None, Node::directory("Applications"));
        let bundle = tree.push(Some(root), Node::directory("Example.app"));
        tree.push(Some(bundle), Node::file("binary", 1024));
        tree.rollup();

        let plan = plan(7, bundle, &tree, &home).expect("the node is in the tree");
        let footprint = resolve(plan);

        assert_eq!(footprint.scan_id, 7);
        assert_eq!(footprint.name, "Example");
        assert_eq!(footprint.bundle_id.as_deref(), Some("com.example.desktop"));
        assert_eq!(footprint.unmeasured_paths, 0);

        let labels: Vec<_> = footprint
            .components
            .iter()
            .map(|component| component.label.as_str())
            .collect();
        assert_eq!(labels, vec!["Application", "Caches", "Logs"]);

        // The bundle came from the tree; the Library paths were not in it.
        assert_eq!(footprint.components[0].total_bytes, 1024);
        assert_eq!(
            footprint.components[0].paths[0].source,
            FootprintSource::Scan
        );
        assert_eq!(
            footprint.components[1].paths[0].source,
            FootprintSource::Filesystem
        );
        assert!(
            footprint.total_bytes > 1024,
            "the footprint is more than the bundle: {}",
            footprint.total_bytes
        );
    }

    /// An application with no identifier still has a footprint. It is the
    /// bundle, and it says so by having one component rather than by being an
    /// error.
    #[test]
    fn without_an_identifier_the_footprint_is_the_bundle_alone() {
        let home = scratch("no-identifier");
        let mut tree = Tree::new(home.join("Applications"));
        let root = tree.push(None, Node::directory("Applications"));
        let bundle = tree.push(Some(root), Node::directory("Example.app"));
        tree.push(Some(bundle), Node::file("binary", 512));
        tree.rollup();

        let plan = plan(1, bundle, &tree, &home).expect("the node is in the tree");
        let footprint = resolve(plan);

        assert_eq!(footprint.bundle_id, None);
        assert_eq!(footprint.components.len(), 1);
        assert_eq!(footprint.total_bytes, 512);
    }
}
