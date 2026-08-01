//! Tests that need a real ncdu 2.x on the machine.
//!
//! They skip themselves where the backend is missing or too old — Windows has
//! no ncdu at all, and Ubuntu still ships the 1.x series — so the suite stays
//! green everywhere while still exercising the real thing on macOS CI and on a
//! developer's machine. The parser itself is covered by fixtures in
//! `tests/contract`, which need no backend.

use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use nirmoka_adapter::wire::{WireItem, WireSink};
use nirmoka_adapter::{Adapter, AdapterError, CancelToken, Detection, ScanOptions, TreeSink};
use nirmoka_adapter_ncdu::NcduAdapter;

/// `None` when this machine cannot run the test, with a printed reason so a
/// silent skip cannot be mistaken for a pass.
fn usable_backend() -> Option<NcduAdapter> {
    let adapter = NcduAdapter::new();
    match adapter.detect() {
        Ok(Detection::Found { .. }) => Some(adapter),
        Ok(Detection::UnsupportedVersion { version, .. }) => {
            eprintln!("skipping: ncdu {version} is outside the supported range");
            None
        }
        Ok(Detection::NotInstalled) => {
            eprintln!("skipping: ncdu is not installed");
            None
        }
        Err(error) => {
            eprintln!("skipping: ncdu detection failed: {error}");
            None
        }
    }
}

/// A small tree with the awkward cases in it, built fresh so the test does not
/// depend on whatever happens to be in the repository.
struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!("nirmoka-live-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("deep/deeper")).unwrap();
        std::fs::create_dir_all(root.join("empty")).unwrap();
        std::fs::write(root.join("deep/big.bin"), vec![0u8; 200 * 1024]).unwrap();
        std::fs::write(root.join("deep/deeper/small.txt"), b"small").unwrap();
        Self { root }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn detection_reports_an_absolute_path() {
    let Some(adapter) = usable_backend() else {
        return;
    };

    match adapter.detect().unwrap() {
        Detection::Found { path, version } => {
            assert!(
                path.is_absolute(),
                "detection must name the binary that will run, got {}",
                path.display()
            );
            assert!(path.exists());
            assert!(version.starts_with('2'));
        }
        other => panic!("expected Found, got {other:?}"),
    }
}

#[test]
fn scans_a_real_directory_into_a_tree() {
    let Some(adapter) = usable_backend() else {
        return;
    };

    let fixture = Fixture::new("scan");
    let mut sink = TreeSink::new();
    let summary = adapter
        .scan(
            &fixture.root,
            &ScanOptions::default(),
            &mut sink,
            &CancelToken::new(),
        )
        .expect("scan succeeds");

    let stats = sink.stats();
    let tree = sink.finish();
    let root = tree.root().expect("a scanned tree has a root");

    // 4 directories (root, deep, deeper, empty) + 2 files.
    assert_eq!(summary.items, 6, "unexpected entry count in {tree:?}");
    assert_eq!(summary.directories, 4);
    assert_eq!(
        summary.root.canonicalize().unwrap(),
        fixture.root.canonicalize().unwrap()
    );
    assert!(summary.backend_version.is_some());
    assert_eq!(stats.read_errors, 0);

    // The 200 KiB file has to dominate the total; anything else means sizes are
    // not being rolled up.
    assert!(tree.get(root).unwrap().total_bytes >= 200 * 1024);

    let biggest = tree.children_by_size(root)[0];
    assert_eq!(tree.get(biggest).unwrap().name, "deep");

    let deep_children = tree.children_by_size(biggest);
    let big = deep_children
        .iter()
        .find(|id| tree.get(**id).unwrap().name == "big.bin")
        .expect("big.bin is in the tree");
    assert_eq!(
        tree.path_of(*big).unwrap().canonicalize().unwrap(),
        fixture.root.canonicalize().unwrap().join("deep/big.bin")
    );
}

#[test]
fn exclusions_are_flagged_rather_than_dropped() {
    let Some(adapter) = usable_backend() else {
        return;
    };

    let fixture = Fixture::new("exclude");
    let mut sink = TreeSink::new();
    adapter
        .scan(
            &fixture.root,
            &ScanOptions {
                exclude: vec!["big.bin".to_string()],
                ..ScanOptions::default()
            },
            &mut sink,
            &CancelToken::new(),
        )
        .expect("scan succeeds");

    let stats = sink.stats();
    let tree = sink.finish();

    assert_eq!(stats.excluded, 1);
    // The excluded entry is still in the tree. A cleanup tool that silently
    // omits what it skipped shows a total that cannot be explained.
    assert!(tree
        .children_by_size(tree.root().unwrap())
        .iter()
        .flat_map(|dir| tree.children_of(*dir))
        .any(|id| tree.get(*id).unwrap().excluded));
}

#[test]
fn cancelling_a_scan_stops_it_and_says_so() {
    let Some(adapter) = usable_backend() else {
        return;
    };

    // The filesystem root is the one directory guaranteed to take long enough
    // to cancel. Nothing is written and ncdu is read-only here.
    let root = if cfg!(windows) { "C:\\" } else { "/" };

    let cancel = CancelToken::new();
    let (started_tx, started_rx) = mpsc::channel();

    let canceller = {
        let cancel = cancel.clone();
        thread::spawn(move || {
            // Wait until the scan has actually produced something, so this
            // tests cancellation of a running backend rather than the
            // pre-flight check.
            let _ = started_rx.recv_timeout(Duration::from_secs(30));
            cancel.cancel();
        })
    };

    struct NotifyOnFirstEntry {
        inner: TreeSink,
        started: Option<mpsc::Sender<()>>,
    }

    impl WireSink for NotifyOnFirstEntry {
        fn open_dir(&mut self, item: WireItem) {
            if let Some(started) = self.started.take() {
                let _ = started.send(());
            }
            self.inner.open_dir(item);
        }
        fn item(&mut self, item: WireItem) {
            self.inner.item(item);
        }
        fn close_dir(&mut self) {
            self.inner.close_dir();
        }
    }

    let mut sink = NotifyOnFirstEntry {
        inner: TreeSink::new(),
        started: Some(started_tx),
    };

    let began = Instant::now();
    let error = adapter
        .scan(root.as_ref(), &ScanOptions::default(), &mut sink, &cancel)
        .expect_err("a cancelled scan must not report success");
    let elapsed = began.elapsed();

    canceller.join().unwrap();

    assert!(
        error.is_cancellation(),
        "expected cancellation, got {error}"
    );
    assert!(matches!(error, AdapterError::Cancelled { .. }));
    // Scanning a whole disk takes minutes. Returning in seconds is the proof
    // that the subprocess was killed rather than waited out.
    assert!(
        elapsed < Duration::from_secs(60),
        "cancellation took {elapsed:?}"
    );
}
