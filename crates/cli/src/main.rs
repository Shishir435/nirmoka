//! `nrmk` — headless driver for Nirmoka core.
//!
//! # This is not a product
//!
//! `nrmk` is a development and CI harness. It is not published, not in
//! releases, and not documented in the user-facing README. Nirmoka's premise is
//! "a GUI for existing CLIs"; shipping a CLI that wraps a CLI would muddy that.
//! See `docs/adr/0007`.
//!
//! # Why it exists anyway
//!
//! It links `nirmoka-core` with no Tauri anywhere. If someone makes core depend
//! on a GUI framework, this binary stops building — the boundary is enforced by
//! the compiler instead of by a paragraph in a design document.
//!
//! It also lets CI exercise the whole stack with no display server, lets
//! adapters be debugged without launching a window, and means a broken GUI
//! leaves a working tool behind rather than nothing.

use clap::{Parser, Subcommand};
use nirmoka_adapter::{Detection, Registry};
use nirmoka_adapter_ncdu::NcduAdapter;
use serde::Serialize;

#[derive(Parser)]
#[command(
    name = "nrmk",
    version,
    about = "Headless driver for Nirmoka core (development harness)",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Report which disk backends are installed and usable.
    Backends {
        /// Emit JSON instead of a table.
        #[arg(long)]
        json: bool,
    },
}

/// Registration order is preference order.
///
/// Both this and the Tauri app must build an identical registry; the contract
/// test suite checks that they agree.
fn build_registry() -> Registry {
    let mut registry = Registry::new();
    registry.register(Box::new(NcduAdapter::new()));
    registry
}

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
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Command::Backends { json } => backends(json),
    }
}

fn backends(json: bool) -> std::process::ExitCode {
    let registry = build_registry();
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
            },
            Err(err) => BackendReport {
                id: entry.id,
                display_name: entry.display_name,
                supported_versions: entry.supported_versions,
                detection: None,
                error: Some(err.to_string()),
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
        std::process::ExitCode::SUCCESS
    } else {
        // Non-zero so CI and scripts can tell "no backend" from "found one".
        std::process::ExitCode::FAILURE
    }
}

fn print_table(reports: &[BackendReport]) {
    // Column widths here must match the row format at the bottom of this fn.
    // DETAIL is inlined rather than passed as an argument because it is the
    // last column and has no width spec (clippy::print_literal).
    println!("{:<10} {:<12} {:<10} DETAIL", "BACKEND", "STATE", "VERSION");

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

        println!("{:<10} {:<12} {:<10} {}", report.id, state, version, detail);
    }
}
