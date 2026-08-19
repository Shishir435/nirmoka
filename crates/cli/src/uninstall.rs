//! `nrmk uninstall` — read a backend's uninstall plan without a window.
//!
//! Preview only. This harness has no confirmation dialog and no journal, and an
//! uninstall without either is not something a development binary should be able
//! to start: the two are what make the operation reviewable and recorded, and
//! ADR 0027 rests on both. So the interesting half — does the backend produce a
//! plan, and does this parse it — is exercisable here, and the destructive half
//! stays in the shell.
//!
//! Which also means this is safe to run against anything installed.

use std::process::ExitCode;

use nirmoka_adapter::{Ability, CancelToken, Preference, Registry, UninstallItemScope};

#[derive(clap::Args)]
pub struct UninstallArgs {
    /// Backend identifiers to preview, as published by `--list`.
    ///
    /// Not display names. "Google Chrome" is listed; `google-chrome` is what the
    /// command takes, and the adapter refuses anything the backend did not just
    /// publish.
    #[arg(value_name = "NAME", required_unless_present = "list")]
    names: Vec<String>,

    /// List the applications the backend can address, with their identifiers.
    #[arg(long)]
    list: bool,

    /// Emit JSON instead of a table.
    #[arg(long)]
    json: bool,

    /// Print the backend's own output verbatim instead of the parsed plan.
    ///
    /// The parse is for rendering; this is the thing a user would be approving.
    #[arg(long)]
    transcript: bool,
}

pub fn run(args: UninstallArgs, registry: &Registry, preference: &Preference) -> ExitCode {
    let ability = if args.list {
        Ability::AppInventory
    } else {
        Ability::UninstallApps
    };
    let Some(choice) = registry.resolve(ability, preference) else {
        eprintln!("no usable backend can {}", ability.name());
        return ExitCode::FAILURE;
    };
    if let Some(instead_of) = &choice.instead_of {
        eprintln!(
            "note: {} cannot {}; using {} instead",
            instead_of,
            ability.name(),
            choice.adapter.id()
        );
    }

    let cancel = CancelToken::new();
    if args.list {
        return match choice.adapter.installed_applications(&cancel) {
            Ok(applications) if args.json => match serde_json::to_string_pretty(&applications) {
                Ok(json) => {
                    println!("{json}");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("could not serialize the inventory: {error}");
                    ExitCode::FAILURE
                }
            },
            Ok(applications) => {
                for application in &applications {
                    println!(
                        "{:<40}  {:>10}  {}",
                        truncate(&application.name, 40),
                        application.reported_size,
                        application.uninstall_name
                    );
                }
                println!("\n{} application(s)", applications.len());
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        };
    }

    let preview = match choice.adapter.uninstall_preview(&args.names, &cancel) {
        Ok(preview) => preview,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    if args.transcript {
        print!("{}", preview.transcript);
        return ExitCode::SUCCESS;
    }
    if args.json {
        return match serde_json::to_string_pretty(&preview) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("could not serialize the plan: {error}");
                ExitCode::FAILURE
            }
        };
    }

    println!(
        "{} {} — plan for {}\n",
        choice.adapter.display_name(),
        preview.backend_version,
        preview.requested.join(", ")
    );
    for app in &preview.apps {
        let cask = if app.homebrew_cask { " [Homebrew]" } else { "" };
        println!(
            "{}{}{}",
            app.name,
            cask,
            app.reported_size
                .as_deref()
                .map(|size| format!("  {size}"))
                .unwrap_or_default()
        );
        for item in &app.items {
            // The classification is the point of printing these separately: a
            // list that showed every row the same way would promise a removal
            // for the rows the backend says it will leave alone.
            let marker = match item.scope {
                UninstallItemScope::Removed => "  remove ",
                UninstallItemScope::System => "  system ",
                UninstallItemScope::ReviewOnly => "  review ",
            };
            println!(
                "{marker}{}{}",
                item.display_path,
                item.reported_size
                    .as_deref()
                    .map(|size| format!("  {size}"))
                    .unwrap_or_default()
            );
        }
        println!();
    }

    for warning in &preview.warnings {
        println!("warning: {warning}");
    }
    for note in &preview.notes {
        println!("note: {note}");
    }
    println!(
        "{} path(s){}. Nothing was modified: this is the backend's own dry run.",
        preview.total_items(),
        preview
            .reported_total
            .as_deref()
            .map(|total| format!(", about {total}"))
            .unwrap_or_default()
    );
    ExitCode::SUCCESS
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    text.chars()
        .take(width.saturating_sub(1))
        .collect::<String>()
        + "…"
}
