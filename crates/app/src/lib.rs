//! The Tauri shell.
//!
//! This crate owns the window and the IPC surface, and nothing else. Detection,
//! scanning, and the tree live in `nirmoka-core` and `nirmoka-adapter`, which
//! know nothing about Tauri — that is what `nirmoka-cli` proves on every build.
//!
//! Keeping the shell thin is what makes ADR 0005 (the frontend is replaceable)
//! true rather than aspirational: everything worth keeping is behind an
//! interface that a different shell could call tomorrow.

#![forbid(unsafe_code)]

pub mod cleanup;
pub mod commands;
pub mod deletion;
pub mod dto;
pub mod inventory;
pub mod path;
pub mod scan;
pub mod settings;
pub mod state;
pub mod volume;

use state::AppState;

/// Build and run the desktop application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::list_backends,
            commands::capabilities,
            commands::backend_selection,
            commands::choose_backend,
            commands::start_scan,
            commands::cancel_scan,
            commands::scan_summary,
            commands::rows,
            commands::prepare_delete,
            commands::confirm_delete,
            commands::undo_delete,
            commands::operation_log,
            commands::volume_info,
            commands::application_inventory,
            commands::installed_application_inventory,
            commands::developer_inventory,
            commands::system_status,
            commands::cleanup_preview,
            commands::cancel_cleanup_preview,
            commands::prepare_cleanup,
            commands::confirm_cleanup,
            commands::cancel_cleanup,
            commands::cleanup_log,
        ])
        .run(tauri::generate_context!())
        .expect("the Tauri application failed to start");
}
