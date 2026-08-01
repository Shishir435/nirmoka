//! Shared helpers for the contract suite.
//!
//! The suite proper lives in `tests/`. This module exists so every test file
//! builds the same registry and finds fixtures the same way — an adapter that
//! passes because its test set it up differently has not been tested.

use std::path::PathBuf;

use nirmoka_adapter::Registry;
use nirmoka_adapter_gdu::GduAdapter;
use nirmoka_adapter_mole::MoleAdapter;
use nirmoka_adapter_ncdu::NcduAdapter;

/// Every adapter Nirmoka ships, in registration order.
///
/// Registration order is not preference order — `Registry::resolve` picks, from
/// the user's choice and the platform default. See ADR 0013. What matters here
/// is only that this registry holds the same adapters as the CLI's and the Tauri
/// app's: when it stops matching, one of the three is running a backend the
/// others do not know about, which is exactly the drift a contract suite exists
/// to catch.
pub fn registry() -> Registry {
    let mut registry = Registry::new();
    registry.register(Box::new(NcduAdapter::new()));
    registry.register(Box::new(MoleAdapter::new()));
    registry.register(Box::new(GduAdapter::new()));
    registry
}

/// Recorded backend output, committed so this suite needs no live backend.
pub fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .canonicalize()
        .expect("fixtures/ is committed at the repository root")
}

/// Every recorded **wire-format export**, as (backend, version, name, path).
///
/// Driven by what is on disk rather than by a list in code: a fixture that is
/// recorded but never asserted on is worse than no fixture, because it looks
/// like coverage.
///
/// Not everything under `fixtures/` is an export. `fixtures/mole/` holds
/// recorded `mo analyze --json` output, which Nirmoka never parses — it is the
/// evidence for why the Mole adapter declares `scan: false`, kept so an upgrade
/// re-tests the finding instead of leaving it as a claim in a comment.
///
/// The filter asks the registry rather than matching on a directory name, so a
/// backend that gains the ability to emit the wire format joins this suite by
/// flipping its capability flag and nothing else.
pub fn all_fixtures() -> Vec<Fixture> {
    let exporters: Vec<String> = registry()
        .iter()
        .filter(|adapter| adapter.capabilities().scan)
        .map(|adapter| adapter.id().to_string())
        .collect();

    let mut fixtures = Vec::new();

    for backend in read_dir(&fixtures_root()) {
        if !backend.is_dir() || !exporters.contains(&name_of(&backend)) {
            continue;
        }
        for version in read_dir(&backend) {
            if !version.is_dir() {
                continue;
            }
            for file in read_dir(&version) {
                if file.extension().and_then(|ext| ext.to_str()) != Some("json") {
                    continue;
                }
                fixtures.push(Fixture {
                    backend: name_of(&backend),
                    version: name_of(&version),
                    name: file
                        .file_stem()
                        .expect("a .json file has a stem")
                        .to_string_lossy()
                        .to_string(),
                    path: file,
                });
            }
        }
    }

    fixtures.sort_by(|a, b| a.path.cmp(&b.path));
    assert!(
        !fixtures.is_empty(),
        "no fixtures found under {}; run scripts/record-ncdu-fixture.sh",
        fixtures_root().display()
    );
    fixtures
}

#[derive(Debug, Clone)]
pub struct Fixture {
    pub backend: String,
    pub version: String,
    pub name: String,
    pub path: PathBuf,
}

impl Fixture {
    pub fn label(&self) -> String {
        format!("{}/{}/{}", self.backend, self.version, self.name)
    }

    pub fn open(&self) -> std::io::BufReader<std::fs::File> {
        std::io::BufReader::new(
            std::fs::File::open(&self.path)
                .unwrap_or_else(|error| panic!("{}: {error}", self.path.display())),
        )
    }
}

fn read_dir(path: &std::path::Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    entries.sort();
    entries
}

fn name_of(path: &std::path::Path) -> String {
    path.file_name()
        .expect("directory entries have names")
        .to_string_lossy()
        .to_string()
}
