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

use tauri::{AppHandle, State};

use crate::dto;
use crate::scan;
use crate::state::AppState;

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

pub fn capabilities_of(state: &AppState) -> Result<dto::Capabilities, String> {
    state
        .usable_adapter()
        .map(|adapter| dto::Capabilities::from(adapter.capabilities()))
        .ok_or_else(|| "no usable backend is installed".to_string())
}

pub fn summary_of(state: &AppState) -> Option<dto::ScanSummary> {
    state
        .scan()
        .result
        .as_ref()
        .map(|result| result.summary.clone())
}

/// One window of children, largest first.
///
/// `parent_id` of `None` means the scan root, so the frontend can ask for the
/// first screen without having read the summary first.
pub fn rows_of(
    state: &AppState,
    parent_id: Option<u32>,
    offset: u32,
    limit: u32,
) -> Result<dto::RowPage, String> {
    let scan = state.scan();
    let result = scan
        .result
        .as_ref()
        .ok_or_else(|| "no scan has completed yet".to_string())?;

    let parent = match parent_id {
        // An id from the webview is untrusted: it may be stale from a tree a
        // rescan has already replaced.
        Some(raw) => result
            .tree
            .node_id(raw)
            .map_err(|error| error.to_string())?,
        None => result
            .tree
            .root()
            .ok_or_else(|| "the scan produced an empty tree".to_string())?,
    };

    Ok(dto::page(&result.tree, parent, offset, limit.min(MAX_ROWS)))
}

#[tauri::command]
pub fn list_backends(state: State<'_, AppState>) -> Vec<dto::Backend> {
    backends(&state)
}

#[tauri::command]
pub fn capabilities(state: State<'_, AppState>) -> Result<dto::Capabilities, String> {
    capabilities_of(&state)
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
    parent_id: Option<u32>,
    offset: u32,
    limit: u32,
) -> Result<dto::RowPage, String> {
    rows_of(&state, parent_id, offset, limit)
}

#[cfg(test)]
mod tests {
    use nirmoka_core::{Node, Tree};

    use super::*;
    use crate::state::ScanResult;

    fn state_with_a_scan() -> AppState {
        let mut tree = Tree::new("/fixtures/root");
        let root = tree.push(None, Node::directory("root"));
        for index in 0..(MAX_ROWS + 5) {
            tree.push(Some(root), Node::file(format!("file-{index}"), 1));
        }
        tree.rollup();

        let state = AppState::new();
        state.scan().result = Some(ScanResult {
            summary: dto::ScanSummary::new(
                &tree,
                &nirmoka_adapter::ScanSummary::default(),
                Default::default(),
                "ncdu",
            ),
            tree,
        });
        state
    }

    #[test]
    fn rows_before_a_scan_says_so_rather_than_returning_nothing() {
        let error = rows_of(&AppState::new(), None, 0, 10).unwrap_err();
        assert!(error.contains("no scan"), "got: {error}");
    }

    #[test]
    fn a_limit_larger_than_the_cap_cannot_pull_the_whole_tree() {
        let page = rows_of(&state_with_a_scan(), None, 0, u32::MAX).expect("a page");

        assert_eq!(page.rows.len(), MAX_ROWS as usize);
        assert_eq!(
            page.total,
            MAX_ROWS + 5,
            "the count stays honest about what was not sent"
        );
    }

    #[test]
    fn a_stale_node_id_is_refused() {
        let error = rows_of(&state_with_a_scan(), Some(u32::MAX), 0, 10).unwrap_err();
        assert!(error.contains("unknown node"), "got: {error}");
    }

    #[test]
    fn a_summary_is_available_once_a_scan_has_landed() {
        assert!(summary_of(&AppState::new()).is_none());

        let summary = summary_of(&state_with_a_scan()).expect("a summary");
        assert_eq!(summary.backend_id, "ncdu");
        assert_eq!(summary.total_bytes, u64::from(MAX_ROWS + 5));
    }
}
