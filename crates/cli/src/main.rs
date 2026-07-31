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

mod backends;
mod scan;

use clap::{Parser, Subcommand};
use nirmoka_adapter::Registry;
use nirmoka_adapter_ncdu::NcduAdapter;

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

    /// Scan a directory with the first usable backend.
    Scan(scan::ScanArgs),
}

/// Registration order is preference order.
///
/// Both this and the Tauri app must build an identical registry; the contract
/// test suite checks that they agree.
pub fn build_registry() -> Registry {
    let mut registry = Registry::new();
    registry.register(Box::new(NcduAdapter::new()));
    registry
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    let registry = build_registry();

    match cli.command {
        Command::Backends { json } => backends::run(json, &registry),
        Command::Scan(args) => scan::run(args, &registry),
    }
}
