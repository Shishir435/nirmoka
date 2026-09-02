//! A set of adapters, assembled by the binary that owns `main`.
//!
//! The registry is push-based rather than self-populating so that this crate
//! never depends on a concrete adapter — that would be a dependency cycle.
//! `nirmoka-cli` builds one today; the Tauri app builds the same one. Both must
//! produce identical results, which is what the contract test suite checks.
//!
//! **Registration order is no longer preference order.** It is the last
//! tiebreak, reached only by a backend that no platform default names. What
//! picks a backend is [`Registry::resolve`] — the user's choice first, then the
//! platform default, and at every step only among backends that can do the thing
//! being asked. See [`crate::preference`].

use std::fmt;

use crate::preference::{default_order, Ability, Preference};
use crate::{Adapter, AdapterError, Capabilities, Detection};

#[derive(Default)]
pub struct Registry {
    adapters: Vec<Box<dyn Adapter>>,
}

/// One adapter's detection outcome, ready for display or serialisation.
#[derive(Debug)]
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

/// The backend that will run an operation, and who was asked for.
pub struct Choice<'a> {
    pub adapter: &'a dyn Adapter,

    /// The backend the user picked, when it is not the one that will run.
    ///
    /// `None` when the user got what they chose, or expressed no choice. `Some`
    /// is not an error — it is the honest half of honouring a preference that
    /// cannot cover everything. A user who picks Mole on macOS should be told
    /// that ncdu scanned, rather than left to conclude the setting did nothing.
    pub instead_of: Option<String>,
}

impl fmt::Debug for Registry {
    /// The ids, not the adapters. `dyn Adapter` is behaviour rather than data,
    /// and requiring `Debug` on the trait would put the bound on every
    /// implementor to print something no one wants to read.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registry")
            .field(
                "adapters",
                &self.adapters.iter().map(|a| a.id()).collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl fmt::Debug for Choice<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Choice")
            .field("adapter", &self.adapter.id())
            .field("instead_of", &self.instead_of)
            .finish()
    }
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

    /// A registered adapter by stable id.
    ///
    /// Used after a one-time destructive confirmation: the operation must run
    /// through the exact adapter that prepared it, even if preferences change
    /// while the dialog is open.
    pub fn by_id(&self, id: &str) -> Option<&dyn Adapter> {
        self.iter().find(|adapter| adapter.id() == id)
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

    /// The backend that will run `ability`, given what the user picked.
    ///
    /// Three passes, each narrower than the last and each filtered by whether
    /// the adapter can actually do the thing:
    ///
    /// 1. The user's choice. A preference is honoured whenever it can be.
    /// 2. The platform default order, best first.
    /// 3. Registration order, for a backend no default names.
    ///
    /// Pass 3 exists so that adding an adapter without touching
    /// [`default_order_for`](crate::preference::default_order_for) makes it
    /// reachable rather than invisible. A new backend that nobody can select is
    /// a worse failure than one in the wrong position.
    ///
    /// Returns `None` when nothing installed can do it — never a backend that
    /// would answer [`AdapterError::Unsupported`], and never one at an untested
    /// version.
    pub fn resolve(&self, ability: Ability, preference: &Preference) -> Option<Choice<'_>> {
        self.resolve_in_order(ability, preference, default_order())
    }

    fn resolve_in_order<'a>(
        &'a self,
        ability: Ability,
        preference: &Preference,
        default_order: &[&str],
    ) -> Option<Choice<'a>> {
        let capable = |adapter: &&dyn Adapter| {
            ability.is_offered_by(&adapter.capabilities()) && Self::is_usable(*adapter)
        };

        if let Some(id) = preference.chosen.as_deref() {
            if let Some(adapter) = self.iter().find(|a| a.id() == id).filter(capable) {
                return Some(Choice {
                    adapter,
                    instead_of: None,
                });
            }
        }

        let adapter = default_order
            .iter()
            .find_map(|id| self.iter().find(|a| a.id() == *id).filter(capable))
            .or_else(|| self.iter().find(capable))?;

        Some(Choice {
            // Set only when a choice was made and not met. Passes 2 and 3 can
            // never land on the chosen backend — pass 1 would have taken it.
            instead_of: preference.chosen.clone(),
            adapter,
        })
    }

    /// The backend a scan would actually run on.
    pub fn scanner(&self, preference: &Preference) -> Option<Choice<'_>> {
        self.resolve(Ability::Scan, preference)
    }

    /// The first registered adapter that is installed at a supported version.
    ///
    /// Answers "is anything installed at all", which is a different question
    /// from "what will run this" and is only ever the right one for a status
    /// line. Anything about to *do* something wants [`Registry::resolve`].
    pub fn first_usable(&self) -> Option<&dyn Adapter> {
        self.iter().find(|adapter| Self::is_usable(*adapter))
    }

    fn is_usable(adapter: &dyn Adapter) -> bool {
        matches!(adapter.detect(), Ok(detection) if detection.is_usable())
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::preference::default_order_for;
    use crate::{CancelToken, ScanOptions, ScanSummary, WireSink};

    /// An adapter with dictated detection and capabilities.
    ///
    /// The real ones shell out, so a registry test that used them would assert
    /// against whatever happens to be installed on the machine running it.
    struct Fake {
        id: &'static str,
        installed: bool,
        caps: Capabilities,
    }

    impl Fake {
        fn scanner(id: &'static str) -> Self {
            Self {
                id,
                installed: true,
                caps: Capabilities::MINIMAL,
            }
        }

        fn cleaner(id: &'static str) -> Self {
            Self {
                id,
                installed: true,
                caps: Capabilities {
                    scan: false,
                    delete: true,
                    undo: false,
                    cleanup_categories: true,
                    ..Capabilities::MINIMAL
                },
            }
        }

        fn deleter(id: &'static str) -> Self {
            Self {
                id,
                installed: true,
                caps: Capabilities {
                    scan: false,
                    delete: true,
                    trash: true,
                    undo: true,
                    ..Capabilities::MINIMAL
                },
            }
        }

        fn missing(mut self) -> Self {
            self.installed = false;
            self
        }
    }

    impl Adapter for Fake {
        fn id(&self) -> &'static str {
            self.id
        }
        fn display_name(&self) -> &'static str {
            self.id
        }
        fn supported_versions(&self) -> &'static str {
            "*"
        }
        fn detect(&self) -> Result<Detection, AdapterError> {
            Ok(if self.installed {
                Detection::Found {
                    path: PathBuf::from("/fake").join(self.id),
                    version: "1.0.0".to_string(),
                }
            } else {
                Detection::NotInstalled
            })
        }
        fn capabilities(&self) -> Capabilities {
            self.caps
        }
        fn scan(
            &self,
            _root: &Path,
            _options: &ScanOptions,
            _sink: &mut dyn WireSink,
            _cancel: &CancelToken,
        ) -> Result<ScanSummary, AdapterError> {
            unreachable!("resolution never scans")
        }
    }

    fn registry_of(adapters: Vec<Fake>) -> Registry {
        let mut registry = Registry::new();
        for adapter in adapters {
            registry.register(Box::new(adapter));
        }
        registry
    }

    /// Shipped backends in the order every binary registers them. Resolution
    /// must follow platform preference, not this order.
    fn as_shipped() -> Registry {
        registry_of(vec![
            Fake::scanner("ncdu"),
            Fake::cleaner("mole"),
            Fake::scanner("gdu"),
            Fake::deleter("rip"),
        ])
    }

    #[test]
    fn a_chosen_backend_that_can_do_the_job_gets_it() {
        let registry = as_shipped();
        let choice = registry
            .resolve(Ability::CleanupCategories, &Preference::of("mole"))
            .expect("mole cleans");

        assert_eq!(choice.adapter.id(), "mole");
        assert!(choice.instead_of.is_none(), "the choice was honoured");
    }

    /// The case this whole module exists for.
    ///
    /// Choosing Mole must not break scanning, and must not silently pretend the
    /// choice was met either.
    #[test]
    fn a_chosen_backend_that_cannot_scan_does_not_stop_the_scan() {
        let registry = as_shipped();
        for (os, expected) in platform_scanners() {
            let choice = registry
                .resolve_in_order(
                    Ability::Scan,
                    &Preference::of("mole"),
                    default_order_for(os),
                )
                .expect("something still scans");

            assert_eq!(choice.adapter.id(), expected, "{os}");
            assert_eq!(
                choice.instead_of.as_deref(),
                Some("mole"),
                "the UI has to be able to say who was asked for on {os}"
            );
        }
    }

    #[test]
    fn a_backend_that_is_not_installed_is_fallen_back_from() {
        let registry = registry_of(vec![Fake::scanner("ncdu"), Fake::scanner("gdu").missing()]);
        let choice = registry
            .scanner(&Preference::of("gdu"))
            .expect("ncdu is there");

        assert_eq!(choice.adapter.id(), "ncdu");
        assert_eq!(choice.instead_of.as_deref(), Some("gdu"));
    }

    /// A settings file is editable, and an id in it may name nothing at all.
    #[test]
    fn a_choice_naming_no_registered_backend_falls_through_rather_than_failing() {
        let registry = as_shipped();
        for (os, expected) in platform_scanners() {
            let choice = registry
                .resolve_in_order(
                    Ability::Scan,
                    &Preference::of("not-a-backend"),
                    default_order_for(os),
                )
                .expect("the default still applies");

            assert_eq!(choice.adapter.id(), expected, "{os}");
            assert_eq!(choice.instead_of.as_deref(), Some("not-a-backend"), "{os}");
        }
    }

    fn platform_scanners() -> [(&'static str, &'static str); 4] {
        [
            ("macos", "ncdu"),
            ("windows", "gdu"),
            ("linux", "ncdu"),
            ("freebsd", "ncdu"),
        ]
    }

    #[test]
    fn every_platform_default_selects_the_right_delete_backend() {
        let registry = as_shipped();
        for (os, expected) in [
            ("macos", "mole"),
            ("windows", "rip"),
            ("linux", "rip"),
            ("freebsd", "rip"),
        ] {
            let choice = registry
                .resolve_in_order(
                    Ability::Delete,
                    &Preference::platform_default(),
                    default_order_for(os),
                )
                .expect("mole and rip can delete");

            assert_eq!(choice.adapter.id(), expected, "{os}");
            assert!(
                choice.instead_of.is_none(),
                "a default is not a fallback from anything on {os}"
            );
        }
    }

    /// Registration order is the last resort, not the first.
    ///
    /// `zzz` is named by no platform default, so only pass 3 can reach it.
    #[test]
    fn a_backend_no_default_names_is_still_reachable() {
        let registry = registry_of(vec![Fake::scanner("zzz")]);

        let by_default = registry
            .scanner(&Preference::platform_default())
            .expect("something has to run");
        assert_eq!(by_default.adapter.id(), "zzz");

        let chosen = registry.scanner(&Preference::of("zzz")).expect("chosen");
        assert_eq!(chosen.adapter.id(), "zzz");
        assert!(chosen.instead_of.is_none());
    }

    #[test]
    fn nothing_capable_resolves_to_nothing_rather_than_to_a_backend_that_would_refuse() {
        let registry = registry_of(vec![Fake::cleaner("mole")]);

        assert!(
            registry.scanner(&Preference::platform_default()).is_none(),
            "a cleaner must never be handed a scan"
        );
        assert!(
            registry.scanner(&Preference::of("mole")).is_none(),
            "not even when it is the one the user picked"
        );
        assert!(
            registry.first_usable().is_some(),
            "it is installed — it just cannot scan"
        );
    }

    #[test]
    fn an_uninstalled_backend_is_never_resolved_to() {
        let registry = registry_of(vec![Fake::scanner("ncdu").missing()]);

        assert!(registry.scanner(&Preference::platform_default()).is_none());
        assert!(registry.first_usable().is_none());
    }

    #[test]
    fn an_empty_registry_answers_nothing_rather_than_panicking() {
        let registry = Registry::new();

        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        assert!(registry.scanner(&Preference::of("ncdu")).is_none());
    }
}
