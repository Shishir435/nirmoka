//! Line-delimited JSON service used by the native macOS SwiftUI shell.
//!
//! The service is a child process of the app. Its stdin/stdout are a private
//! request channel, so it needs no local listening socket or network
//! permission. The completed scan remains in `AppState`; only bounded row
//! pages cross into Swift.

use std::io::{self, BufRead, Write};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};

use nirmoka_adapter::Preference;
use serde::Deserialize;
use serde::Serialize;
use serde_json::{json, Value};

use crate::commands;
use crate::dto;
use crate::scan::{self, Reporter};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    id: Value,
    method: String,
    #[serde(default)]
    params: Value,
}

struct ScanReporter {
    writer: Arc<Mutex<io::Stdout>>,
    latest: Mutex<Option<dto::ScanProgress>>,
}

impl ScanReporter {
    fn event<T: Serialize>(&self, name: &str, payload: &T) {
        if let Ok(payload) = serde_json::to_value(payload) {
            write_message(&self.writer, &json!({ "event": name, "payload": payload }));
        }
    }

    fn latest(&self) -> Option<dto::ScanProgress> {
        lock(&self.latest).clone()
    }
}

impl Reporter for ScanReporter {
    fn progress(&self, progress: dto::ScanProgress) {
        *lock(&self.latest) = Some(progress.clone());
        self.event(scan::EVENT_PROGRESS, &progress);
    }

    fn finished(&self, summary: dto::ScanSummary) {
        *lock(&self.latest) = None;
        self.event(scan::EVENT_FINISHED, &summary);
    }

    fn failed(&self, failure: dto::ScanFailure) {
        *lock(&self.latest) = None;
        self.event(scan::EVENT_FAILED, &failure);
    }
}

/// Run the private service protocol until the SwiftUI parent closes stdin.
pub fn run() -> io::Result<()> {
    let writer = Arc::new(Mutex::new(io::stdout()));
    let state = Arc::new(AppState::new());
    let reporter = Arc::new(ScanReporter {
        writer: Arc::clone(&writer),
        latest: Mutex::new(None),
    });
    let mut workers: Vec<JoinHandle<()>> = Vec::new();

    for line in io::stdin().lock().lines() {
        let line = line?;
        reap_finished(&mut workers);

        let request = match serde_json::from_str::<Request>(&line) {
            Ok(request) => request,
            Err(error) => {
                write_message(
                    &writer,
                    &json!({ "id": Value::Null, "error": error.to_string() }),
                );
                continue;
            }
        };

        let worker_state = Arc::clone(&state);
        let worker_reporter = Arc::clone(&reporter);
        let worker_writer = Arc::clone(&writer);
        workers.push(thread::spawn(move || {
            let result = dispatch(
                &request.method,
                &request.params,
                &worker_state,
                &worker_reporter,
            );
            let message = match result {
                Ok(result) => json!({ "id": request.id, "result": result }),
                Err(error) => json!({ "id": request.id, "error": error }),
            };
            write_message(&worker_writer, &message);
        }));
    }

    // Closing the app closes the service pipe. Forward that shutdown to every
    // subprocess-backed operation before waiting for request workers to exit.
    scan::cancel(&state);
    state.cleanup().cancel_preview();
    state.cleanup().cancel_execution();
    state.uninstall().cancel_preview();
    state.uninstall().cancel_execution();
    state.optimization().cancel_preview();
    state.optimization().cancel_execution();

    // A request may still be entering its operation when EOF arrives. Repeat
    // cancellation while draining workers so a late-starting scan or cleanup
    // cannot miss the shutdown signal.
    while !workers.is_empty() || state.scan().active.is_some() {
        scan::cancel(&state);
        state.cleanup().cancel_preview();
        state.cleanup().cancel_execution();
        state.uninstall().cancel_preview();
        state.uninstall().cancel_execution();
        state.optimization().cancel_preview();
        state.optimization().cancel_execution();
        reap_finished(&mut workers);
        thread::sleep(std::time::Duration::from_millis(10));
    }

    Ok(())
}

fn dispatch(
    method: &str,
    params: &Value,
    state: &Arc<AppState>,
    reporter: &Arc<ScanReporter>,
) -> Result<Value, String> {
    match method {
        "listBackends" => encoded(commands::backends(state)),
        "capabilities" => encoded(commands::capabilities_of(state)?),
        "backendSelection" => encoded(commands::selection_of(state)),
        "systemStatus" => encoded(commands::system_status_of(state)?),
        "optimizePreview" => {
            let (preview_id, cancel) = state.optimization().start_preview()?;
            let result = commands::optimize_preview_of(state, &cancel);
            state.optimization().finish_preview(preview_id);
            encoded(result?)
        }
        "cancelOptimizePreview" => encoded(state.optimization().cancel_preview()),
        "prepareOptimize" => encoded(commands::prepare_optimize_of(state)?),
        "confirmOptimize" => {
            let token: u64 = read_param(params, "confirmationToken")?;
            let cancel = state.optimization().start_execution()?;
            let result = commands::confirm_optimize_in(state, token, &cancel);
            state.optimization().finish_execution();
            encoded(result?)
        }
        "cancelOptimize" => encoded(state.optimization().cancel_execution()),
        "chooseBackend" => {
            let chosen = read_param(params, "id")?;
            encoded(commands::choose_backend_in(state, chosen))
        }
        "startScan" => {
            let root: String = read_param(params, "rootPath")?;
            let scan_reporter: Arc<dyn Reporter> = reporter.clone();
            let path = scan::start_native(Arc::clone(state), &root, scan_reporter)?;
            encoded(json!({ "rootPath": path.display().to_string() }))
        }
        "cancelScan" => encoded(scan::cancel(state)),
        "scanStatus" => {
            let scan = state.scan();
            let active = scan.active.as_ref().map(|active| {
                json!({
                    "scanId": active.id,
                    "rootPath": active.root.display().to_string(),
                })
            });
            encoded(json!({
                "active": active,
                "progress": reporter.latest(),
                "summary": scan.result.as_ref().map(|result| &result.summary),
            }))
        }
        "rows" => {
            let scan_id: u64 = read_param(params, "scanId")?;
            let parent_id: Option<u32> = read_param(params, "parentId")?;
            let sort: dto::Sort = read_param(params, "sort")?;
            let offset: u32 = read_param(params, "offset")?;
            let limit: u32 = read_param(params, "limit")?;
            encoded(commands::rows_of(
                state, scan_id, parent_id, sort, offset, limit,
            )?)
        }
        "categoryBreakdown" => encoded(category_breakdown(state, read_param(params, "scanId")?)?),
        "appFootprint" => {
            let plan = commands::app_footprint_plan_of(
                state,
                read_param(params, "scanId")?,
                read_param(params, "nodeId")?,
            )?;
            encoded(crate::attribution::resolve(plan))
        }
        "installedAppFootprint" => {
            let name: String = read_param(params, "uninstallName")?;
            let inventory = commands::installed_application_inventory_of(state)?;
            let application = inventory
                .rows
                .iter()
                .find(|app| app.uninstall_name == name)
                .ok_or_else(|| {
                    "the application is no longer in the installed inventory".to_string()
                })?;
            encoded(crate::attribution::installed(
                std::path::Path::new(&application.path),
                &crate::path::expand_home("~"),
            )?)
        }
        "revealPath" => {
            let path: String = read_param(params, "path")?;
            crate::reveal::reveal(&crate::path::expand_home(&path))?;
            encoded(json!({ "revealed": true }))
        }
        "applicationInventory" => encoded(commands::application_inventory_of(
            state,
            read_param(params, "scanId")?,
        )?),
        "installedApplicationInventory" => {
            encoded(commands::installed_application_inventory_of(state)?)
        }
        "developerInventory" => encoded(commands::developer_inventory_of(
            state,
            read_param(params, "scanId")?,
        )?),
        "platformFeatures" => encoded(crate::reveal::features()),
        "volumeInfo" => {
            let path: String = read_param(params, "path")?;
            encoded(crate::volume::info(&crate::path::expand_home(&path))?)
        }
        "reveal" => {
            let scan_id: u64 = read_param(params, "scanId")?;
            let node_id: u32 = read_param(params, "nodeId")?;
            let path = commands::node_path_of(state, scan_id, node_id)?;
            crate::reveal::reveal(&path)?;
            encoded(json!({ "revealed": true }))
        }
        "prepareTrash" => {
            let scan_id: u64 = read_param(params, "scanId")?;
            let node_id: u32 = read_param(params, "nodeId")?;
            let preparation = commands::prepare_trash_in(state, scan_id, node_id)
                .map_err(|failure| failure.message)?;
            encoded(preparation)
        }
        "confirmTrash" => {
            let token: u64 = read_param(params, "confirmationToken")?;
            let operation =
                commands::confirm_trash_in(state, token).map_err(|failure| failure.message)?;
            encoded(operation)
        }
        "operationLog" => encoded(commands::operation_log_of(state)),
        "trashLog" => encoded(commands::trash_log_of(state)),
        "cleanupLog" => encoded(commands::cleanup_log_of(state)),
        "uninstallLog" => encoded(commands::uninstall_log_of(state)),
        "cleanupPreview" => {
            let (preview_id, cancel) = state.cleanup().start_preview()?;
            let result = commands::cleanup_preview_of(state, &cancel, &mut |progress| {
                reporter.event(crate::commands::EVENT_CLEANUP_PROGRESS, &progress);
            });
            state.cleanup().finish_preview(preview_id);
            encoded(result?)
        }
        "cancelCleanupPreview" => encoded(state.cleanup().cancel_preview()),
        "prepareCleanup" => encoded(commands::prepare_cleanup_of(state)?),
        "confirmCleanup" => {
            let token: u64 = read_param(params, "confirmationToken")?;
            let cancel = state.cleanup().start_execution()?;
            let result = commands::confirm_cleanup_in(state, token, &cancel);
            state.cleanup().finish_execution();
            encoded(result?)
        }
        "cancelCleanup" => encoded(state.cleanup().cancel_execution()),
        "prepareUninstall" => encoded(commands::prepare_uninstall_of(state)?),
        "uninstallPreview" => {
            let names: Vec<String> = read_param(params, "names")?;
            let (preview_id, cancel) = state.uninstall().start_preview()?;
            let result = commands::uninstall_preview_of(state, &names, &cancel);
            state.uninstall().finish_preview(preview_id);
            encoded(result?)
        }
        "cancelUninstallPreview" => encoded(state.uninstall().cancel_preview()),
        "confirmUninstall" => {
            let token: u64 = read_param(params, "confirmationToken")?;
            let cancel = state.uninstall().start_execution()?;
            let result = commands::confirm_uninstall_in(state, token, &cancel);
            state.uninstall().finish_execution();
            encoded(result?)
        }
        "cancelUninstall" => encoded(state.uninstall().cancel_execution()),
        "settingsDefault" => encoded(Preference::platform_default()),
        _ => Err(format!("unknown method: {method}")),
    }
}

fn category_breakdown(state: &AppState, scan_id: u64) -> Result<dto::CategoryBreakdown, String> {
    let root = {
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
        result.tree.root_path().to_path_buf()
    };

    // Volume information is supplementary. If it is unavailable, the category
    // totals remain valid and the native dashboard omits the capacity bar.
    let volume = crate::volume::info(&root).ok();
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
    let home = crate::path::expand_home("~");
    Ok(crate::categories::breakdown(
        result.id,
        &result.tree,
        &home,
        volume,
    ))
}

fn read_param<T: serde::de::DeserializeOwned>(params: &Value, key: &str) -> Result<T, String> {
    let value = params
        .get(key)
        .cloned()
        .ok_or_else(|| format!("missing parameter: {key}"))?;
    serde_json::from_value(value).map_err(|error| format!("invalid {key}: {error}"))
}

fn encoded(value: impl Serialize) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| error.to_string())
}

fn write_message(writer: &Arc<Mutex<io::Stdout>>, message: &Value) {
    let Ok(mut output) = writer.lock() else {
        return;
    };
    if serde_json::to_writer(&mut *output, message).is_ok() {
        let _ = output.write_all(b"\n");
        let _ = output.flush();
    }
}

fn reap_finished(workers: &mut Vec<JoinHandle<()>>) {
    let mut index = 0;
    while index < workers.len() {
        if workers[index].is_finished() {
            let worker = workers.swap_remove(index);
            let _ = worker.join();
        } else {
            index += 1;
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
