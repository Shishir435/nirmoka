//! Everything the shell remembers between commands.
//!
//! The tree lives here, in Rust, and never crosses to the webview (invariant 5).
//! A scan of a home directory is millions of nodes; the frontend asks for the
//! window it is about to paint and gets exactly that.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use nirmoka_adapter::registry::RegistryEntry;
use nirmoka_adapter::{Adapter, CancelToken, Registry};
use nirmoka_adapter_ncdu::NcduAdapter;
use nirmoka_core::Tree;

use crate::dto;

/// The registry every entry point builds.
///
/// `nirmoka-cli` and `tests/contract` build the same one. They must agree — a
/// backend that works in the CLI and is missing in the app would be found by
/// nobody until a user reported it.
pub fn registry() -> Registry {
    let mut registry = Registry::new();
    registry.register(Box::new(NcduAdapter::new()));
    registry
}

/// Which scan a node id came from.
///
/// Node ids are indices into an arena, and every scan numbers its nodes from
/// zero. Without this, an id held by the webview across a rescan still resolves
/// against the replacement tree whenever that tree is long enough — and names a
/// different file. Ids are only meaningful together with the scan that issued
/// them, so they travel together.
pub type ScanId = u64;

/// A completed scan: the tree, and the facts about how it was produced.
pub struct ScanResult {
    pub id: ScanId,
    pub tree: Tree,
    pub summary: dto::ScanSummary,
}

/// A scan in flight. Holding the token is what makes the stop button real.
pub struct ActiveScan {
    pub id: ScanId,
    pub root: PathBuf,
    pub cancel: CancelToken,
}

#[derive(Default)]
pub struct ScanState {
    pub active: Option<ActiveScan>,
    pub result: Option<ScanResult>,
    /// Never reused, never reset. A counter that wrapped would hand a new scan
    /// an id an old one already used, which is the failure this exists to stop;
    /// at u64, one scan per nanosecond runs out after five hundred years.
    next_id: ScanId,
}

impl ScanState {
    pub fn issue_id(&mut self) -> ScanId {
        self.next_id += 1;
        self.next_id
    }
}

pub struct AppState {
    registry: Registry,
    scan: Mutex<ScanState>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            registry: registry(),
            scan: Mutex::new(ScanState::default()),
        }
    }

    /// Lock the scan state.
    ///
    /// A poisoned mutex means a worker thread panicked mid-scan. The tree it was
    /// building is gone either way, so recovering the guard and letting the next
    /// scan overwrite it beats making every later command fail.
    pub fn scan(&self) -> MutexGuard<'_, ScanState> {
        self.scan
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn detect_all(&self) -> Vec<RegistryEntry> {
        self.registry.detect_all()
    }

    /// The backend a scan would actually use: the first one detection says is
    /// installed at a version this build understands.
    ///
    /// Returns `None` rather than falling back to an untested version. See
    /// `docs/adapters.md` — an untested version is `UnsupportedVersion`, not an
    /// optimistic `Found`.
    pub fn usable_adapter(&self) -> Option<&dyn Adapter> {
        self.registry
            .iter()
            .find(|adapter| matches!(adapter.detect(), Ok(detection) if detection.is_usable()))
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_holds_the_backends_the_cli_reports() {
        let ids: Vec<_> = registry().iter().map(|adapter| adapter.id()).collect();
        assert_eq!(ids, vec!["ncdu"]);
    }

    #[test]
    fn a_fresh_state_has_no_scan_and_no_result() {
        let state = AppState::new();
        let scan = state.scan();

        assert!(scan.active.is_none());
        assert!(scan.result.is_none());
    }
}
