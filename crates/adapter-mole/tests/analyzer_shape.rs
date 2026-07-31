//! The evidence behind `scan: false`.
//!
//! The Mole adapter refuses to scan because `mo analyze --json` returns one
//! directory's children rather than a tree. That is a claim about a backend
//! this repository does not control, so it is asserted against recorded output
//! instead of being left in a comment: if a future Mole emits nested entries,
//! this fails and [ADR 0012](../../../docs/adr/0012-mole-is-not-a-scanner.md)
//! gets revisited rather than quietly outliving its reason.
//!
//! No adapter code parses these files. Nirmoka never reads Mole's analyzer
//! output at runtime — these tests read it so a human does not have to
//! re-derive the finding by hand after an upgrade.
//!
//! Re-record with `./scripts/record-mole-fixture.sh`.

use std::fs;
use std::path::PathBuf;

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/mole/1.48.1")
        .join(format!("{name}.json"));

    fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!("{}: {error}", path.display());
    })
}

/// Every `"name"` key in the recording, in order. A crude scan of the text
/// rather than a parse: pulling in a JSON dependency to assert that a file has
/// no nesting would be a dependency added for one test.
fn names(json: &str) -> Vec<String> {
    json.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("\"name\":")?;
            Some(
                rest.trim()
                    .trim_matches(|c| c == '"' || c == ',')
                    .to_string(),
            )
        })
        .collect()
}

/// The finding, stated as a test.
///
/// `nested` holds `big.bin` and a `deeper/` directory. The top-level recording
/// names `nested` and stops — its children appear only in a second recording,
/// from a second invocation of the backend.
#[test]
fn the_analyzer_returns_one_level_not_a_tree() {
    let root = names(&fixture("root"));

    assert!(
        root.iter().any(|name| name == "nested"),
        "the recording lost its directory: {root:?}"
    );
    assert!(
        !root.iter().any(|name| name == "big.bin"),
        "mo analyze now reports entries below its immediate children — \
         ADR 0012 rests on it not doing that: {root:?}"
    );

    // The same directory, asked for directly, is where its children live.
    let nested = names(&fixture("nested"));
    assert!(nested.iter().any(|name| name == "big.bin"), "{nested:?}");
    assert!(
        !nested.iter().any(|name| name == "leaf.txt"),
        "one level from here too: {nested:?}"
    );
}

/// A directory's size is recursive even though its contents are not listed, so
/// the totals are real. It is the shape that cannot be used, not the numbers.
#[test]
fn directory_sizes_are_recursive_even_though_the_listing_is_not() {
    let root = fixture("root");

    assert!(
        root.contains("\"size\": 65537"),
        "nested's size should count big.bin (65536) plus leaf.txt (1): {root}"
    );
}

/// Names carry a display artefact — a symlink is recorded as `link.txt →`.
///
/// Worth pinning: it is the clearest sign that this output is a TUI's data
/// source rather than an interchange format, and any future use of it would
/// have to strip presentation out of the `name` field.
#[test]
fn names_carry_presentation_that_a_wire_format_would_not() {
    let root = names(&fixture("root"));

    assert!(
        root.iter().any(|name| name.contains('→')),
        "the symlink lost its arrow, so Mole's output may have become cleaner: {root:?}"
    );
}
