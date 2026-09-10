//! The wire format: ncdu's JSON export.
//!
//! Every adapter emits this shape, whatever its backend actually speaks. ncdu
//! and gdu produce it natively; Mole's adapter translates down into it. See
//! `docs/adr/0002`.
//!
//! # Why the parser lives in the contract crate
//!
//! The format is part of the adapter contract, not an ncdu implementation
//! detail. Two of three planned backends emit it verbatim, and the step 6
//! contract suite parses recorded fixtures from *every* backend with one
//! parser. Putting it in `adapter-ncdu` would force `adapter-gdu` and
//! `adapter-mole` to depend on a sibling adapter. See `docs/adr/0008`.
//!
//! It cannot live in `crates/core` either: core is restricted to the standard
//! library, serde, and thiserror, and a JSON parser is none of those.
//!
//! # Shape
//!
//! ```text
//! [1, 2, {"progname":"ncdu","progver":"2.8.2","timestamp":…}, DIR]
//!
//! DIR   = [ITEM, ENTRY, ENTRY, …]     the first element describes the directory
//! ENTRY = ITEM | DIR                  an object is a leaf, an array is a subdir
//! ITEM  = {"name":"…", "asize":…, "dsize":…, …}
//! ```
//!
//! The leading `1` is the *format* major version, which is unrelated to the
//! ncdu release version: ncdu 2.8.2 emits format 1.2. Fields absent from an
//! item are zero or false, which is why a sparse file carries `asize` but no
//! `dsize` — its disk usage really is zero.
//!
//! # Streaming
//!
//! Parsing is a pull over the reader that hands each entry to a [`WireSink`] as
//! it is decoded. Nothing accumulates the whole document first, so a scan of a
//! large home directory starts producing entries immediately and never holds
//! the JSON text in memory.

use std::collections::HashSet;
use std::io::Read;
use std::path::PathBuf;

use serde::de::{self, DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use nirmoka_core::{Node, NodeId, NodeKind, Tree};

/// Export format major version this parser understands. Bumped only by an ADR.
pub const SUPPORTED_FORMAT_MAJOR: u64 = 1;

/// Directory nesting cap.
///
/// serde_json's own recursion limit is 128, which real filesystems get close to
/// on their own (nested `node_modules`), so it is disabled and replaced with
/// this.
///
/// The ceiling is the stack, not taste: a level of nesting costs roughly two
/// kilobytes of serde frames in a debug build, and threads default to two
/// megabytes — Rust's test harness and Tauri's worker pool both. 1024 levels
/// overflows and aborts the process; 256 leaves an order of magnitude of
/// headroom and is still twice what serde_json allows. Deeper trees are
/// reported as [`WireError::TooDeep`], which is a scan that explains itself
/// rather than a crash.
pub const MAX_DEPTH: usize = 256;

/// Header of an export: format version plus whatever the producer said about
/// itself.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireHeader {
    pub format_major: u64,
    pub format_minor: u64,
    pub program: Option<String>,
    pub program_version: Option<String>,
    pub timestamp: Option<i64>,
}

/// One entry as the backend reported it, before any interpretation.
///
/// Field names are Nirmoka's; the serde renames are the format's.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct WireItem {
    pub name: String,

    /// Apparent size — what the file claims to be.
    #[serde(default, rename = "asize")]
    pub apparent_bytes: u64,

    /// Disk usage. Absent means zero, which is the truth for sparse files and
    /// for directories on filesystems that do not charge for them.
    #[serde(default, rename = "dsize")]
    pub disk_bytes: u64,

    /// Emitted only when it differs from the parent's, so readers inherit it.
    #[serde(default)]
    pub dev: Option<u64>,

    #[serde(default)]
    pub ino: Option<u64>,

    /// The backend saw more than one link to this inode. Together with `ino`
    /// this is what makes deduplication possible.
    #[serde(default, rename = "hlnkc")]
    pub hardlink_candidate: bool,

    #[serde(default)]
    pub nlink: u64,

    /// Neither a regular file nor a directory: symlink, socket, device, fifo.
    #[serde(default, rename = "notreg")]
    pub not_regular: bool,

    /// The backend could not read this entry.
    #[serde(default)]
    pub read_error: bool,

    /// Present when the backend skipped the entry; the value says why
    /// (`"pattern"`, `"othfs"`, `"kernfs"`, `"frmlnk"`).
    #[serde(default)]
    pub excluded: Option<String>,

    /// Only present in extended exports (`ncdu -e`). Used to tell an excluded
    /// directory from an excluded file, which is otherwise unknowable.
    #[serde(default)]
    pub mode: Option<u32>,
}

/// What a parse produced, independent of what the sink did with it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WireStats {
    pub header: WireHeader,
    pub items: u64,
    pub directories: u64,
}

#[derive(Debug, Error)]
pub enum WireError {
    #[error("the backend produced no output at all")]
    Empty,

    #[error("output ended mid-export: {0}")]
    Truncated(String),

    #[error(
        "unsupported export format version {major}.{minor}; this build understands {supported}.x"
    )]
    UnsupportedFormat {
        major: u64,
        minor: u64,
        supported: u64,
    },

    #[error("directory nesting exceeded {limit} levels")]
    TooDeep { limit: usize },

    #[error("malformed export: {0}")]
    Malformed(String),

    #[error("could not read export: {0}")]
    Io(#[source] std::io::Error),
}

/// Receives entries as they are decoded.
///
/// `open_dir` and `close_dir` bracket a directory's children, so a sink can
/// keep a stack and never needs to know the tree in advance. Implementors that
/// only care about totals can ignore the bracketing entirely.
pub trait WireSink {
    fn header(&mut self, _header: &WireHeader) {}

    fn open_dir(&mut self, item: WireItem);

    fn item(&mut self, item: WireItem);

    fn close_dir(&mut self);
}

/// Parse an export, streaming entries into `sink`.
pub fn parse<R: Read, S: WireSink + ?Sized>(
    reader: R,
    sink: &mut S,
) -> Result<WireStats, WireError> {
    let mut counting = CountingReader {
        inner: reader,
        bytes: 0,
    };

    let mut ctx = Ctx {
        sink,
        header: None,
        items: 0,
        directories: 0,
        depth: 0,
        fatal: None,
    };

    let outcome = {
        let mut de = serde_json::Deserializer::from_reader(&mut counting);
        // Replaced by MAX_DEPTH below; see the constant's docs.
        de.disable_recursion_limit();
        ExportSeed { ctx: &mut ctx }
            .deserialize(&mut de)
            // `ncdu -o -` emits exactly one JSON value. Trailing bytes mean the
            // stream is not what it claims to be, which is worth failing on
            // rather than ignoring.
            .and_then(|()| de.end())
    };

    let bytes_read = counting.bytes;

    match outcome {
        Ok(()) => Ok(WireStats {
            header: ctx.header.unwrap_or_default(),
            items: ctx.items,
            directories: ctx.directories,
        }),
        // A fatal set by a visitor is always more specific than the serde error
        // that carried it out of the parse.
        Err(err) => Err(ctx
            .fatal
            .take()
            .unwrap_or_else(|| translate(err, bytes_read))),
    }
}

fn translate(err: serde_json::Error, bytes_read: u64) -> WireError {
    match err.classify() {
        serde_json::error::Category::Io => WireError::Io(err.into()),
        serde_json::error::Category::Eof => {
            if bytes_read == 0 {
                WireError::Empty
            } else {
                WireError::Truncated(err.to_string())
            }
        }
        _ => WireError::Malformed(err.to_string()),
    }
}

/// Counts bytes so an empty stream can be reported as empty rather than as a
/// truncated one — "the backend printed nothing" and "the backend died halfway"
/// need different messages in the UI.
struct CountingReader<R> {
    inner: R,
    bytes: u64,
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.bytes += read as u64;
        Ok(read)
    }
}

// ---------------------------------------------------------------------------
// Seeds and visitors.
//
// `DeserializeSeed` rather than `Deserialize` because the sink has to be
// threaded through the decode. That is what makes this streaming: entries reach
// the sink during the parse instead of after it.
// ---------------------------------------------------------------------------

struct Ctx<'a, S: WireSink + ?Sized> {
    sink: &'a mut S,
    header: Option<WireHeader>,
    items: u64,
    directories: u64,
    depth: usize,
    /// Set by a visitor that knows exactly what went wrong, immediately before
    /// it returns a generic serde error. serde's error type cannot carry ours.
    fatal: Option<WireError>,
}

#[derive(Debug, Default, Deserialize)]
struct WireMeta {
    progname: Option<String>,
    progver: Option<String>,
    timestamp: Option<i64>,
}

struct ExportSeed<'c, 'a, S: WireSink + ?Sized> {
    ctx: &'c mut Ctx<'a, S>,
}

impl<'de, S: WireSink + ?Sized> DeserializeSeed<'de> for ExportSeed<'_, '_, S> {
    type Value = ();

    fn deserialize<D: de::Deserializer<'de>>(self, de: D) -> Result<(), D::Error> {
        de.deserialize_seq(ExportVisitor { ctx: self.ctx })
    }
}

struct ExportVisitor<'c, 'a, S: WireSink + ?Sized> {
    ctx: &'c mut Ctx<'a, S>,
}

impl<'de, S: WireSink + ?Sized> Visitor<'de> for ExportVisitor<'_, '_, S> {
    type Value = ();

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("an ncdu export: [major, minor, metadata, tree]")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        let major: u64 = seq
            .next_element()?
            .ok_or_else(|| de::Error::custom("export has no format version"))?;
        let minor: u64 = seq.next_element()?.unwrap_or(0);

        if major != SUPPORTED_FORMAT_MAJOR {
            // Fail closed. An unknown format is not an optimistic parse — the
            // same reasoning that makes an untested backend version
            // `UnsupportedVersion` rather than `Found`.
            self.ctx.fatal = Some(WireError::UnsupportedFormat {
                major,
                minor,
                supported: SUPPORTED_FORMAT_MAJOR,
            });
            return Err(de::Error::custom("unsupported export format version"));
        }

        let meta: WireMeta = seq.next_element()?.unwrap_or_default();
        let header = WireHeader {
            format_major: major,
            format_minor: minor,
            program: meta.progname,
            program_version: meta.progver,
            timestamp: meta.timestamp,
        };

        self.ctx.sink.header(&header);
        self.ctx.header = Some(header);

        seq.next_element_seed(DirSeed {
            ctx: &mut *self.ctx,
        })?
        .ok_or_else(|| de::Error::custom("export contains no tree"))?;

        // Later format minors may append fields; ignoring them is what makes a
        // minor bump backwards-compatible.
        while seq.next_element::<IgnoredAny>()?.is_some() {}

        Ok(())
    }
}

struct DirSeed<'c, 'a, S: WireSink + ?Sized> {
    ctx: &'c mut Ctx<'a, S>,
}

impl<'de, S: WireSink + ?Sized> DeserializeSeed<'de> for DirSeed<'_, '_, S> {
    type Value = ();

    fn deserialize<D: de::Deserializer<'de>>(self, de: D) -> Result<(), D::Error> {
        de.deserialize_seq(DirVisitor { ctx: self.ctx })
    }
}

struct DirVisitor<'c, 'a, S: WireSink + ?Sized> {
    ctx: &'c mut Ctx<'a, S>,
}

impl<'de, S: WireSink + ?Sized> Visitor<'de> for DirVisitor<'_, '_, S> {
    type Value = ();

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a directory: [item, entry, …]")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        if self.ctx.depth >= MAX_DEPTH {
            self.ctx.fatal = Some(WireError::TooDeep { limit: MAX_DEPTH });
            return Err(de::Error::custom("directory nesting too deep"));
        }

        let item: WireItem = seq
            .next_element()?
            .ok_or_else(|| de::Error::custom("directory array is missing its own record"))?;

        self.ctx.items += 1;
        self.ctx.directories += 1;
        self.ctx.sink.open_dir(item);
        self.ctx.depth += 1;

        while seq
            .next_element_seed(EntrySeed {
                ctx: &mut *self.ctx,
            })?
            .is_some()
        {}

        self.ctx.depth -= 1;
        self.ctx.sink.close_dir();

        Ok(())
    }
}

struct EntrySeed<'c, 'a, S: WireSink + ?Sized> {
    ctx: &'c mut Ctx<'a, S>,
}

impl<'de, S: WireSink + ?Sized> DeserializeSeed<'de> for EntrySeed<'_, '_, S> {
    type Value = ();

    fn deserialize<D: de::Deserializer<'de>>(self, de: D) -> Result<(), D::Error> {
        // An entry is an object (leaf) or an array (subdirectory), and which
        // one is only known once a byte has been looked at.
        de.deserialize_any(EntryVisitor { ctx: self.ctx })
    }
}

struct EntryVisitor<'c, 'a, S: WireSink + ?Sized> {
    ctx: &'c mut Ctx<'a, S>,
}

impl<'de, S: WireSink + ?Sized> Visitor<'de> for EntryVisitor<'_, '_, S> {
    type Value = ();

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("an item object or a nested directory array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<(), A::Error> {
        DirVisitor { ctx: self.ctx }.visit_seq(seq)
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<(), A::Error> {
        let item = WireItem::deserialize(de::value::MapAccessDeserializer::new(map))?;
        self.ctx.items += 1;
        self.ctx.sink.item(item);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// The sink that produces a Tree.
// ---------------------------------------------------------------------------

/// Counts that explain why a total is what it is.
///
/// A cleanup tool that reports a number without saying "and 12 directories were
/// unreadable" is lying by omission, so these travel with the tree.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeStats {
    pub read_errors: u64,
    pub excluded: u64,
    /// Entries whose bytes were already counted under another name.
    pub hardlinks_deduplicated: u64,
    /// Bytes those entries would have added if they had been counted twice.
    pub hardlink_bytes_saved: u64,
}

/// Builds a [`Tree`] from a parse.
///
/// # Hardlinks
///
/// The first entry seen for an inode carries its bytes; later ones are flagged
/// [`Node::hardlink`] and carry zero. This is what ncdu itself does, and it is
/// the only way a directory total means "space freed if this is deleted".
#[derive(Debug)]
pub struct TreeSink {
    tree: Option<Tree>,
    stack: Vec<NodeId>,
    /// Device of the directory at each level. ncdu emits `dev` only when it
    /// changes, so children inherit it, and an inode number alone is not unique
    /// across devices.
    devices: Vec<u64>,
    seen_inodes: HashSet<(u64, u64)>,
    stats: TreeStats,
}

impl Default for TreeSink {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeSink {
    pub fn new() -> Self {
        Self {
            tree: None,
            stack: Vec::new(),
            devices: Vec::new(),
            seen_inodes: HashSet::new(),
            stats: TreeStats::default(),
        }
    }

    pub fn stats(&self) -> TreeStats {
        self.stats
    }

    /// The finished tree, with sizes rolled up.
    pub fn finish(mut self) -> Tree {
        let mut tree = self.tree.take().unwrap_or_else(|| Tree::new(""));
        tree.rollup();
        tree
    }

    fn current_device(&self) -> u64 {
        self.devices.last().copied().unwrap_or(0)
    }

    fn node_from(&mut self, item: &WireItem, kind: NodeKind) -> Node {
        let device = item.dev.unwrap_or_else(|| self.current_device());

        let mut node = Node {
            name: item.name.clone(),
            kind,
            own_bytes: item.disk_bytes,
            apparent_bytes: item.apparent_bytes,
            total_bytes: 0,
            read_error: item.read_error,
            hardlink: false,
            excluded: item.excluded.is_some(),
        };

        if item.read_error {
            self.stats.read_errors += 1;
        }
        if node.excluded {
            self.stats.excluded += 1;
        }

        if item.hardlink_candidate {
            if let Some(ino) = item.ino {
                if !self.seen_inodes.insert((device, ino)) {
                    self.stats.hardlinks_deduplicated += 1;
                    self.stats.hardlink_bytes_saved = self
                        .stats
                        .hardlink_bytes_saved
                        .saturating_add(node.own_bytes);
                    node.hardlink = true;
                    node.own_bytes = 0;
                    node.apparent_bytes = 0;
                }
            }
        }

        node
    }

    fn push(&mut self, node: Node) -> NodeId {
        let parent = self.stack.last().copied();
        let tree = self
            .tree
            .get_or_insert_with(|| Tree::new(PathBuf::from(&node.name)));
        tree.push(parent, node)
    }
}

impl WireSink for TreeSink {
    fn open_dir(&mut self, item: WireItem) {
        let device = item.dev.unwrap_or_else(|| self.current_device());
        let node = self.node_from(&item, NodeKind::Directory);
        let id = self.push(node);
        self.stack.push(id);
        self.devices.push(device);
    }

    fn item(&mut self, item: WireItem) {
        let kind = kind_of(&item);
        let node = self.node_from(&item, kind);
        self.push(node);
    }

    fn close_dir(&mut self) {
        self.stack.pop();
        self.devices.pop();
    }
}

/// What an entry is, using only what the wire format actually says.
///
/// Without extended mode the format distinguishes exactly two things: regular
/// and not-regular. Guessing "symlink" from a not-regular entry would be
/// inventing information, so it stays [`NodeKind::Other`] unless `mode` is
/// present to say otherwise.
fn kind_of(item: &WireItem) -> NodeKind {
    const FORMAT_MASK: u32 = 0o170000;
    const DIRECTORY: u32 = 0o040000;
    const SYMLINK: u32 = 0o120000;
    const REGULAR: u32 = 0o100000;

    if let Some(mode) = item.mode {
        return match mode & FORMAT_MASK {
            DIRECTORY => NodeKind::Directory,
            SYMLINK => NodeKind::Symlink,
            REGULAR => NodeKind::File,
            _ => NodeKind::Other,
        };
    }

    if item.not_regular {
        NodeKind::Other
    } else {
        NodeKind::File
    }
}

/// Parse straight into a tree. The common case.
pub fn parse_tree<R: Read>(reader: R) -> Result<(Tree, WireStats, TreeStats), WireError> {
    let mut sink = TreeSink::new();
    let stats = parse(reader, &mut sink)?;
    let tree_stats = sink.stats();
    Ok((sink.finish(), stats, tree_stats))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape ncdu 2.8.2 produces, trimmed to what each test needs.
    const SIMPLE: &str = r#"[1,2,{"progname":"ncdu","progver":"2.8.2","timestamp":1},
        [{"name":"/root","asize":192,"dsize":4096,"dev":16777230},
         [{"name":"empty","asize":64,"dsize":4096}],
         {"name":"big.bin","asize":5000,"dsize":8192}]]"#;

    fn tree_of(json: &str) -> (Tree, WireStats, TreeStats) {
        parse_tree(json.as_bytes()).expect("fixture parses")
    }

    #[test]
    fn reads_the_header() {
        let (_, stats, _) = tree_of(SIMPLE);
        assert_eq!(stats.header.format_major, 1);
        assert_eq!(stats.header.format_minor, 2);
        assert_eq!(stats.header.program.as_deref(), Some("ncdu"));
        assert_eq!(stats.header.program_version.as_deref(), Some("2.8.2"));
    }

    #[test]
    fn builds_the_tree_and_counts_entries() {
        let (tree, stats, _) = tree_of(SIMPLE);
        assert_eq!(stats.items, 3);
        assert_eq!(stats.directories, 2);
        assert_eq!(tree.len(), 3);

        let root = tree.root().unwrap();
        assert_eq!(tree.get(root).unwrap().name, "/root");
        assert_eq!(tree.children_of(root).len(), 2);
    }

    #[test]
    fn rolls_disk_usage_not_apparent_size() {
        let (tree, _, _) = tree_of(SIMPLE);
        let root = tree.root().unwrap();
        // 4096 (root) + 4096 (empty) + 8192 (big.bin), not the asize numbers.
        assert_eq!(tree.get(root).unwrap().total_bytes, 16384);
    }

    #[test]
    fn a_sparse_file_reports_zero_disk_usage() {
        // asize without dsize is exactly how ncdu writes a sparse file: it
        // claims 2 MB and occupies nothing. Falling back to asize here would
        // promise 2 MB of reclaimable space that deleting it will not free.
        let json = r#"[1,2,{},[{"name":"/root","dsize":4096},
            {"name":"sparse.img","asize":2097152}]]"#;
        let (tree, _, _) = tree_of(json);
        let file = tree.children_of(tree.root().unwrap())[0];
        assert_eq!(tree.get(file).unwrap().own_bytes, 0);
        assert_eq!(tree.get(file).unwrap().apparent_bytes, 2097152);
        assert_eq!(tree.get(tree.root().unwrap()).unwrap().total_bytes, 4096);
    }

    #[test]
    fn counts_a_hardlinked_inode_once() {
        let json = r#"[1,2,{},[{"name":"/root","dev":1},
            {"name":"a.txt","asize":11,"dsize":4096,"ino":7,"hlnkc":true,"nlink":2},
            {"name":"b.txt","asize":11,"dsize":4096,"ino":7,"hlnkc":true,"nlink":2}]]"#;
        let (tree, _, tree_stats) = tree_of(json);
        let root = tree.root().unwrap();

        assert_eq!(tree.get(root).unwrap().total_bytes, 4096);
        assert_eq!(tree_stats.hardlinks_deduplicated, 1);
        assert_eq!(tree_stats.hardlink_bytes_saved, 4096);

        let second = tree.children_of(root)[1];
        assert!(tree.get(second).unwrap().hardlink);
        assert_eq!(tree.get(second).unwrap().own_bytes, 0);
    }

    #[test]
    fn the_same_inode_on_another_device_is_a_different_file() {
        // Inode numbers are only unique per device. Deduplicating across
        // devices would silently under-report a mounted volume.
        let json = r#"[1,2,{},[{"name":"/root","dev":1},
            {"name":"a","dsize":4096,"ino":7,"hlnkc":true},
            [{"name":"mnt","dev":2},
             {"name":"b","dsize":4096,"ino":7,"hlnkc":true}]]]"#;
        let (tree, _, tree_stats) = tree_of(json);
        assert_eq!(tree_stats.hardlinks_deduplicated, 0);
        assert_eq!(tree.get(tree.root().unwrap()).unwrap().total_bytes, 8192);
    }

    #[test]
    fn records_read_errors_without_dropping_the_entry() {
        let json = r#"[1,2,{},[{"name":"/root","dsize":4096},
            [{"name":"noread","dsize":4096,"read_error":true}]]]"#;
        let (tree, _, tree_stats) = tree_of(json);
        assert_eq!(tree_stats.read_errors, 1);

        let dir = tree.children_of(tree.root().unwrap())[0];
        let node = tree.get(dir).unwrap();
        assert!(node.read_error);
        assert!(node.size_is_partial());
        assert!(node.is_dir());
    }

    #[test]
    fn marks_excluded_entries_rather_than_calling_them_empty() {
        let json = r#"[1,2,{},[{"name":"/root","dsize":4096},
            {"name":"skipped","excluded":"pattern"}]]"#;
        let (tree, _, tree_stats) = tree_of(json);
        assert_eq!(tree_stats.excluded, 1);

        let node = tree.get(tree.children_of(tree.root().unwrap())[0]).unwrap();
        assert!(node.excluded);
        assert!(node.size_is_partial());
        assert_eq!(node.own_bytes, 0);
    }

    #[test]
    fn a_non_regular_file_is_other_not_a_guessed_symlink() {
        let json = r#"[1,2,{},[{"name":"/root"},{"name":"link","asize":9,"notreg":true}]]"#;
        let (tree, _, _) = tree_of(json);
        let node = tree.get(tree.children_of(tree.root().unwrap())[0]).unwrap();
        assert_eq!(node.kind, NodeKind::Other);
    }

    #[test]
    fn extended_mode_identifies_symlinks_and_directories() {
        // `ncdu -e` adds mode, which is the only thing that can tell an
        // excluded directory from an excluded file.
        let json = r#"[1,2,{},[{"name":"/root"},
            {"name":"link","mode":41471,"notreg":true},
            {"name":"skipped","mode":16877,"excluded":"pattern"}]]"#;
        let (tree, _, _) = tree_of(json);
        let children = tree.children_of(tree.root().unwrap());
        assert_eq!(tree.get(children[0]).unwrap().kind, NodeKind::Symlink);
        assert_eq!(tree.get(children[1]).unwrap().kind, NodeKind::Directory);
    }

    #[test]
    fn reconstructs_paths_from_the_scan_root() {
        let json = r#"[1,2,{},[{"name":"/root"},[{"name":"a"},{"name":"f"}]]]"#;
        let (tree, _, _) = tree_of(json);
        let a = tree.children_of(tree.root().unwrap())[0];
        let f = tree.children_of(a)[0];
        assert_eq!(tree.path_of(f).unwrap(), PathBuf::from("/root/a/f"));
    }

    #[test]
    fn nests_deeper_than_serde_jsons_own_recursion_limit() {
        // serde_json stops at 128 levels by default. Real trees go deeper, and
        // a scan that dies at level 129 would look like a corrupt backend.
        let depth = 200;
        let mut json = String::from(r#"[1,2,{},"#);
        for level in 0..depth {
            json.push_str(&format!(r#"[{{"name":"d{level}","dsize":1}},"#));
        }
        json.push_str(r#"{"name":"leaf","dsize":7}"#);
        json.push_str(&"]".repeat(depth));
        json.push(']');

        let (tree, stats, _) = tree_of(&json);
        assert_eq!(stats.directories, depth as u64);
        assert_eq!(tree.get(tree.root().unwrap()).unwrap().total_bytes, 207);
    }

    #[test]
    fn refuses_absurd_nesting() {
        let depth = MAX_DEPTH + 1;
        let mut json = String::from(r#"[1,2,{},"#);
        for _ in 0..depth {
            json.push_str(r#"[{"name":"d"},"#);
        }
        json.push_str(r#"{"name":"leaf"}"#);
        json.push_str(&"]".repeat(depth));
        json.push(']');

        assert!(matches!(
            parse_tree(json.as_bytes()),
            Err(WireError::TooDeep { .. })
        ));
    }

    #[test]
    fn rejects_an_empty_stream() {
        assert!(matches!(parse_tree(b"".as_slice()), Err(WireError::Empty)));
    }

    #[test]
    fn rejects_truncated_output() {
        // The failure mode this guards: a killed backend leaving half an
        // export, which must not be mistaken for a small disk.
        let truncated = &SIMPLE[..SIMPLE.len() / 2];
        assert!(matches!(
            parse_tree(truncated.as_bytes()),
            Err(WireError::Truncated(_))
        ));
    }

    #[test]
    fn rejects_an_unknown_format_major() {
        let json = r#"[2,0,{},[{"name":"/root"}]]"#;
        match parse_tree(json.as_bytes()) {
            Err(WireError::UnsupportedFormat { major, minor, .. }) => {
                assert_eq!((major, minor), (2, 0));
            }
            other => panic!("expected UnsupportedFormat, got {other:?}"),
        }
    }

    #[test]
    fn accepts_an_unknown_format_minor() {
        // Minor bumps add fields. Refusing them would break on an ncdu release
        // that changed nothing that matters here.
        let json = r#"[1,99,{},[{"name":"/root","dsize":1,"future_field":[1,2]}]]"#;
        let (tree, stats, _) = tree_of(json);
        assert_eq!(stats.header.format_minor, 99);
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn rejects_json_that_is_not_an_export() {
        for json in [
            r#"{"name":"/root"}"#,
            "[]",
            "[1,2,{}]",
            "null",
            "[1,2,{},[]]",
        ] {
            assert!(
                matches!(
                    parse_tree(json.as_bytes()),
                    Err(WireError::Malformed(_) | WireError::Truncated(_))
                ),
                "{json} should not parse as an export"
            );
        }
    }

    #[test]
    fn rejects_an_item_with_no_name() {
        let json = r#"[1,2,{},[{"dsize":1}]]"#;
        assert!(matches!(
            parse_tree(json.as_bytes()),
            Err(WireError::Malformed(_))
        ));
    }

    #[test]
    fn rejects_trailing_data_after_the_export() {
        // Two concatenated exports are not one big scan.
        let json = format!("{SIMPLE}{SIMPLE}");
        assert!(matches!(
            parse_tree(json.as_bytes()),
            Err(WireError::Malformed(_))
        ));
    }

    #[test]
    fn reports_reader_failures_as_io_not_as_malformed_output() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "gone"))
            }
        }

        assert!(matches!(parse_tree(Broken), Err(WireError::Io(_))));
    }

    #[test]
    fn streams_entries_before_the_parse_finishes() {
        // The property that matters for a 2M-node scan: entries arrive during
        // the parse. A sink that panics on the first item proves nothing was
        // buffered up front, because it fires before the reader is exhausted.
        struct Recorder {
            seen: Vec<String>,
        }
        impl WireSink for Recorder {
            fn open_dir(&mut self, item: WireItem) {
                self.seen.push(format!("dir:{}", item.name));
            }
            fn item(&mut self, item: WireItem) {
                self.seen.push(format!("file:{}", item.name));
            }
            fn close_dir(&mut self) {
                self.seen.push("close".into());
            }
        }

        let mut recorder = Recorder { seen: Vec::new() };
        parse(SIMPLE.as_bytes(), &mut recorder).unwrap();
        assert_eq!(
            recorder.seen,
            ["dir:/root", "dir:empty", "close", "file:big.bin", "close"]
        );
    }
}
