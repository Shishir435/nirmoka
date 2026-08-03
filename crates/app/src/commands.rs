//! The IPC surface. Every one of these is a thin translation.
//!
//! Nothing here decides anything: detection lives in the adapters, sizes and
//! ordering live in `nirmoka-core`, and scanning lives on a worker thread. A
//! command that starts making decisions is a sign the logic landed in the shell,
//! where the CLI and the tests cannot reach it.
//!
//! Each command delegates to a plain function taking `&AppState`, because
//! `tauri::State` cannot be constructed outside a running application — so a
//! command body would be untestable, and the tests would end up asserting
//! against a copy of it.
//!
//! Errors cross as strings. The frontend shows them; it does not branch on them.
//! When it needs to — step 10, where deletion has failure modes worth
//! distinguishing — this becomes a typed error, with an ADR for the change.

use std::time::Instant;

use tauri::{AppHandle, Manager, State};

use nirmoka_adapter::{Ability, CancelToken, DeleteMode};

use crate::deletion::PendingDelete;
use crate::dto;
use crate::inventory;
use crate::scan;
use crate::state::{AppState, ScanId};

/// The most rows one call may return.
///
/// A window is a few dozen rows. This is not a performance tuning knob — it is
/// invariant 5 made unbypassable: no caller can ask for the whole tree by
/// passing a large enough limit.
pub const MAX_ROWS: u32 = 1_000;

/// Backends and their detection state.
///
/// Detection runs on every call rather than being cached. It is a subprocess and
/// a version parse — a few milliseconds — and a cached answer would keep saying
/// "not installed" after the user installed the thing.
pub fn backends(state: &AppState) -> Vec<dto::Backend> {
    state.detect_all().iter().map(dto::Backend::from).collect()
}

/// What the backend running a scan can do.
///
/// Scoped to the scanner rather than to the chosen backend, because the controls
/// this gates sit on the browser: they act on the tree a scan produced, so the
/// backend that produced it is the one whose abilities apply. Deletion in step
/// 10 asks a different question and gets its own resolution.
pub fn capabilities_of(state: &AppState) -> Result<dto::Capabilities, String> {
    state
        .scanner()
        .map(|choice| dto::Capabilities::from(choice.adapter.capabilities()))
        .ok_or_else(|| "no usable backend is installed".to_string())
}

/// The backend choice, and what it actually resolves to.
pub fn selection_of(state: &AppState) -> dto::BackendSelection {
    let preference = state.preference();
    let scanner = state.scanner();

    dto::BackendSelection {
        chosen: preference.chosen,
        default_order: nirmoka_adapter::default_order()
            .iter()
            .map(|id| id.to_string())
            .collect(),
        scanner: scanner
            .as_ref()
            .map(|choice| choice.adapter.id().to_string()),
        scanner_instead_of: scanner.and_then(|choice| choice.instead_of),
        persistent: state.is_persistent(),
        // Reading the selection never writes anything, so there is nothing here
        // to have failed. Only `choose_backend_in` can fill this in.
        save_error: None,
    }
}

/// One backend-produced system-health snapshot.
pub fn system_status_of(state: &AppState) -> Result<dto::SystemStatus, String> {
    let choice = state
        .resolve(Ability::SystemStatus)
        .ok_or_else(|| "no usable backend provides system status".to_string())?;
    let backend = choice.adapter.id();
    let instead_of = choice.instead_of;
    let status = choice
        .adapter
        .system_status(&CancelToken::new())
        .map_err(|error| error.to_string())?;

    Ok(dto::SystemStatus::from_adapter(backend, instead_of, status))
}

/// One fresh backend-owned cleanup discovery. This never removes anything.
pub fn cleanup_preview_of(
    state: &AppState,
    cancel: &CancelToken,
) -> Result<dto::CleanupPreview, String> {
    let choice = state
        .resolve(Ability::CleanupPreview)
        .ok_or_else(|| "no usable backend provides cleanup preview".to_string())?;
    let backend = choice.adapter.id();
    let instead_of = choice.instead_of;
    let preview = choice
        .adapter
        .cleanup_preview(cancel)
        .map_err(|error| error.to_string())?;

    state
        .cleanup()
        .remember(backend, instead_of.clone(), preview.clone(), Instant::now());

    Ok(dto::CleanupPreview::from_adapter(
        backend, instead_of, preview,
    ))
}

/// Bind the latest backend-produced preview to one short-lived, one-time token.
/// No cleanup path returns as an execute parameter.
pub fn prepare_cleanup_of(state: &AppState) -> Result<dto::CleanupPreparation, String> {
    let preparation = state.cleanup().prepare(Instant::now()).ok_or_else(|| {
        "no fresh non-empty cleanup preview is available; generate a new preview".to_string()
    })?;

    Ok(dto::CleanupPreparation::from_state(preparation))
}

/// Consume a one-time confirmation and run the backend's own cleanup.
///
/// The token names a reviewed preview, not a list of arguments: Mole 1.48.1
/// accepts neither paths nor categories and re-discovers candidates itself. What
/// crosses into the backend is the reviewed *version*, which the adapter refuses
/// if the installed Mole has changed since the review.
///
/// The confirmation is spent whether or not the run succeeds, and the reviewed
/// preview is dropped either way — after a run, every path in it is a statement
/// about the past.
pub fn confirm_cleanup_in(
    state: &AppState,
    confirmation_token: u64,
    cancel: &CancelToken,
) -> Result<dto::CleanupOperation, String> {
    let pending = state
        .cleanup()
        .take(confirmation_token, Instant::now())
        .ok_or_else(|| {
            "this cleanup confirmation is invalid, expired, or was already used; generate a new preview"
                .to_string()
        })?;

    let result = state
        .adapter(&pending.backend)
        .ok_or_else(|| {
            format!(
                "the backend that produced this cleanup review ({}) is no longer registered",
                pending.backend
            )
        })
        .and_then(|adapter| {
            adapter
                .execute_cleanup(&pending.preview.backend_version, cancel)
                .map_err(|error| error.to_string())
        });

    state.cleanup().forget();

    let execution = result?;
    let operation = state
        .deletion()
        .record_cleanup(crate::deletion::CleanupRecord {
            backend: pending.backend,
            backend_version: pending.preview.backend_version,
            preview_generated_at: pending.preview.generated_at,
            reviewed_items: pending.preview.total_items,
            reviewed_potential_cleanup: pending.preview.potential_cleanup,
            execution,
        });

    Ok(dto::CleanupOperation::from_operation(&operation))
}

pub fn cleanup_log_of(state: &AppState) -> Vec<dto::CleanupOperation> {
    state
        .deletion()
        .cleanups()
        .iter()
        .map(dto::CleanupOperation::from_operation)
        .collect()
}

/// Pick a backend, or pass `None` to go back to the platform default.
///
/// Returns the selection as it now stands rather than the id that was passed in.
/// The caller needs to know what the choice *resolved* to — picking Mole on
/// macOS is accepted and still leaves ncdu scanning — and a round trip that
/// echoed the input would leave the UI to work that out for itself.
///
/// An id naming no registered backend is not rejected. It resolves to nothing,
/// falls back, and says so; refusing it would mean the only way to recover from
/// a hand-edited settings file is to hand-edit it again.
///
/// **Never fails.** A choice that could not be written down still took effect,
/// so failing the call would leave the picker showing the previous backend while
/// the process ran on the new one — the window and the program disagreeing about
/// a setting the user is looking at. The write failure travels as
/// `save_error` instead, alongside the selection it did not prevent.
pub fn choose_backend_in(state: &AppState, id: Option<String>) -> dto::BackendSelection {
    let outcome = state.choose(nirmoka_adapter::Preference { chosen: id });

    dto::BackendSelection {
        save_error: outcome.err(),
        ..selection_of(state)
    }
}

pub fn summary_of(state: &AppState) -> Option<dto::ScanSummary> {
    state
        .scan()
        .result
        .as_ref()
        .map(|result| result.summary.clone())
}

/// One window of one directory's children, in the requested order.
///
/// `scan_id` is the one the row or summary carrying `parent_id` came from. An
/// index alone does not identify a node: every scan numbers its tree from zero,
/// so an id left over from a replaced scan resolves against the new tree
/// whenever that tree is long enough — and names a different directory. The pair
/// identifies a node; the index on its own identifies a slot.
///
/// `parent_id` of `None` means the scan root, so the frontend can ask for its
/// first screen with nothing but the id of the scan that just finished.
///
/// `sort` orders the whole directory before the window is cut. Sorting a window
/// after the fact would only ever order the rows already on screen.
pub fn rows_of(
    state: &AppState,
    scan_id: ScanId,
    parent_id: Option<u32>,
    sort: dto::Sort,
    offset: u32,
    limit: u32,
) -> Result<dto::RowPage, String> {
    let scan = state.scan();
    let result = scan
        .result
        .as_ref()
        .ok_or_else(|| "no scan has completed yet".to_string())?;

    if result.id != scan_id {
        return Err(format!(
            "scan {scan_id} has been replaced by scan {}; ask again from its root",
            result.id
        ));
    }

    let parent = match parent_id {
        Some(raw) => result
            .tree
            .node_id(raw)
            .map_err(|error| error.to_string())?,
        None => result
            .tree
            .root()
            .ok_or_else(|| "the scan produced an empty tree".to_string())?,
    };

    Ok(dto::page(
        result.id,
        &result.tree,
        parent,
        sort,
        offset,
        limit.min(MAX_ROWS),
    ))
}

/// Validate a selected row through the deletion adapter and issue a one-time
/// confirmation token. The raw path never returns as an execute parameter.
pub fn prepare_delete_in(
    state: &AppState,
    scan_id: ScanId,
    node_id: u32,
) -> Result<dto::DeletePreparation, dto::DeleteFailure> {
    let (scan_root, target, total_bytes) = {
        let scan = state.scan();
        let result = scan.result.as_ref().ok_or_else(|| {
            dto::DeleteFailure::new(
                dto::DeleteFailureCode::NoCompletedScan,
                "no scan has completed yet",
            )
        })?;
        if result.id != scan_id {
            return Err(dto::DeleteFailure::new(
                dto::DeleteFailureCode::StaleScan,
                format!("scan {scan_id} has been replaced by scan {}", result.id),
            ));
        }
        let node = result.tree.node_id(node_id).map_err(|error| {
            dto::DeleteFailure::new(dto::DeleteFailureCode::UnknownNode, error.to_string())
        })?;
        let target = result.tree.path_of(node).map_err(|error| {
            dto::DeleteFailure::new(dto::DeleteFailureCode::UnknownNode, error.to_string())
        })?;
        let total_bytes = result
            .tree
            .get(node)
            .map(|node| node.total_bytes)
            .map_err(|error| {
                dto::DeleteFailure::new(dto::DeleteFailureCode::UnknownNode, error.to_string())
            })?;
        (result.tree.root_path().to_path_buf(), target, total_bytes)
    };

    let choice = state.resolve(Ability::Trash).ok_or_else(|| {
        dto::DeleteFailure::new(
            dto::DeleteFailureCode::NoBackend,
            "no backend safely supports execution-bound selected-path deletion",
        )
    })?;
    let plan = choice
        .adapter
        .prepare_delete(&scan_root, &target, DeleteMode::Trash)
        .map_err(backend_failure)?;
    let target_path = plan.target().display().to_string();
    let backend = plan.backend().to_string();
    let token = state.deletion().prepare(PendingDelete {
        scan_id,
        plan,
        total_bytes,
    });

    Ok(dto::DeletePreparation {
        confirmation_token: token,
        backend,
        backend_instead_of: choice.instead_of,
        target_path,
        total_bytes,
        disposition: dto::DeleteDisposition::Trash,
        recoverable: true,
        dry_run: false,
        requires_confirmation: true,
        warning: "This item will be moved to rip's recoverable graveyard. Confirm explicitly to continue."
            .to_string(),
    })
}

/// Consume a one-time confirmation and execute the exact plan it names.
pub fn confirm_delete_in(
    state: &AppState,
    confirmation_token: u64,
) -> Result<dto::DeleteOperation, dto::DeleteFailure> {
    let pending = state
        .deletion()
        .take_pending(confirmation_token)
        .ok_or_else(|| {
            dto::DeleteFailure::new(
                dto::DeleteFailureCode::ConfirmationExpired,
                "this deletion confirmation is invalid, expired, or was already used",
            )
        })?;

    let current_scan = state.scan().result.as_ref().map(|result| result.id);
    if current_scan != Some(pending.scan_id) {
        return Err(dto::DeleteFailure::new(
            dto::DeleteFailureCode::StaleScan,
            "the scan changed after confirmation was prepared; select the item again",
        ));
    }

    let adapter = state.adapter(pending.plan.backend()).ok_or_else(|| {
        dto::DeleteFailure::new(
            dto::DeleteFailureCode::NoBackend,
            "the backend that prepared this deletion is no longer registered",
        )
    })?;
    let receipt = adapter
        .delete(&pending.plan, &CancelToken::new())
        .map_err(backend_failure)?;
    let operation = state.deletion().record_delete(receipt).map_err(|error| {
        dto::DeleteFailure::new(
            dto::DeleteFailureCode::Backend,
            format!("deletion receipt could not be made durable: {error}"),
        )
    })?;

    // The scan describes a path that no longer exists. Keeping it would make a
    // second click target stale filesystem state, so require a rescan.
    state.scan().result = None;
    Ok(dto::DeleteOperation::from_operation(&operation))
}

pub fn undo_delete_in(
    state: &AppState,
    operation_id: u64,
) -> Result<dto::DeleteOperation, dto::DeleteFailure> {
    let operation = state.deletion().operation(operation_id).ok_or_else(|| {
        dto::DeleteFailure::new(
            dto::DeleteFailureCode::ConfirmationExpired,
            format!("unknown deletion operation {operation_id}"),
        )
    })?;
    if operation.undone_at_ms.is_some() {
        return Err(dto::DeleteFailure::new(
            dto::DeleteFailureCode::AlreadyUndone,
            "this deletion has already been undone",
        ));
    }

    let adapter = state.adapter(operation.receipt.backend()).ok_or_else(|| {
        dto::DeleteFailure::new(
            dto::DeleteFailureCode::NoBackend,
            "the backend needed to undo this deletion is no longer registered",
        )
    })?;
    adapter
        .undo(&operation.receipt, &CancelToken::new())
        .map_err(backend_failure)?;
    let operation = state
        .deletion()
        .mark_undone(operation_id)
        .map_err(|message| dto::DeleteFailure::new(dto::DeleteFailureCode::Backend, message))?;
    Ok(dto::DeleteOperation::from_operation(&operation))
}

pub fn operation_log_of(state: &AppState) -> Vec<dto::DeleteOperation> {
    state
        .deletion()
        .operations()
        .iter()
        .map(dto::DeleteOperation::from_operation)
        .collect()
}

pub fn application_inventory_of(
    state: &AppState,
    scan_id: ScanId,
) -> Result<dto::ApplicationInventory, String> {
    let scan = state.scan();
    let result = scan
        .result
        .as_ref()
        .ok_or_else(|| "no scan has completed yet".to_string())?;
    if result.id != scan_id {
        return Err(format!(
            "scan {scan_id} has been replaced by scan {}",
            result.id
        ));
    }
    Ok(inventory::applications(result.id, &result.tree))
}

pub fn installed_application_inventory_of(
    state: &AppState,
) -> Result<dto::InstalledApplicationInventory, String> {
    let choice = state
        .resolve(Ability::UninstallApps)
        .ok_or_else(|| "no usable backend provides application uninstall".to_string())?;
    let backend = choice.adapter.id();
    let instead_of = choice.instead_of;
    let applications = choice
        .adapter
        .installed_applications(&CancelToken::new())
        .map_err(|error| error.to_string())?;

    Ok(dto::InstalledApplicationInventory::from_adapter(
        backend,
        instead_of,
        applications,
    ))
}

pub fn developer_inventory_of(
    state: &AppState,
    scan_id: ScanId,
) -> Result<dto::DeveloperInventory, String> {
    let scan = state.scan();
    let result = scan
        .result
        .as_ref()
        .ok_or_else(|| "no scan has completed yet".to_string())?;
    if result.id != scan_id {
        return Err(format!(
            "scan {scan_id} has been replaced by scan {}",
            result.id
        ));
    }
    Ok(inventory::developer(result.id, &result.tree))
}

fn backend_failure(error: nirmoka_adapter::AdapterError) -> dto::DeleteFailure {
    dto::DeleteFailure::new(dto::DeleteFailureCode::Backend, error.to_string())
}

#[tauri::command]
pub fn list_backends(state: State<'_, AppState>) -> Vec<dto::Backend> {
    backends(&state)
}

#[tauri::command]
pub fn capabilities(state: State<'_, AppState>) -> Result<dto::Capabilities, String> {
    capabilities_of(&state)
}

#[tauri::command]
pub fn backend_selection(state: State<'_, AppState>) -> dto::BackendSelection {
    selection_of(&state)
}

#[tauri::command]
pub fn choose_backend(state: State<'_, AppState>, id: Option<String>) -> dto::BackendSelection {
    choose_backend_in(&state, id)
}

/// Returns the canonical root actually being scanned, which differs from what
/// was asked for whenever the path was relative or went through a symlink.
#[tauri::command]
pub fn start_scan(app: AppHandle, root_path: String) -> Result<String, String> {
    scan::start(&app, &root_path).map(|root| root.display().to_string())
}

/// Returns whether there was a scan to stop.
#[tauri::command]
pub fn cancel_scan(state: State<'_, AppState>) -> bool {
    scan::cancel(&state)
}

#[tauri::command]
pub fn scan_summary(state: State<'_, AppState>) -> Option<dto::ScanSummary> {
    summary_of(&state)
}

#[tauri::command]
pub fn rows(
    state: State<'_, AppState>,
    scan_id: ScanId,
    parent_id: Option<u32>,
    sort: dto::Sort,
    offset: u32,
    limit: u32,
) -> Result<dto::RowPage, String> {
    rows_of(&state, scan_id, parent_id, sort, offset, limit)
}

#[tauri::command]
pub fn prepare_delete(
    state: State<'_, AppState>,
    scan_id: ScanId,
    node_id: u32,
) -> Result<dto::DeletePreparation, dto::DeleteFailure> {
    prepare_delete_in(&state, scan_id, node_id)
}

#[tauri::command]
pub fn confirm_delete(
    state: State<'_, AppState>,
    confirmation_token: u64,
) -> Result<dto::DeleteOperation, dto::DeleteFailure> {
    confirm_delete_in(&state, confirmation_token)
}

#[tauri::command]
pub fn undo_delete(
    state: State<'_, AppState>,
    operation_id: u64,
) -> Result<dto::DeleteOperation, dto::DeleteFailure> {
    undo_delete_in(&state, operation_id)
}

#[tauri::command]
pub fn operation_log(state: State<'_, AppState>) -> Vec<dto::DeleteOperation> {
    operation_log_of(&state)
}

#[tauri::command]
pub fn volume_info(path: String) -> Result<dto::VolumeInfo, String> {
    crate::volume::info(std::path::Path::new(&path))
}

#[tauri::command]
pub fn application_inventory(
    state: State<'_, AppState>,
    scan_id: ScanId,
) -> Result<dto::ApplicationInventory, String> {
    application_inventory_of(&state, scan_id)
}

#[tauri::command]
pub fn installed_application_inventory(
    state: State<'_, AppState>,
) -> Result<dto::InstalledApplicationInventory, String> {
    installed_application_inventory_of(&state)
}

#[tauri::command]
pub fn developer_inventory(
    state: State<'_, AppState>,
    scan_id: ScanId,
) -> Result<dto::DeveloperInventory, String> {
    developer_inventory_of(&state, scan_id)
}

#[tauri::command]
pub fn system_status(state: State<'_, AppState>) -> Result<dto::SystemStatus, String> {
    system_status_of(&state)
}

#[tauri::command]
pub async fn cleanup_preview(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<dto::CleanupPreview, String> {
    let (preview_id, cancel) = state.cleanup().start_preview()?;
    let worker_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        cleanup_preview_of(worker_app.state::<AppState>().inner(), &cancel)
    })
    .await;

    state.cleanup().finish_preview(preview_id);
    result.map_err(|error| format!("cleanup preview worker failed: {error}"))?
}

#[tauri::command]
pub fn cancel_cleanup_preview(state: State<'_, AppState>) -> bool {
    state.cleanup().cancel_preview()
}

#[tauri::command]
pub fn prepare_cleanup(state: State<'_, AppState>) -> Result<dto::CleanupPreparation, String> {
    prepare_cleanup_of(&state)
}

/// Runs on a worker thread, because Mole's cleanup takes minutes and a blocked
/// command would freeze the window it is reporting to.
#[tauri::command]
pub async fn confirm_cleanup(
    app: AppHandle,
    state: State<'_, AppState>,
    confirmation_token: u64,
) -> Result<dto::CleanupOperation, String> {
    let cancel = state.cleanup().start_execution()?;
    let worker_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        confirm_cleanup_in(
            worker_app.state::<AppState>().inner(),
            confirmation_token,
            &cancel,
        )
    })
    .await;

    state.cleanup().finish_execution();
    result.map_err(|error| format!("cleanup worker failed: {error}"))?
}

/// Returns whether there was a run to stop. Cancellation kills Mole; what it
/// had already removed stays removed, which the returned operation says.
#[tauri::command]
pub fn cancel_cleanup(state: State<'_, AppState>) -> bool {
    state.cleanup().cancel_execution()
}

#[tauri::command]
pub fn cleanup_log(state: State<'_, AppState>) -> Vec<dto::CleanupOperation> {
    cleanup_log_of(&state)
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use nirmoka_adapter::{CleanupPreview, CleanupSystemScope};
    use nirmoka_core::{Node, Tree};

    use super::*;
    use crate::state::ScanResult;

    const SCAN: ScanId = 7;
    const SORT: dto::Sort = dto::Sort::LargestFirst;

    /// A tree of `size` files under a root, recorded as scan `id`.
    fn state_with_a_scan_of(id: ScanId, size: u32) -> AppState {
        let mut tree = Tree::new("/fixtures/root");
        let root = tree.push(None, Node::directory("root"));
        for index in 0..size {
            tree.push(Some(root), Node::file(format!("file-{index}"), 1));
        }
        tree.rollup();

        let state = AppState::new();
        state.scan().result = Some(ScanResult {
            id,
            summary: dto::ScanSummary::new(
                id,
                &tree,
                &nirmoka_adapter::ScanSummary::default(),
                Default::default(),
                "ncdu",
            ),
            tree,
        });
        state
    }

    fn state_with_a_scan() -> AppState {
        state_with_a_scan_of(SCAN, MAX_ROWS + 5)
    }

    #[test]
    fn cleanup_preparation_uses_only_the_preview_held_in_rust() {
        let state = AppState::with_preference(nirmoka_adapter::Preference::of("mole"), false);
        state.cleanup().remember(
            "mole",
            None,
            CleanupPreview {
                backend_version: "1.48.1".to_string(),
                generated_at: "2026-08-02 12:00:00".to_string(),
                categories: Vec::new(),
                potential_cleanup: Some("12MB".to_string()),
                total_items: 3,
                system_scope: CleanupSystemScope::UserOnly,
                warnings: vec!["partial preview".to_string()],
            },
            Instant::now(),
        );

        let preparation = prepare_cleanup_of(&state).expect("fresh preview");

        assert_eq!(preparation.backend, "mole");
        assert_eq!(preparation.backend_version, "1.48.1");
        assert_eq!(preparation.preview_generated_at, "2026-08-02 12:00:00");
        assert_eq!(preparation.total_items, 3);
        assert_eq!(preparation.warnings, ["partial preview"]);
        assert!(preparation.requires_confirmation);
        assert!(preparation.warning.contains("re-discover"));
    }

    /// A stand-in for Mole. It cleans only when handed the version that produced
    /// the review, which is the plumbing this fake exists to check.
    struct FakeCleaner {
        outcome: Result<nirmoka_adapter::CleanupExecution, &'static str>,
    }

    impl FakeCleaner {
        fn finishing() -> Self {
            Self {
                outcome: Ok(nirmoka_adapter::CleanupExecution {
                    system_scope: CleanupSystemScope::UserOnly,
                    completion: nirmoka_adapter::CleanupCompletion::Partial,
                    warnings: vec!["System-level cleanup was skipped.".to_string()],
                }),
            }
        }

        fn refusing() -> Self {
            Self {
                outcome: Err("mo changed from reviewed version 1.48.1 to 1.49.0"),
            }
        }
    }

    impl nirmoka_adapter::Adapter for FakeCleaner {
        fn id(&self) -> &'static str {
            "mole"
        }
        fn display_name(&self) -> &'static str {
            "Mole"
        }
        fn supported_versions(&self) -> &'static str {
            "1.48.x"
        }
        fn detect(&self) -> Result<nirmoka_adapter::Detection, nirmoka_adapter::AdapterError> {
            Ok(nirmoka_adapter::Detection::Found {
                path: std::path::PathBuf::from("/fixtures/mo"),
                version: "1.48.1".to_string(),
            })
        }
        fn capabilities(&self) -> nirmoka_adapter::Capabilities {
            nirmoka_adapter::Capabilities {
                scan: false,
                dry_run: true,
                cleanup_categories: true,
                ..nirmoka_adapter::Capabilities::MINIMAL
            }
        }
        fn execute_cleanup(
            &self,
            reviewed_version: &str,
            _cancel: &CancelToken,
        ) -> Result<nirmoka_adapter::CleanupExecution, nirmoka_adapter::AdapterError> {
            assert_eq!(
                reviewed_version, "1.48.1",
                "execution must carry the version the preview was produced by"
            );
            self.outcome
                .clone()
                .map_err(|reason| nirmoka_adapter::AdapterError::OperationFailed {
                    backend: "mole",
                    operation: "cleanup execution",
                    reason: reason.to_string(),
                })
        }
        fn scan(
            &self,
            _root: &std::path::Path,
            _options: &nirmoka_adapter::ScanOptions,
            _sink: &mut dyn nirmoka_adapter::WireSink,
            _cancel: &CancelToken,
        ) -> Result<nirmoka_adapter::ScanSummary, nirmoka_adapter::AdapterError> {
            unreachable!("the cleanup backend never scans")
        }
    }

    fn reviewed_preview() -> CleanupPreview {
        CleanupPreview {
            backend_version: "1.48.1".to_string(),
            generated_at: "2026-08-02 12:00:00".to_string(),
            categories: Vec::new(),
            potential_cleanup: Some("At least 192.00MB".to_string()),
            total_items: 6,
            system_scope: CleanupSystemScope::UserOnly,
            warnings: Vec::new(),
        }
    }

    /// A state whose only backend is `cleaner`, journalling to `log`.
    fn state_with_a_reviewed_cleanup(
        cleaner: FakeCleaner,
        log: Option<std::path::PathBuf>,
    ) -> AppState {
        let mut registry = nirmoka_adapter::Registry::new();
        registry.register(Box::new(cleaner));
        let state = AppState::with_parts(
            nirmoka_adapter::Preference::of("mole"),
            false,
            registry,
            log,
        );
        state
            .cleanup()
            .remember("mole", None, reviewed_preview(), Instant::now());
        state
    }

    fn journal_path(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "nirmoka-cleanup-command-{name}-{}.jsonl",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn a_confirmed_cleanup_is_journalled_with_the_reviewed_evidence() {
        let log = journal_path("recorded");
        let state = state_with_a_reviewed_cleanup(FakeCleaner::finishing(), Some(log.clone()));
        let token = prepare_cleanup_of(&state)
            .expect("preparation")
            .confirmation_token;

        let operation =
            confirm_cleanup_in(&state, token, &CancelToken::new()).expect("cleanup ran");

        assert_eq!(operation.backend, "mole");
        assert_eq!(operation.backend_version, "1.48.1");
        assert_eq!(operation.preview_generated_at, "2026-08-02 12:00:00");
        assert_eq!(operation.reviewed_items, 6);
        assert_eq!(operation.completion, dto::CleanupCompletion::Partial);
        assert_eq!(operation.system_scope, dto::CleanupSystemScope::UserOnly);
        assert_eq!(operation.warnings.len(), 1);
        assert!(operation.log_error.is_none());
        assert!(operation.executed_at_ms > 0);

        // Durable, and the same entry the log command serves.
        assert_eq!(cleanup_log_of(&state), vec![operation.clone()]);
        let recorded = std::fs::read_to_string(&log).expect("journal file");
        assert!(recorded.contains("\"event\":\"cleaned\""), "{recorded}");
        assert!(recorded.contains("\"reviewed_items\":6"), "{recorded}");

        let reloaded = AppState::with_parts(
            nirmoka_adapter::Preference::of("mole"),
            false,
            crate::state::registry(),
            Some(log.clone()),
        );
        assert_eq!(cleanup_log_of(&reloaded), vec![operation]);
        let _ = std::fs::remove_file(log);
    }

    /// The confirmation is spent and the review is dropped even when the backend
    /// refuses, because a refusal at this point cannot prove nothing was removed.
    #[test]
    fn a_refused_cleanup_spends_the_confirmation_and_journals_nothing() {
        let log = journal_path("refused");
        let state = state_with_a_reviewed_cleanup(FakeCleaner::refusing(), Some(log.clone()));
        let token = prepare_cleanup_of(&state)
            .expect("preparation")
            .confirmation_token;

        let error =
            confirm_cleanup_in(&state, token, &CancelToken::new()).expect_err("backend refused");
        assert!(error.contains("changed from reviewed version"), "{error}");

        assert!(cleanup_log_of(&state).is_empty(), "nothing to report");
        assert!(!log.exists(), "and nothing written");
        assert!(confirm_cleanup_in(&state, token, &CancelToken::new()).is_err());
        assert!(
            prepare_cleanup_of(&state).is_err(),
            "the spent review is gone; a new preview is required"
        );
        let _ = std::fs::remove_file(log);
    }

    #[test]
    fn cleanup_execution_needs_a_live_confirmation() {
        let state = state_with_a_reviewed_cleanup(FakeCleaner::finishing(), None);

        let error = confirm_cleanup_in(&state, 999, &CancelToken::new())
            .expect_err("an unknown token is not a confirmation");

        assert!(
            error.contains("invalid, expired, or was already used"),
            "{error}"
        );
    }

    /// The window and the process must never disagree about the setting.
    ///
    /// `choose` applies the preference in memory before it tries to write it,
    /// so a call that returned an error on a failed write would leave the
    /// picker rendering the old backend while scans ran on the new one. The
    /// selection always comes back; the write failure rides along with it.
    #[test]
    fn choosing_always_returns_what_the_process_is_now_using() {
        let state = AppState::with_preference(
            nirmoka_adapter::Preference::platform_default(),
            // No configuration directory, so nothing is written at all — the
            // strongest version of "the save did not happen".
            false,
        );

        let selection = choose_backend_in(&state, Some("mole".to_string()));

        assert_eq!(selection.chosen.as_deref(), Some("mole"));
        assert_eq!(
            selection.chosen,
            state.preference().chosen,
            "the reported selection is not the one in force"
        );
        assert!(!selection.persistent, "and it says it will not survive");
    }

    /// An id naming nothing is a recoverable state, not a rejection: the only
    /// other way out of a hand-edited settings file would be to edit it again.
    #[test]
    fn choosing_a_backend_that_does_not_exist_falls_back_rather_than_failing() {
        let state = AppState::with_preference(nirmoka_adapter::Preference::default(), false);

        let selection = choose_backend_in(&state, Some("not-a-backend".to_string()));
        assert_eq!(selection.chosen.as_deref(), Some("not-a-backend"));

        if let Some(scanner) = &selection.scanner {
            assert_ne!(scanner, "not-a-backend");
            assert_eq!(
                selection.scanner_instead_of.as_deref(),
                Some("not-a-backend")
            );
        }

        // And it is reversible.
        let cleared = choose_backend_in(&state, None);
        assert!(cleared.chosen.is_none());
        assert!(cleared.scanner_instead_of.is_none());
    }

    #[test]
    fn rows_before_a_scan_says_so_rather_than_returning_nothing() {
        let error = rows_of(&AppState::new(), SCAN, None, SORT, 0, 10).unwrap_err();
        assert!(error.contains("no scan"), "got: {error}");
    }

    #[test]
    fn a_limit_larger_than_the_cap_cannot_pull_the_whole_tree() {
        let page = rows_of(&state_with_a_scan(), SCAN, None, SORT, 0, u32::MAX).expect("a page");

        assert_eq!(page.rows.len(), MAX_ROWS as usize);
        assert_eq!(
            page.total,
            MAX_ROWS + 5,
            "the count stays honest about what was not sent"
        );
    }

    #[test]
    fn a_node_id_past_the_end_is_refused() {
        let error = rows_of(&state_with_a_scan(), SCAN, Some(u32::MAX), SORT, 0, 10).unwrap_err();
        assert!(error.contains("unknown node"), "got: {error}");
    }

    /// The reason ids travel with a scan id at all.
    ///
    /// Every tree numbers its nodes from zero, so an id from a replaced scan
    /// still lands inside the new tree. Bounds checking cannot tell the
    /// difference: node 3 exists in both, and it is a different file.
    #[test]
    fn an_id_from_a_replaced_scan_is_refused_even_though_the_index_exists() {
        let state = state_with_a_scan_of(2, 10);

        let page = rows_of(&state, 2, Some(3), SORT, 0, 10).expect("the current scan answers");
        assert_eq!(page.scan_id, 2);

        let error = rows_of(&state, 1, Some(3), SORT, 0, 10)
            .expect_err("an id from the previous scan must not resolve");
        assert!(error.contains("has been replaced"), "got: {error}");
    }

    #[test]
    fn a_page_carries_the_scan_it_came_from() {
        let page = rows_of(&state_with_a_scan(), SCAN, None, SORT, 0, 5).expect("a page");
        assert_eq!(
            page.scan_id, SCAN,
            "a caller cannot pair ids with a scan it was never told about"
        );
    }

    /// Sorting has to reorder the directory before the window is cut, so a
    /// changed sort has to change what the *first* page contains — not just the
    /// arrangement of rows already on screen.
    #[test]
    fn a_sort_reorders_the_directory_and_not_only_the_window() {
        let state = state_with_a_scan_of(SCAN, 30);

        let ascending =
            rows_of(&state, SCAN, None, dto::Sort::NameAscending, 0, 3).expect("a page");
        let descending =
            rows_of(&state, SCAN, None, dto::Sort::NameDescending, 0, 3).expect("a page");

        let names = |page: &dto::RowPage| -> Vec<String> {
            page.rows.iter().map(|row| row.name.clone()).collect()
        };

        assert_eq!(names(&ascending), vec!["file-0", "file-1", "file-10"]);
        assert_eq!(names(&descending), vec!["file-9", "file-8", "file-7"]);
        assert_eq!(descending.sort, dto::Sort::NameDescending);
    }

    #[test]
    fn a_page_knows_the_way_back_to_the_root() {
        let state = state_with_a_scan_of(SCAN, 3);
        let root = rows_of(&state, SCAN, None, SORT, 0, 3).expect("a page");
        assert!(root.ancestors.is_empty(), "the root is the way back");

        let child = root.rows[0].id;
        let page = rows_of(&state, SCAN, Some(child), SORT, 0, 3).expect("a page");
        assert_eq!(page.ancestors.len(), 1);
        assert_eq!(page.ancestors[0].id, root.parent_id);
    }

    #[test]
    fn a_summary_is_available_once_a_scan_has_landed() {
        assert!(summary_of(&AppState::new()).is_none());

        let summary = summary_of(&state_with_a_scan()).expect("a summary");
        assert_eq!(summary.backend_id, "ncdu");
        assert_eq!(summary.scan_id, SCAN);
        assert_eq!(summary.total_bytes, u64::from(MAX_ROWS + 5));
    }

    #[cfg(unix)]
    #[test]
    fn unsafe_selected_path_deletion_is_not_offered() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        use nirmoka_adapter::{Preference, Registry};
        use nirmoka_adapter_rip::RipAdapter;

        let base =
            std::env::temp_dir().join(format!("nirmoka-command-delete-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let root = base.join("scan");
        let target = root.join("file.txt");
        let binary = base.join("rip");
        let recovery = base.join("recovery");
        fs::create_dir_all(&root).unwrap();
        fs::write(&target, b"important").unwrap();
        fs::write(
            &binary,
            r#"#!/bin/sh
graveyard=""
undo=""
target=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --version) echo "rm-improved 0.13.1"; exit 0 ;;
    --graveyard) graveyard="$2"; shift 2 ;;
    -u|--unbury) undo="$2"; shift 2 ;;
    *) target="$1"; shift ;;
  esac
done
if [ -n "$undo" ]; then
  original="${undo#"$graveyard"}"
  mkdir -p "$(dirname "$original")"
  mv "$undo" "$original"
else
  destination="$graveyard$target"
  mkdir -p "$(dirname "$destination")"
  mv "$target" "$destination"
fi
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&binary).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&binary, permissions).unwrap();

        let mut registry = Registry::new();
        registry.register(Box::new(RipAdapter::with_binary_and_recovery_root(
            binary, recovery,
        )));
        let state = AppState::with_parts(Preference::platform_default(), false, registry, None);

        let mut tree = Tree::new(&root);
        let root_id = tree.push(None, Node::directory("scan"));
        let file_id = tree.push(Some(root_id), Node::file("file.txt", 9));
        tree.rollup();
        state.scan().result = Some(ScanResult {
            id: SCAN,
            summary: dto::ScanSummary::new(
                SCAN,
                &tree,
                &nirmoka_adapter::ScanSummary::default(),
                Default::default(),
                "test",
            ),
            tree,
        });

        let error = prepare_delete_in(&state, SCAN, file_id.raw()).unwrap_err();
        assert_eq!(error.code, dto::DeleteFailureCode::NoBackend);
        assert!(target.exists(), "refusal must leave the target untouched");
        let _ = fs::remove_dir_all(base);
    }
}
