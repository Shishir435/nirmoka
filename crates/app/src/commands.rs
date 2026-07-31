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
    }
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
pub fn choose_backend_in(
    state: &AppState,
    id: Option<String>,
) -> Result<dto::BackendSelection, String> {
    state.choose(nirmoka_adapter::Preference { chosen: id })?;
    Ok(selection_of(state))
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
pub fn choose_backend(
    state: State<'_, AppState>,
    id: Option<String>,
) -> Result<dto::BackendSelection, String> {
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

#[cfg(test)]
mod tests {
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
}
