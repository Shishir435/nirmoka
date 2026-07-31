//! A set of adapters, assembled by the binary that owns `main`.
//!
//! The registry is push-based rather than self-populating so that this crate
//! never depends on a concrete adapter — that would be a dependency cycle.
//! `nirmoka-cli` builds one today; the Tauri app will build the same one later.
//! Both must produce identical results, which is what the contract test suite
//! checks.

use crate::{Adapter, AdapterError, Capabilities, Detection};

#[derive(Default)]
pub struct Registry {
    adapters: Vec<Box<dyn Adapter>>,
}

/// One adapter's detection outcome, ready for display or serialisation.
pub struct RegistryEntry {
    pub id: &'static str,
    pub display_name: &'static str,
    pub supported_versions: &'static str,
    pub detection: Result<Detection, AdapterError>,
    /// What this backend can do, per backend rather than for the app as a
    /// whole. Once two backends differ — Mole cleans and cannot scan, ncdu
    /// scans and cannot preview — a single set of flags describes neither.
    pub capabilities: Capabilities,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, adapter: Box<dyn Adapter>) -> &mut Self {
        self.adapters.push(adapter);
        self
    }

    pub fn len(&self) -> usize {
        self.adapters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &dyn Adapter> {
        self.adapters.iter().map(AsRef::as_ref)
    }

    /// Run detection for every registered adapter.
    ///
    /// A failing adapter yields an `Err` entry rather than aborting the sweep —
    /// one broken backend must not hide the working ones.
    pub fn detect_all(&self) -> Vec<RegistryEntry> {
        self.adapters
            .iter()
            .map(|adapter| RegistryEntry {
                id: adapter.id(),
                display_name: adapter.display_name(),
                supported_versions: adapter.supported_versions(),
                detection: adapter.detect(),
                capabilities: adapter.capabilities(),
            })
            .collect()
    }

    /// The first registered adapter that is installed at a supported version.
    ///
    /// Registration order is preference order, so callers control which backend
    /// wins on a machine that has several.
    pub fn first_usable(&self) -> Option<&dyn Adapter> {
        self.iter()
            .find(|adapter| matches!(adapter.detect(), Ok(d) if d.is_usable()))
    }

    /// The first usable adapter that satisfies `wanted`.
    ///
    /// [`first_usable`](Self::first_usable) answers "is anything installed",
    /// which stopped being the same question as "can anything do this" when
    /// Mole joined the registry: it is usable on macOS and cannot scan. A
    /// caller that wants an ability has to ask for it.
    pub fn first_usable_with(
        &self,
        wanted: impl Fn(&Capabilities) -> bool,
    ) -> Option<&dyn Adapter> {
        self.iter().find(|adapter| {
            wanted(&adapter.capabilities()) && matches!(adapter.detect(), Ok(d) if d.is_usable())
        })
    }

    /// The backend a scan would actually run on.
    ///
    /// Registration order is preference order, so a machine with several
    /// scanners uses the one registered first.
    pub fn first_scanner(&self) -> Option<&dyn Adapter> {
        self.first_usable_with(|caps| caps.scan)
    }
}
