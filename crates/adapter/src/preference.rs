//! Which backend runs an operation.
//!
//! Registration order used to answer this. It cannot any more, for two reasons
//! that arrived together: the backends stopped agreeing on what they can do, and
//! the right default stopped being the same on every platform. Mole is the
//! better tool on macOS and cannot scan; ncdu scans everywhere and cleans
//! nothing; gdu is the primary Windows scanner.
//!
//! So selection is two separate questions, asked in this order:
//!
//! 1. **What did the user pick?** A [`Preference`] naming a backend id, or
//!    nothing, which means "whatever this platform defaults to".
//! 2. **Can it do the thing being asked?** A preference is a preference, not an
//!    override of reality. Choosing Mole on macOS does not make Mole a scanner —
//!    it makes Mole the backend for cleanup, while ncdu still scans, and
//!    [`Choice::instead_of`] carries the fact so the UI can say so.
//!
//! The alternative — honouring the choice and failing the scan — would be a user
//! picking their preferred tool and watching the app stop working.
//!
//! # No `#[cfg]` here
//!
//! The platform defaults are matched on [`std::env::consts::OS`] at runtime
//! rather than compiled per target. It costs a string compare that is never on a
//! hot path, and it buys the thing that matters: every platform's default is
//! testable from every platform, so the Windows ordering is covered by CI on
//! Linux and macOS rather than only by the one job that runs on Windows.

use serde::{Deserialize, Serialize};

use crate::Capabilities;

/// One thing a caller can ask a backend to do.
///
/// A named enum rather than the `Fn(&Capabilities) -> bool` closure this
/// replaces, because a fallback has to be explainable: "ncdu ran this instead"
/// is only useful next to *what* was being asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Ability {
    Scan,
    Delete,
    Trash,
    Undo,
    DryRun,
    CleanupCategories,
    CleanupPreview,
    UninstallApps,
    SystemStatus,
}

impl Ability {
    /// Wording for an error message or a tooltip, not an identifier.
    pub fn name(self) -> &'static str {
        match self {
            Self::Scan => "scan",
            Self::Delete => "delete",
            Self::Trash => "move to Trash",
            Self::Undo => "undo deletion",
            Self::DryRun => "dry run",
            Self::CleanupCategories => "cleanup categories",
            Self::CleanupPreview => "cleanup preview",
            Self::UninstallApps => "uninstall applications",
            Self::SystemStatus => "system status",
        }
    }

    pub fn is_offered_by(self, caps: &Capabilities) -> bool {
        match self {
            Self::Scan => caps.scan,
            Self::Delete => caps.delete,
            Self::Trash => caps.trash,
            Self::Undo => caps.undo,
            Self::DryRun => caps.dry_run,
            Self::CleanupCategories => caps.cleanup_categories,
            Self::CleanupPreview => caps.cleanup_categories && caps.dry_run,
            Self::UninstallApps => caps.uninstall_apps,
            Self::SystemStatus => caps.system_status,
        }
    }
}

/// Backend ids in preference order for one platform, best first.
///
/// Every list names every backend. A platform that omitted one would silently
/// stop offering it the moment the user's chosen backend could not do something,
/// which is the opposite of what a default is for.
///
/// Every list names every shipped backend, so a user preference can always fall
/// through to a platform-appropriate scanner.
pub fn default_order_for(os: &str) -> &'static [&'static str] {
    match os {
        // Mole is macOS-only and does the things nothing else here can.
        "macos" => &["mole", "rip", "ncdu", "gdu"],
        // gdu is the backend Windows users realistically have; ncdu is a
        // distant second there.
        "windows" => &["gdu", "rip", "ncdu", "mole"],
        // ncdu is packaged everywhere else. Mole trails because it refuses to
        // install off macOS at all.
        _ => &["ncdu", "rip", "gdu", "mole"],
    }
}

/// The default order for the machine this is running on.
pub fn default_order() -> &'static [&'static str] {
    default_order_for(std::env::consts::OS)
}

/// The backend the user picked.
///
/// `None` is a real answer and the default one — it means "follow the platform
/// default", and it keeps following it when the defaults change in a later
/// release. A first run that eagerly wrote `mole` into a settings file would
/// freeze today's guess into every future upgrade.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preference {
    pub chosen: Option<String>,
}

impl Preference {
    pub fn of(id: impl Into<String>) -> Self {
        Self {
            chosen: Some(id.into()),
        }
    }

    /// Follow the platform default.
    pub fn platform_default() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_platform_prefers_the_backend_it_actually_has() {
        assert_eq!(default_order_for("macos")[0], "mole");
        assert_eq!(default_order_for("windows")[0], "gdu");
        assert_eq!(default_order_for("linux")[0], "ncdu");
        // An OS nobody has thought about gets the cross-platform scanner rather
        // than a macOS-only tool that will not install.
        assert_eq!(default_order_for("freebsd")[0], "ncdu");
    }

    /// A shorter list would mean a platform quietly refusing a backend it has,
    /// which is a worse failure than a bad ordering: the user's own choice would
    /// still work, but every fallback would stop one entry early.
    #[test]
    fn every_default_names_every_backend() {
        for os in ["macos", "windows", "linux", "freebsd"] {
            let order = default_order_for(os);
            let mut sorted = order.to_vec();
            sorted.sort_unstable();
            sorted.dedup();

            assert_eq!(sorted, vec!["gdu", "mole", "ncdu", "rip"], "{os}");
            assert_eq!(sorted.len(), order.len(), "{os} names one twice");
        }
    }

    #[test]
    fn the_running_platform_has_a_default_like_any_other() {
        assert!(!default_order().is_empty());
        assert_eq!(default_order(), default_order_for(std::env::consts::OS));
    }

    #[test]
    fn abilities_read_the_flag_they_are_named_after() {
        let ncdu = Capabilities::MINIMAL;
        assert!(Ability::Scan.is_offered_by(&ncdu));
        assert!(!Ability::Delete.is_offered_by(&ncdu));
        assert!(!Ability::DryRun.is_offered_by(&ncdu));
        assert!(!Ability::Trash.is_offered_by(&ncdu));

        let mole = Capabilities {
            scan: false,
            delete: true,
            trash: false,
            undo: false,
            dry_run: true,
            cleanup_categories: true,
            uninstall_apps: true,
            system_status: true,
        };
        assert!(!Ability::Scan.is_offered_by(&mole));
        assert!(Ability::CleanupCategories.is_offered_by(&mole));
        assert!(Ability::CleanupPreview.is_offered_by(&mole));
        assert!(Ability::UninstallApps.is_offered_by(&mole));
        assert!(Ability::SystemStatus.is_offered_by(&mole));
    }

    #[test]
    fn no_preference_is_the_default_and_means_the_platform_default() {
        assert_eq!(Preference::default(), Preference::platform_default());
        assert!(Preference::default().chosen.is_none());
        assert_eq!(Preference::of("mole").chosen.as_deref(), Some("mole"));
    }

    /// The preference is written to a settings file, so its spelling is a
    /// compatibility surface: a rename would silently reset every user's choice.
    #[test]
    fn a_preference_round_trips_through_its_stored_form() {
        let json = serde_json::to_string(&Preference::of("mole")).unwrap();
        assert_eq!(json, r#"{"chosen":"mole"}"#);
        assert_eq!(
            serde_json::from_str::<Preference>(&json).unwrap(),
            Preference::of("mole")
        );

        assert_eq!(
            serde_json::to_string(&Preference::default()).unwrap(),
            r#"{"chosen":null}"#
        );
    }
}
