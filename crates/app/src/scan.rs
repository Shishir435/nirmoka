//! Running a scan without freezing the window.
//!
//! A scan is a subprocess and a blocking read that can last a minute on a home
//! directory. It runs on a worker thread; the command that starts it returns as
//! soon as the thread is spawned, and everything after that arrives as events.
//!
//! The [`CancelToken`] handed to the worker is the same one the stop button
//! trips. Cancelling kills the backend process — see `crates/adapter/src/process.rs`
//! — rather than abandoning it to keep churning the disk.

use std::path::{Path, PathBuf};
use std::thread;

use nirmoka_adapter::wire::{TreeSink, WireHeader, WireItem, WireSink};
use nirmoka_adapter::{validate_scan_root, CancelToken, ScanOptions};
use tauri::{AppHandle, Emitter, Manager};

use crate::dto;
use crate::state::{ActiveScan, AppState, ScanId, ScanResult};

/// Emitted periodically while a scan runs.
pub const EVENT_PROGRESS: &str = "scan://progress";
/// Emitted once, with the totals, when a scan completes.
pub const EVENT_FINISHED: &str = "scan://finished";
/// Emitted once when a scan ends without a result — including cancellation.
pub const EVENT_FAILED: &str = "scan://failed";

/// Entries between progress events.
///
/// A home directory produces millions of entries. One event each would spend
/// more time serialising progress than walking the disk, and would flood the
/// webview's event loop with work it cannot render anyway.
const PROGRESS_EVERY: u64 = 25_000;

/// The parser recurses, and [`nirmoka_adapter::wire::MAX_DEPTH`] caps that at
/// 256 levels of roughly 2 KB each. The default stack would survive it; saying
/// so explicitly means a later change to either number is a visible decision.
const WORKER_STACK_BYTES: usize = 4 * 1024 * 1024;

/// Somewhere to report progress. Exists so the sink can be tested without a
/// running Tauri application.
pub trait Reporter: Send + Sync {
    fn progress(&self, progress: dto::ScanProgress);
}

impl Reporter for AppHandle {
    fn progress(&self, progress: dto::ScanProgress) {
        // A failed emit means the window is gone. The scan is about to notice
        // that too; there is nobody left to tell.
        let _ = self.emit(EVENT_PROGRESS, progress);
    }
}

/// Wraps [`TreeSink`] and reports where the backend has got to.
///
/// It tracks the directory stack rather than a path string, because building a
/// path per entry would allocate millions of times to display a few hundred.
pub struct ProgressSink<'a, R: Reporter> {
    inner: TreeSink,
    reporter: &'a R,
    stack: Vec<String>,
    seen: u64,
    next_report: u64,
}

impl<'a, R: Reporter> ProgressSink<'a, R> {
    pub fn new(reporter: &'a R) -> Self {
        Self {
            inner: TreeSink::new(),
            reporter,
            stack: Vec::new(),
            seen: 0,
            next_report: 1,
        }
    }

    pub fn into_inner(self) -> TreeSink {
        self.inner
    }

    fn current_path(&self) -> String {
        let mut path = PathBuf::new();
        for segment in &self.stack {
            path.push(segment);
        }
        path.display().to_string()
    }

    fn tick(&mut self) {
        self.seen += 1;
        if self.seen < self.next_report {
            return;
        }

        self.next_report = self.seen + PROGRESS_EVERY;
        self.reporter.progress(dto::ScanProgress {
            scanned: self.seen,
            current_path: self.current_path(),
        });
    }
}

impl<R: Reporter> WireSink for ProgressSink<'_, R> {
    fn header(&mut self, header: &WireHeader) {
        self.inner.header(header);
    }

    fn open_dir(&mut self, item: WireItem) {
        self.stack.push(item.name.clone());
        self.inner.open_dir(item);
        self.tick();
    }

    fn item(&mut self, item: WireItem) {
        self.inner.item(item);
        self.tick();
    }

    fn close_dir(&mut self) {
        self.stack.pop();
        self.inner.close_dir();
    }
}

/// Begin a scan of `root`, if none is already running.
///
/// The path is validated here rather than on the worker, so a mistyped
/// directory is an error the caller sees immediately instead of an event that
/// arrives after the UI has already switched to a scanning state.
pub fn start(app: &AppHandle, root: &str) -> Result<PathBuf, String> {
    let state = app.state::<AppState>();

    if state.usable_adapter().is_none() {
        return Err("no usable backend is installed".into());
    }

    let root = validate_scan_root(Path::new(root)).map_err(|error| error.to_string())?;
    let cancel = CancelToken::new();

    let id = claim(&state, root.clone(), cancel.clone())?;

    let worker_app = app.clone();
    let worker_root = root.clone();

    let spawned = thread::Builder::new()
        .name("nirmoka-scan".into())
        .stack_size(WORKER_STACK_BYTES)
        .spawn(move || run(worker_app, id, worker_root, cancel));

    if let Err(error) = spawned {
        release(&state);
        return Err(format!("could not start the scan thread: {error}"));
    }

    Ok(root)
}

/// Mark a scan as running, refusing if one already is.
///
/// Split out from [`start`] because the pairing with [`release`] is the part
/// worth testing, and `start` needs an `AppHandle` that only exists inside a
/// running application.
fn claim(state: &AppState, root: PathBuf, cancel: CancelToken) -> Result<ScanId, String> {
    let mut scan = state.scan();

    if let Some(active) = &scan.active {
        return Err(format!(
            "a scan of {} is already running",
            active.root.display()
        ));
    }

    let id = scan.issue_id();
    scan.active = Some(ActiveScan { id, root, cancel });
    // The previous result goes now, not when the new one lands. Leaving it in
    // place would let `rows` answer from the old tree while the UI says it is
    // scanning something else.
    scan.result = None;

    Ok(id)
}

/// Give up a claim nothing is going to honour.
///
/// Without this, a worker thread that fails to spawn leaves an active scan that
/// no thread will ever clear: every later scan is refused as "already running",
/// and the stop button trips a token nobody is watching. The window would have
/// to be restarted to scan anything again.
fn release(state: &AppState) {
    state.scan().active = None;
}

/// Stop the running scan. Returns whether there was one.
pub fn cancel(state: &AppState) -> bool {
    match &state.scan().active {
        Some(active) => {
            active.cancel.cancel();
            true
        }
        None => false,
    }
}

fn run(app: AppHandle, id: ScanId, root: PathBuf, cancel: CancelToken) {
    let state = app.state::<AppState>();
    let outcome = walk(&state, &app, id, &root, &cancel);

    let mut scan = state.scan();
    scan.active = None;

    match outcome {
        Ok(result) => {
            let summary = result.summary.clone();
            scan.result = Some(result);
            drop(scan);
            let _ = app.emit(EVENT_FINISHED, summary);
        }
        Err(failure) => {
            drop(scan);
            let _ = app.emit(EVENT_FAILED, failure);
        }
    }
}

fn walk(
    state: &AppState,
    app: &AppHandle,
    id: ScanId,
    root: &Path,
    cancel: &CancelToken,
) -> Result<ScanResult, dto::ScanFailure> {
    let adapter = state.usable_adapter().ok_or_else(|| dto::ScanFailure {
        message: "no usable backend is installed".into(),
        cancelled: false,
    })?;

    let mut sink = ProgressSink::new(app);
    let summary = adapter
        .scan(root, &ScanOptions::default(), &mut sink, cancel)
        .map_err(|error| dto::ScanFailure {
            message: error.to_string(),
            cancelled: error.is_cancellation(),
        })?;

    let sink = sink.into_inner();
    let stats = sink.stats();
    let tree = sink.finish();
    let summary = dto::ScanSummary::new(id, &tree, &summary, stats, adapter.id());

    Ok(ScanResult { id, tree, summary })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct Recorder {
        seen: Mutex<Vec<dto::ScanProgress>>,
    }

    impl Reporter for Recorder {
        fn progress(&self, progress: dto::ScanProgress) {
            self.seen.lock().unwrap().push(progress);
        }
    }

    fn item(name: &str) -> WireItem {
        WireItem {
            name: name.into(),
            ..WireItem::default()
        }
    }

    #[test]
    fn a_second_scan_is_refused_while_one_is_running() {
        let state = AppState::new();
        claim(&state, "/fixtures/root".into(), CancelToken::new()).expect("the first claim");

        let error = claim(&state, "/fixtures/other".into(), CancelToken::new())
            .expect_err("the second claim");
        assert!(error.contains("already running"), "got: {error}");
    }

    #[test]
    fn a_claim_nothing_honours_is_released_rather_than_stranded() {
        let state = AppState::new();
        claim(&state, "/fixtures/root".into(), CancelToken::new()).expect("the first claim");

        // What `start` does when the thread fails to spawn. Without it the app
        // refuses every later scan until the window is restarted.
        release(&state);

        assert!(state.scan().active.is_none());
        claim(&state, "/fixtures/root".into(), CancelToken::new())
            .expect("scanning must still be possible after a failed spawn");
    }

    #[test]
    fn each_scan_gets_an_id_no_earlier_scan_used() {
        let state = AppState::new();

        let first = claim(&state, "/fixtures/root".into(), CancelToken::new()).expect("first");
        release(&state);
        let second = claim(&state, "/fixtures/root".into(), CancelToken::new()).expect("second");

        assert_ne!(
            first, second,
            "a reused id would let a node id from the previous scan resolve"
        );
    }

    #[test]
    fn cancelling_with_nothing_running_says_so() {
        assert!(!cancel(&AppState::new()), "there was no scan to stop");
    }

    #[test]
    fn cancelling_trips_the_token_the_worker_was_given() {
        let state = AppState::new();
        let token = CancelToken::new();
        claim(&state, "/fixtures/root".into(), token.clone()).expect("a claim");

        assert!(cancel(&state));
        assert!(
            token.is_cancelled(),
            "the stop button must trip the token the scan is watching"
        );
    }

    #[test]
    fn the_first_entry_reports_immediately() {
        let recorder = Recorder::default();
        let mut sink = ProgressSink::new(&recorder);

        sink.open_dir(item("/fixtures/root"));

        let seen = recorder.seen.lock().unwrap();
        assert_eq!(seen.len(), 1, "a scan that reports nothing looks hung");
        assert_eq!(seen[0].scanned, 1);
    }

    #[test]
    fn progress_is_reported_periodically_not_per_entry() {
        let recorder = Recorder::default();
        let mut sink = ProgressSink::new(&recorder);

        sink.open_dir(item("/fixtures/root"));
        for index in 0..PROGRESS_EVERY {
            sink.item(item(&format!("file-{index}")));
        }

        let seen = recorder.seen.lock().unwrap();
        assert_eq!(
            seen.len(),
            2,
            "one report at the first entry and one at the interval, not {}",
            PROGRESS_EVERY + 1
        );
        assert_eq!(seen[1].scanned, PROGRESS_EVERY + 1);
    }

    #[test]
    fn progress_names_the_directory_being_walked() {
        let recorder = Recorder::default();
        let mut sink = ProgressSink::new(&recorder);

        sink.open_dir(item("/fixtures/root"));
        sink.next_report = sink.seen + 1;
        sink.open_dir(item("nested"));

        // Built with PathBuf rather than written out, because the separator is
        // the platform's: this is `/fixtures/root\nested` on Windows, and a
        // literal here would assert that Nirmoka only runs on Unix.
        let expected = PathBuf::from("/fixtures/root").join("nested");

        let seen = recorder.seen.lock().unwrap();
        assert_eq!(seen[1].current_path, expected.display().to_string());
    }

    #[test]
    fn leaving_a_directory_pops_it_from_the_reported_path() {
        let recorder = Recorder::default();
        let mut sink = ProgressSink::new(&recorder);

        sink.open_dir(item("/fixtures/root"));
        sink.open_dir(item("nested"));
        sink.close_dir();
        sink.next_report = sink.seen + 1;
        sink.item(item("after"));

        let seen = recorder.seen.lock().unwrap();
        assert_eq!(
            seen.last().unwrap().current_path,
            "/fixtures/root",
            "the stack must unwind, or every scan looks stuck in its deepest directory"
        );
    }

    #[test]
    fn the_wrapped_sink_still_builds_a_tree() {
        let recorder = Recorder::default();
        let mut sink = ProgressSink::new(&recorder);

        sink.open_dir(item("/fixtures/root"));
        sink.item(WireItem {
            name: "file".into(),
            disk_bytes: 4096,
            ..WireItem::default()
        });
        sink.close_dir();

        let tree = sink.into_inner().finish();
        let root = tree.root().expect("a root");
        assert_eq!(tree.get(root).unwrap().total_bytes, 4096);
    }
}
