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
use nirmoka_adapter::{Preference, Registry};
use nirmoka_adapter_gdu::GduAdapter;
use nirmoka_adapter_mole::MoleAdapter;
use nirmoka_adapter_ncdu::NcduAdapter;
use nirmoka_adapter_rip::RipAdapter;

#[derive(Parser)]
#[command(
    name = "nrmk",
    version,
    about = "Headless driver for Nirmoka core (development harness)",
    long_about = None
)]
struct Cli {
    /// Prefer this backend where it can do the job.
    ///
    /// A preference, not an override: a backend that cannot do what is being
    /// asked is fallen back from, with a note on stderr saying so. Without the
    /// flag the platform default applies.
    ///
    /// The GUI stores its own choice in a settings file. `nrmk` deliberately
    /// does not read it — a harness that inherited a developer's preferences
    /// would reproduce their machine rather than the default one. See ADR 0007
    /// on why this binary is not a product.
    #[arg(long, global = true, value_name = "ID")]
    backend: Option<String>,

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

    /// Scan a directory with whichever backend is selected for scanning.
    Scan(scan::ScanArgs),
}

/// The registry, in registration order.
///
/// Registration order is *not* preference order — it is the last tiebreak,
/// reached only by a backend no platform default names. `Registry::resolve`
/// picks. See `crates/adapter/src/preference.rs`.
///
/// Both this and the Tauri app must build an identical registry; the contract
/// test suite checks that they agree.
pub fn build_registry() -> Registry {
    let mut registry = Registry::new();
    registry.register(Box::new(NcduAdapter::new()));
    registry.register(Box::new(MoleAdapter::new()));
    registry.register(Box::new(GduAdapter::new()));
    registry.register(Box::new(RipAdapter::new()));
    registry
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    let registry = build_registry();
    let preference = Preference {
        chosen: cli.backend,
    };

    match cli.command {
        Command::Backends { json } => backends::run(json, &registry, &preference),
        Command::Scan(args) => scan::run(args, &registry, &preference),
    }
}
