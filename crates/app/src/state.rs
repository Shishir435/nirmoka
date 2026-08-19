//! Everything the shell remembers between commands.
//!
//! The tree lives here, in Rust, and never crosses to the webview (invariant 5).
//! A scan of a home directory is millions of nodes; the frontend asks for the
//! window it is about to paint and gets exactly that.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use nirmoka_adapter::registry::RegistryEntry;
use nirmoka_adapter::{Ability, CancelToken, Choice, Preference, Registry};
use nirmoka_adapter_gdu::GduAdapter;
use nirmoka_adapter_mole::MoleAdapter;
use nirmoka_adapter_ncdu::NcduAdapter;
use nirmoka_adapter_rip::RipAdapter;
use nirmoka_core::Tree;

use crate::cleanup::CleanupState;
use crate::deletion::DeletionState;
use crate::uninstall::UninstallState;
use crate::{dto, settings};

/// The registry every entry point builds.
///
/// `nirmoka-cli` and `tests/contract` build the same one. They must agree — a
/// backend that works in the CLI and is missing in the app would be found by
/// nobody until a user reported it.
///
/// Order here is *registration* order, which is the last tiebreak and not the
/// preference. What picks a backend is `Registry::resolve`: the user's choice,
/// then the platform default. Adding an adapter at the top of this list changes
/// nothing about which one runs.
pub fn registry() -> Registry {
    let mut registry = Registry::new();
    registry.register(Box::new(NcduAdapter::new()));
    registry.register(Box::new(MoleAdapter::new()));
    registry.register(Box::new(GduAdapter::new()));
    registry.register(Box::new(RipAdapter::new()));
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
    /// The user's backend choice, loaded once at startup and written back on
    /// every change. Held here rather than re-read per call because a settings
    /// file read on the path of every command would be a file system round trip
    /// to answer a question whose answer this process already owns.
    preference: Mutex<Preference>,
    /// Whether the choice is being persisted. False when this machine has no
    /// configuration directory, which the UI says out loud rather than letting
    /// the setting silently evaporate on quit.
    persistent: bool,
    cleanup: Mutex<CleanupState>,
    uninstall: Mutex<UninstallState>,
    deletion: Mutex<DeletionState>,
}

impl AppState {
    pub fn new() -> Self {
        Self::with_parts(
            settings::load(),
            settings::settings_path().is_some(),
            registry(),
            settings::operation_log_path(),
        )
    }

    /// A state with a dictated preference and no persistence.
    ///
    /// Tests use this so they assert against a preference they set rather than
    /// against whatever the developer running them happens to have chosen.
    pub fn with_preference(preference: Preference, persistent: bool) -> Self {
        Self::with_parts(preference, persistent, registry(), None)
    }

    pub(crate) fn with_parts(
        preference: Preference,
        persistent: bool,
        registry: Registry,
        operation_log: Option<PathBuf>,
    ) -> Self {
        Self {
            registry,
            scan: Mutex::new(ScanState::default()),
            preference: Mutex::new(preference),
            persistent,
            cleanup: Mutex::new(CleanupState::default()),
            uninstall: Mutex::new(UninstallState::default()),
            deletion: Mutex::new(DeletionState::new(operation_log)),
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

    pub fn deletion(&self) -> MutexGuard<'_, DeletionState> {
        self.deletion
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn cleanup(&self) -> MutexGuard<'_, CleanupState> {
        self.cleanup
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn uninstall(&self) -> MutexGuard<'_, UninstallState> {
        self.uninstall
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn adapter(&self, id: &str) -> Option<&dyn nirmoka_adapter::Adapter> {
        self.registry.by_id(id)
    }

    /// The backend the user picked, or `None` for the platform default.
    pub fn preference(&self) -> Preference {
        self.preference
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Whether a change to the preference outlives this process.
    pub fn is_persistent(&self) -> bool {
        self.persistent
    }

    /// Record a backend choice, and write it down.
    ///
    /// The in-memory choice is updated even when the write fails, so a machine
    /// with no writable configuration directory still honours the setting for
    /// the session. The error is returned rather than swallowed — a preference
    /// that appears to take and is gone next launch is worse than one that
    /// says it could not be saved.
    pub fn choose(&self, preference: Preference) -> Result<(), String> {
        *self
            .preference
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = preference.clone();

        if self.persistent {
            settings::save(&preference)
        } else {
            Ok(())
        }
    }

    /// The backend that will run `ability`, and who was asked for instead.
    ///
    /// Never a backend that would answer `Unsupported`, and never one at an
    /// untested version — see `docs/adapters.md`. A `None` here is the UI's cue
    /// to disable a control rather than to offer one that fails on click.
    pub fn resolve(&self, ability: Ability) -> Option<Choice<'_>> {
        self.registry.resolve(ability, &self.preference())
    }

    /// The backend a scan would actually run on.
    ///
    /// A separate question from "did the user pick a backend" since Mole joined
    /// the registry: Mole is usable on macOS, is the macOS default, and cannot
    /// scan. Picking it for a scan would put the button in front of an adapter
    /// that answers `Unsupported`.
    pub fn scanner(&self) -> Option<Choice<'_>> {
        self.resolve(Ability::Scan)
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

    fn state() -> AppState {
        AppState::with_preference(Preference::platform_default(), false)
    }

    #[test]
    fn the_registry_holds_the_backends_the_cli_reports() {
        let ids: Vec<_> = registry().iter().map(|adapter| adapter.id()).collect();
        assert_eq!(ids, vec!["ncdu", "mole", "gdu", "rip"]);
    }

    #[test]
    fn a_fresh_state_has_no_scan_and_no_result() {
        let state = state();
        let scan = state.scan();

        assert!(scan.active.is_none());
        assert!(scan.result.is_none());
    }

    #[test]
    fn a_state_starts_on_the_platform_default() {
        assert!(state().preference().chosen.is_none());
    }

    /// A choice with nowhere to be written still applies to this session.
    ///
    /// The alternative — refusing the change because it cannot be persisted —
    /// would make the picker dead on any machine without a config directory.
    #[test]
    fn a_choice_takes_effect_even_when_it_cannot_be_stored() {
        let state = state();
        assert!(!state.is_persistent());

        state
            .choose(Preference::of("mole"))
            .expect("no write, no error");
        assert_eq!(state.preference().chosen.as_deref(), Some("mole"));

        state
            .choose(Preference::platform_default())
            .expect("cleared");
        assert!(state.preference().chosen.is_none());
    }

    /// Whatever is installed on the machine running this, a resolved scanner is
    /// always one that says it can scan. That is the promise the button rests on.
    #[test]
    fn a_resolved_scanner_can_always_actually_scan() {
        let state = AppState::with_preference(Preference::of("mole"), false);

        if let Some(choice) = state.scanner() {
            assert!(
                choice.adapter.capabilities().scan,
                "{} was handed a scan it cannot do",
                choice.adapter.id()
            );
            assert_ne!(
                choice.adapter.id(),
                "mole",
                "mole must never be resolved for a scan, even when chosen"
            );
        }
    }
}
