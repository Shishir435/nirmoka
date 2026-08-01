//! The contract every adapter must satisfy, whatever backend it drives.
//!
//! Nothing here installs, requires, or runs a backend. These are the promises
//! an adapter makes before it has spoken to anything: stable identity, honest
//! capabilities, and validation that happens before a path can reach a
//! subprocess. Backend-specific behaviour is covered by fixtures in
//! `wire_format.rs` and by the live tests in each adapter crate.
//!
//! Adding an adapter means registering it in `nirmoka_contract_tests::registry`
//! and running this suite. If a backend needs a special case here, the trait is
//! probably wrong — that is the signal this suite exists to send.

use std::path::Path;

use nirmoka_adapter::{Adapter, AdapterError, CancelToken, Detection, ScanOptions, TreeSink};
use nirmoka_contract_tests::registry;

fn for_each_adapter(mut check: impl FnMut(&dyn Adapter)) {
    let registry = registry();
    assert!(!registry.is_empty(), "the registry must not be empty");
    for adapter in registry.iter() {
        check(adapter);
    }
}

/// Only the adapters that claim to scan.
///
/// The scan promises below — validate the root, honour a tripped token — are
/// promises about *performing* a scan. A backend that declares `scan: false`
/// keeps a different promise, checked by
/// [`a_backend_that_cannot_scan_refuses_every_scan`]: it must refuse, whatever
/// it is handed.
///
/// This is a capability branch rather than a special case. Nothing here names a
/// backend, and an adapter that flips the flag moves between the two sets
/// without either test changing.
fn for_each_scanner(mut check: impl FnMut(&dyn Adapter)) {
    let registry = registry();
    let mut scanners = 0;

    for adapter in registry.iter() {
        if adapter.capabilities().scan {
            scanners += 1;
            check(adapter);
        }
    }

    assert!(
        scanners > 0,
        "no registered backend can scan, which would make Nirmoka a disk browser with no disk"
    );
}

#[test]
fn identities_are_stable_machine_names() {
    // Ids end up in config files and logs, so they cannot be display strings.
    for_each_adapter(|adapter| {
        let id = adapter.id();
        assert!(!id.is_empty());
        assert!(
            id.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "{id} is not a stable machine identifier"
        );
        assert!(!adapter.display_name().is_empty());
        assert!(
            !adapter.supported_versions().is_empty(),
            "{id} claims no tested version range"
        );
    });
}

#[test]
fn ids_are_unique() {
    let registry = registry();
    let mut ids: Vec<&str> = registry.iter().map(|adapter| adapter.id()).collect();
    let count = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), count, "two adapters share an id");
}

#[test]
fn capabilities_are_internally_coherent() {
    for_each_adapter(|adapter| {
        let caps = adapter.capabilities();
        let id = adapter.id();

        // An adapter must offer at least one real operation. No single flag is
        // the floor: ncdu scans but cannot script its interactive deletion;
        // Mole cleans categories and uninstalls apps but cannot scan or remove
        // an arbitrary caller-selected path.
        assert!(
            caps.scan
                || caps.delete
                || caps.undo
                || caps.cleanup_categories
                || caps.uninstall_apps
                || caps.system_status,
            "{id} declares no usable operation"
        );

        // Every removal mode is a way of deleting, so claiming one without the
        // other is a flag that will fail at call time.
        if caps.trash {
            assert!(caps.delete, "{id} offers Trash without delete");
        }
        // Undo may remain available for durable receipts created by an older
        // release after new deletion is withdrawn for safety.
    });
}

#[test]
fn detection_reports_a_state_it_can_defend() {
    // Runs everywhere, including where the backend is missing: all three
    // outcomes are valid, but each one has to be self-consistent.
    for_each_adapter(|adapter| {
        let id = adapter.id();
        match adapter.detect() {
            Ok(Detection::Found { path, version }) => {
                assert!(!version.is_empty(), "{id} reported an empty version");
                assert!(
                    path.is_absolute(),
                    "{id} reported {} rather than the binary that will run",
                    path.display()
                );
                assert!(path.exists(), "{id} reported a path that does not exist");
            }
            Ok(Detection::UnsupportedVersion {
                version, supported, ..
            }) => {
                assert!(!version.is_empty());
                assert!(!supported.is_empty());
                assert_eq!(supported, adapter.supported_versions());
            }
            Ok(Detection::NotInstalled) => {}
            // A backend that fails detection must not take the sweep down with
            // it; the registry is required to keep going, and so is this.
            Err(error) => {
                assert!(!error.to_string().is_empty(), "{id} failed opaquely");
            }
        }
    });
}

#[test]
fn only_a_found_backend_counts_as_usable() {
    // A detection that failed outright says nothing about usability, so there
    // is nothing to check on that arm.
    for_each_adapter(|adapter| {
        if let Ok(detection) = adapter.detect() {
            assert_eq!(
                detection.is_usable(),
                matches!(detection, Detection::Found { .. }),
                "{} treats a non-Found state as usable",
                adapter.id()
            );
        }
    });
}

#[test]
fn the_registry_agrees_with_the_adapters_it_holds() {
    let registry = registry();
    let entries = registry.detect_all();
    assert_eq!(entries.len(), registry.len());

    let usable_by_sweep = entries
        .iter()
        .any(|entry| matches!(&entry.detection, Ok(detection) if detection.is_usable()));

    assert_eq!(
        usable_by_sweep,
        registry.first_usable().is_some(),
        "detect_all and first_usable disagree about whether anything is usable"
    );
}

/// Resolution never hands a backend a job it has said it cannot do.
///
/// The promise the whole picker rests on, and the reason a preference is not an
/// override: a user may choose any backend for any reason, and the resolver has
/// to keep that from becoming an `Unsupported` at call time. Asserted across
/// every ability and every possible choice, including ids that name nothing.
///
/// Machine-independent: what is installed changes which arm runs, not whether
/// the promise holds. On a machine with no backend at all, every resolution is
/// `None`, which is also a pass — `None` disables a control instead of offering
/// one that fails.
#[test]
fn a_resolved_backend_can_always_do_what_it_was_resolved_for() {
    use nirmoka_adapter::{Ability, Preference};

    const ABILITIES: [Ability; 9] = [
        Ability::Scan,
        Ability::Delete,
        Ability::Trash,
        Ability::Undo,
        Ability::DryRun,
        Ability::CleanupCategories,
        Ability::CleanupPreview,
        Ability::UninstallApps,
        Ability::SystemStatus,
    ];

    let registry = registry();

    let mut choices: Vec<Preference> = registry.iter().map(|a| Preference::of(a.id())).collect();
    choices.push(Preference::platform_default());
    choices.push(Preference::of("not-a-backend"));

    for ability in ABILITIES {
        for preference in &choices {
            let Some(choice) = registry.resolve(ability, preference) else {
                continue;
            };

            assert!(
                ability.is_offered_by(&choice.adapter.capabilities()),
                "{} was resolved for {} and does not offer it",
                choice.adapter.id(),
                ability.name()
            );
            assert!(
                matches!(choice.adapter.detect(), Ok(d) if d.is_usable()),
                "{} was resolved and is not usable",
                choice.adapter.id()
            );

            // A fallback must name who it displaced, and must never claim to
            // have displaced the backend it actually is.
            match (&preference.chosen, &choice.instead_of) {
                (Some(asked), Some(displaced)) => {
                    assert_eq!(asked, displaced);
                    assert_ne!(displaced, choice.adapter.id());
                }
                (Some(asked), None) => assert_eq!(asked, choice.adapter.id()),
                (None, displaced) => assert!(
                    displaced.is_none(),
                    "a default is not a fallback from anything"
                ),
            }
        }
    }
}

#[test]
fn a_scan_root_that_does_not_exist_is_refused() {
    // Validation happens at the adapter boundary, before a path can become a
    // subprocess argument — so this must hold even where no backend exists.
    for_each_scanner(|adapter| {
        let mut sink = TreeSink::new();
        let error = adapter
            .scan(
                Path::new("nirmoka-contract-no-such-directory"),
                &ScanOptions::default(),
                &mut sink,
                &CancelToken::new(),
            )
            .expect_err("a missing scan root must be refused");

        assert!(
            matches!(error, AdapterError::RefusedPath { .. }),
            "{} reported {error} instead of refusing the path",
            adapter.id()
        );
    });
}

#[test]
fn a_file_is_not_a_scan_root() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");

    for_each_scanner(|adapter| {
        let mut sink = TreeSink::new();
        let error = adapter
            .scan(
                &file,
                &ScanOptions::default(),
                &mut sink,
                &CancelToken::new(),
            )
            .expect_err("a file must not be accepted as a scan root");

        assert!(
            matches!(error, AdapterError::RefusedPath { .. }),
            "{} reported {error} instead of refusing a file",
            adapter.id()
        );
    });
}

#[test]
fn an_already_cancelled_scan_does_not_run() {
    // The stop button can be pressed before the backend is even reached. The
    // result must be a cancellation, never a partial success.
    let cancel = CancelToken::new();
    cancel.cancel();

    for_each_scanner(|adapter| {
        let mut sink = TreeSink::new();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));

        match adapter.scan(root, &ScanOptions::default(), &mut sink, &cancel) {
            Ok(summary) => panic!(
                "{} completed a cancelled scan of {} entries",
                adapter.id(),
                summary.items
            ),
            Err(error) => assert!(
                error.is_cancellation()
                    // A machine with no usable backend legitimately fails
                    // earlier than the cancellation check.
                    || matches!(
                        error,
                        AdapterError::NotInstalled { .. } | AdapterError::UnsupportedVersion { .. }
                    ),
                "{} reported {error} for a cancelled scan",
                adapter.id()
            ),
        }
    });
}

/// The other half of the capability split: a backend that says it cannot scan
/// has to actually refuse.
///
/// "Degrade, don't lie" is only worth writing down if something checks it. The
/// failure this prevents is the quiet one — an adapter that declares
/// `scan: false` and then returns an empty tree, or a tree one level deep,
/// which reads on screen as a disk with nothing on it.
///
/// The root here is a real, readable directory, so nothing else can be doing
/// the refusing.
#[test]
fn a_backend_that_cannot_scan_refuses_every_scan() {
    let real_directory = Path::new(env!("CARGO_MANIFEST_DIR"));

    for_each_adapter(|adapter| {
        if adapter.capabilities().scan {
            return;
        }

        let mut sink = TreeSink::new();
        let error = match adapter.scan(
            real_directory,
            &ScanOptions::default(),
            &mut sink,
            &CancelToken::new(),
        ) {
            Ok(summary) => panic!(
                "{} declares scan: false and then returned a scan of {} entries",
                adapter.id(),
                summary.items
            ),
            Err(error) => error,
        };

        assert!(
            matches!(error, AdapterError::Unsupported { .. }),
            "{} declares scan: false but reported {error}, which does not tell a \
             caller the ability is missing rather than broken",
            adapter.id()
        );
    });
}
