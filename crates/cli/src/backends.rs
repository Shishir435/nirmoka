//! `nrmk backends` — what is installed, and is it usable?

use std::process::ExitCode;

use nirmoka_adapter::{Capabilities, Detection, Registry};
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

pub fn run(json: bool, registry: &Registry) -> ExitCode {
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
