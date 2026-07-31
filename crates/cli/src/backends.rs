//! `nrmk backends` — what is installed, and is it usable?

use std::process::ExitCode;

use nirmoka_adapter::{default_order, Capabilities, Detection, Preference, Registry};
use serde::Serialize;

#[derive(Serialize)]
struct BackendReport {
    id: &'static str,
    #[serde(rename = "displayName")]
    display_name: &'static str,
    #[serde(rename = "supportedVersions")]
    supported_versions: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    detection: Option<Detection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    /// Per backend, because they no longer agree: Mole cleans and cannot scan,
    /// ncdu scans and cannot preview. A single set of flags for the whole app
    /// would describe neither.
    capabilities: Capabilities,
}

pub fn run(json: bool, registry: &Registry, preference: &Preference) -> ExitCode {
    let entries = registry.detect_all();

    let reports: Vec<BackendReport> = entries
        .into_iter()
        .map(|entry| match entry.detection {
            Ok(detection) => BackendReport {
                id: entry.id,
                display_name: entry.display_name,
                supported_versions: entry.supported_versions,
                detection: Some(detection),
                error: None,
                capabilities: entry.capabilities,
            },
            Err(err) => BackendReport {
                id: entry.id,
                display_name: entry.display_name,
                supported_versions: entry.supported_versions,
                detection: None,
                error: Some(err.to_string()),
                capabilities: entry.capabilities,
            },
        })
        .collect();

    if json {
        // Unwrap is acceptable: these are plain owned structs with no
        // non-serialisable variants, so failure is not reachable.
        println!(
            "{}",
            serde_json::to_string_pretty(&reports).expect("BackendReport is serialisable")
        );
    } else {
        print_table(&reports);
        print_selection(registry, preference);
    }

    let any_usable = reports
        .iter()
        .any(|r| r.detection.as_ref().is_some_and(Detection::is_usable));

    if any_usable {
        ExitCode::SUCCESS
    } else {
        // Non-zero so CI and scripts can tell "no backend" from "found one".
        ExitCode::FAILURE
    }
}

/// Which backend a scan would use, printed under the table.
///
/// A footer rather than a column, for two reasons. The table answers "what is
/// installed" and this answers "what will run", which are different questions
/// that stopped having the same answer once backends stopped agreeing on what
/// they can do. And a column would move the DETAIL field again — CI asserts
/// against these rows, and a table whose shape shifts every step is a set of
/// assertions nobody trusts.
fn print_selection(registry: &Registry, preference: &Preference) {
    println!();

    match registry.scanner(preference) {
        Some(choice) => {
            println!("SCANS WITH  {}", choice.adapter.id());
            // Deliberately states the fact and not the reason. "cannot scan" and
            // "is not installed" are both possible here and the difference is
            // already in the table above — asserting one of them would be a
            // guess printed as a finding.
            if let Some(asked_for) = &choice.instead_of {
                println!("PREFERRED   {asked_for}, which is not running this — see its row above");
            }
        }
        None => println!("SCANS WITH  nothing installed can scan"),
    }

    // The default is worth showing even when a choice overrides it: it is what
    // clearing the choice goes back to, and it differs per platform.
    println!("DEFAULT     {}", default_order().join(", "));
}

fn print_table(reports: &[BackendReport]) {
    // Column widths here must match the row format at the bottom of this fn.
    // DETAIL is inlined rather than passed as an argument because it is the
    // last column and has no width spec (clippy::print_literal).
    println!(
        "{:<10} {:<12} {:<10} {:<10} DETAIL",
        "BACKEND", "STATE", "VERSION", "SCANS"
    );

    for report in reports {
        let (state, version, detail) = match (&report.detection, &report.error) {
            (Some(Detection::Found { version, path }), _) => {
                ("ok", version.as_str(), format!("{}", path.display()))
            }
            (Some(Detection::UnsupportedVersion { version, .. }), _) => (
                "unsupported",
                version.as_str(),
                format!("needs {}", report.supported_versions),
            ),
            (Some(Detection::NotInstalled), _) => ("missing", "-", String::new()),
            (None, Some(error)) => ("error", "-", error.clone()),
            (None, None) => ("error", "-", "no detection result".to_string()),
        };

        // "ok" alone would hide the thing a user most needs to know about a
        // backend that is installed and still cannot answer `nrmk scan`.
        let scans = if report.capabilities.scan {
            "yes"
        } else {
            "no"
        };

        println!(
            "{:<10} {:<12} {:<10} {:<10} {}",
            report.id, state, version, scans, detail
        );
    }
}
