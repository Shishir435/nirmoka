//! `nrmk scan` — the whole stack, headless.
//!
//! This is the command that proves the architecture: a real backend runs as a
//! subprocess, its output is parsed into a `nirmoka-core` tree, and the tree is
//! rendered — with no GUI framework anywhere in the dependency graph. If this
//! works, `core` is genuinely framework-independent. See `docs/roadmap.md`
//! step 5.

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use clap::Args;
use nirmoka_adapter::wire::{self, TreeStats, WireItem, WireSink};
use nirmoka_adapter::{CancelToken, Registry, ScanOptions, TreeSink};
use nirmoka_core::{format_bytes, NodeId, Tree};
use serde::Serialize;

#[derive(Args)]
pub struct ScanArgs {
    /// Directory to scan.
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Emit JSON instead of a table.
    #[arg(long)]
    json: bool,

    /// Levels of children to show. The tree itself is always scanned in full.
    #[arg(long, default_value_t = 1, value_name = "N")]
    depth: usize,

    /// Entries to show per directory; 0 shows all of them.
    #[arg(long, default_value_t = 20, value_name = "N")]
    limit: usize,

    /// Do not cross filesystem boundaries.
    #[arg(short = 'x', long)]
    one_file_system: bool,

    /// Skip directories tagged with CACHEDIR.TAG.
    #[arg(long)]
    exclude_caches: bool,

    /// Skip entries matching a glob. Repeatable.
    #[arg(long, value_name = "PATTERN")]
    exclude: Vec<String>,

    /// Read a recorded export instead of running a backend.
    ///
    /// Lets the parser be exercised against a fixture on a machine with no
    /// backend installed — including CI on Windows.
    #[arg(
        long,
        value_name = "FILE",
        conflicts_with_all = ["one_file_system", "exclude_caches", "exclude"]
    )]
    from_export: Option<PathBuf>,
}

pub fn run(args: ScanArgs, registry: &Registry) -> ExitCode {
    let started = Instant::now();

    let outcome = match &args.from_export {
        Some(file) => from_export(file),
        None => from_backend(&args, registry),
    };

    let scan = match outcome {
        Ok(scan) => scan,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::FAILURE;
        }
    };

    let elapsed = started.elapsed().as_secs_f64();

    if args.json {
        print_json(&args, &scan, elapsed);
    } else {
        print_table(&args, &scan, elapsed);
    }

    ExitCode::SUCCESS
}

struct Scan {
    tree: Tree,
    stats: TreeStats,
    items: u64,
    directories: u64,
    source: String,
}

fn from_export(file: &PathBuf) -> Result<Scan, String> {
    let handle = File::open(file).map_err(|source| format!("{}: {source}", file.display()))?;

    let (tree, wire_stats, stats) = wire::parse_tree(BufReader::new(handle))
        .map_err(|source| format!("{}: {source}", file.display()))?;

    let producer = match (
        &wire_stats.header.program,
        &wire_stats.header.program_version,
    ) {
        (Some(program), Some(version)) => format!("{program} {version}"),
        (Some(program), None) => program.clone(),
        _ => "unknown producer".to_string(),
    };

    Ok(Scan {
        tree,
        stats,
        items: wire_stats.items,
        directories: wire_stats.directories,
        source: format!("{} ({producer})", file.display()),
    })
}

fn from_backend(args: &ScanArgs, registry: &Registry) -> Result<Scan, String> {
    let adapter = registry.first_usable().ok_or_else(|| {
        "no usable backend found. Run `nrmk backends` to see what is installed.".to_string()
    })?;

    let options = ScanOptions {
        one_file_system: args.one_file_system,
        exclude_caches: args.exclude_caches,
        exclude: args.exclude.clone(),
    };

    // Cancellation is wired all the way through, but nothing here presses the
    // button: `nrmk` installs no signal handler, so Ctrl-C kills this process
    // and the backend dies with the pipe. The GUI is what needs a real stop
    // button, and `Adapter::scan` already takes the token it will pass.
    let cancel = CancelToken::new();

    let mut sink = Progress {
        inner: TreeSink::new(),
        items: 0,
    };

    let summary = adapter
        .scan(&args.path, &options, &mut sink, &cancel)
        .map_err(|error| error.to_string())?;

    let stats = sink.inner.stats();

    Ok(Scan {
        tree: sink.inner.finish(),
        stats,
        items: summary.items,
        directories: summary.directories,
        source: match summary.backend_version {
            Some(version) => format!("{} {version}", adapter.display_name()),
            None => adapter.display_name().to_string(),
        },
    })
}

/// Reports progress to stderr during a long scan.
///
/// Also the visible proof that scanning streams: on a home directory these
/// lines appear steadily while the backend is still walking the disk. If the
/// pipeline buffered, they would all arrive at once at the end.
struct Progress {
    inner: TreeSink,
    items: u64,
}

impl Progress {
    const EVERY: u64 = 100_000;

    fn tick(&mut self) {
        self.items += 1;
        if self.items.is_multiple_of(Self::EVERY) {
            eprintln!("  scanned {} entries…", thousands(self.items));
        }
    }
}

impl WireSink for Progress {
    fn open_dir(&mut self, item: WireItem) {
        self.tick();
        self.inner.open_dir(item);
    }

    fn item(&mut self, item: WireItem) {
        self.tick();
        self.inner.item(item);
    }

    fn close_dir(&mut self) {
        self.inner.close_dir();
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn print_table(args: &ScanArgs, scan: &Scan, elapsed: f64) {
    let Some(root) = scan.tree.root() else {
        println!("empty scan");
        return;
    };

    let total = scan
        .tree
        .get(root)
        .map(|node| node.total_bytes)
        .unwrap_or(0);

    println!("ROOT     {}", scan.tree.root_path().display());
    println!("SOURCE   {}", scan.source);
    println!(
        "ITEMS    {} ({} directories) in {elapsed:.2}s",
        thousands(scan.items),
        thousands(scan.directories)
    );
    println!("TOTAL    {}", format_bytes(total));

    for note in notes(&scan.stats) {
        println!("NOTE     {note}");
    }

    println!();
    println!("{:>10}  {:>5}  NAME", "SIZE", "SHARE");
    print_children(&scan.tree, root, total, args.depth, args.limit, 0);
}

fn print_children(
    tree: &Tree,
    parent: NodeId,
    total: u64,
    depth: usize,
    limit: usize,
    indent: usize,
) {
    if depth == 0 {
        return;
    }

    let children = tree.children_by_size(parent);
    let shown = if limit == 0 {
        children.len()
    } else {
        limit.min(children.len())
    };

    for id in &children[..shown] {
        let Ok(node) = tree.get(*id) else { continue };

        let share = if total == 0 {
            0.0
        } else {
            node.total_bytes as f64 / total as f64 * 100.0
        };

        let mut name = node.name.clone();
        if node.is_dir() {
            name.push('/');
        }

        // One column of flags, so a zero that has an explanation never looks
        // like a zero that does not.
        let flag = if node.read_error {
            " !unreadable"
        } else if node.excluded {
            " <excluded"
        } else if node.hardlink {
            " =hardlink"
        } else {
            ""
        };

        println!(
            "{:>10}  {share:>4.0}%  {:indent$}{name}{flag}",
            format_bytes(node.total_bytes),
            "",
            indent = indent
        );

        print_children(tree, *id, total, depth - 1, limit, indent + 2);
    }

    if shown < children.len() {
        println!(
            "{:>10}  {:>5}  {:indent$}… {} more",
            "",
            "",
            "",
            children.len() - shown,
            indent = indent
        );
    }
}

fn notes(stats: &TreeStats) -> Vec<String> {
    let mut notes = Vec::new();

    if stats.read_errors > 0 {
        notes.push(format!(
            "{} could not be read — the total is a lower bound",
            plural(stats.read_errors, "entry", "entries")
        ));
    }
    if stats.excluded > 0 {
        notes.push(format!(
            "{} excluded — the size is unknown, not zero",
            plural(stats.excluded, "entry was", "entries were")
        ));
    }
    if stats.hardlinks_deduplicated > 0 {
        notes.push(format!(
            "{} counted once — {} not double counted",
            plural(stats.hardlinks_deduplicated, "hardlink", "hardlinks"),
            format_bytes(stats.hardlink_bytes_saved)
        ));
    }

    notes
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanReport<'a> {
    root: String,
    source: &'a str,
    elapsed_seconds: f64,
    items: u64,
    directories: u64,
    total_bytes: u64,
    warnings: TreeStats,
    entries: Vec<Entry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    name: String,
    kind: nirmoka_core::NodeKind,
    total_bytes: u64,
    own_bytes: u64,
    apparent_bytes: u64,
    read_error: bool,
    excluded: bool,
    hardlink: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<Entry>,
}

fn print_json(args: &ScanArgs, scan: &Scan, elapsed: f64) {
    let root = scan.tree.root();

    let report = ScanReport {
        root: scan.tree.root_path().display().to_string(),
        source: &scan.source,
        elapsed_seconds: elapsed,
        items: scan.items,
        directories: scan.directories,
        total_bytes: root
            .and_then(|id| scan.tree.get(id).ok())
            .map(|node| node.total_bytes)
            .unwrap_or(0),
        warnings: scan.stats,
        // Only the requested window is serialised, never the whole tree. Two
        // million nodes of JSON is exactly the mistake invariant 5 exists to
        // prevent, and the CLI is not exempt from it.
        entries: root
            .map(|id| collect(&scan.tree, id, args.depth, args.limit))
            .unwrap_or_default(),
    };

    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("ScanReport is serialisable")
    );
}

fn collect(tree: &Tree, parent: NodeId, depth: usize, limit: usize) -> Vec<Entry> {
    if depth == 0 {
        return Vec::new();
    }

    let children = tree.children_by_size(parent);
    let shown = if limit == 0 {
        children.len()
    } else {
        limit.min(children.len())
    };

    children[..shown]
        .iter()
        .filter_map(|id| {
            let node = tree.get(*id).ok()?;
            Some(Entry {
                name: node.name.clone(),
                kind: node.kind,
                total_bytes: node.total_bytes,
                own_bytes: node.own_bytes,
                apparent_bytes: node.apparent_bytes,
                read_error: node.read_error,
                excluded: node.excluded,
                hardlink: node.hardlink,
                children: collect(tree, *id, depth - 1, limit),
            })
        })
        .collect()
}

/// "1 entry" / "2 entries". A warning line about disk space that reads as
/// broken English invites the reader to distrust the number next to it.
fn plural(count: u64, one: &str, many: &str) -> String {
    if count == 1 {
        format!("{count} {one}")
    } else {
        format!("{} {many}", thousands(count))
    }
}

/// 1234567 → "1,234,567". Item counts are the one place in this tool where a
/// raw number is genuinely hard to read.
fn thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);

    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_digits_in_threes() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(1234567), "1,234,567");
    }

    #[test]
    fn notes_explain_every_kind_of_incomplete_number() {
        let stats = TreeStats {
            read_errors: 2,
            excluded: 1,
            hardlinks_deduplicated: 3,
            hardlink_bytes_saved: 4096,
        };

        let notes = notes(&stats);
        assert_eq!(notes.len(), 3);
        assert!(notes[0].contains("lower bound"));
        assert!(notes[1].contains("unknown, not zero"));
        assert!(notes[2].contains("4.00 KB"));
    }

    #[test]
    fn counts_of_one_read_as_english() {
        let notes = notes(&TreeStats {
            read_errors: 1,
            excluded: 1,
            hardlinks_deduplicated: 1,
            hardlink_bytes_saved: 1,
        });

        assert!(notes[0].starts_with("1 entry could not"), "{}", notes[0]);
        assert!(notes[1].starts_with("1 entry was excluded"), "{}", notes[1]);
        assert!(notes[2].starts_with("1 hardlink counted"), "{}", notes[2]);
    }

    #[test]
    fn a_clean_scan_has_nothing_to_explain() {
        assert!(notes(&TreeStats::default()).is_empty());
    }
}
