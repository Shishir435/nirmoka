//! Tests that exercise a real supported gdu when one is installed.

use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use nirmoka_adapter::{Adapter, AdapterError, CancelToken, Detection, ScanOptions, TreeSink};
use nirmoka_adapter_gdu::GduAdapter;

fn usable_backend() -> Option<GduAdapter> {
    let adapter = GduAdapter::new();
    match adapter.detect() {
        Ok(Detection::Found { .. }) => Some(adapter),
        Ok(Detection::UnsupportedVersion { version, .. }) => {
            eprintln!("skipping: gdu {version} is outside the supported range");
            None
        }
        Ok(Detection::NotInstalled) => {
            eprintln!("skipping: gdu is not installed");
            None
        }
        Err(error) => {
            eprintln!("skipping: gdu detection failed: {error}");
            None
        }
    }
}

struct Fixture(PathBuf);

impl Fixture {
    fn new(tag: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("nirmoka-gdu-live-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("deep/deeper")).unwrap();
        std::fs::create_dir_all(root.join("empty")).unwrap();
        std::fs::write(root.join("deep/big.bin"), vec![0u8; 200 * 1024]).unwrap();
        std::fs::write(root.join("deep/deeper/small.txt"), b"small").unwrap();
        Self(root)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn detection_reports_the_binary_that_will_run() {
    let Some(adapter) = usable_backend() else {
        return;
    };

    match adapter.detect().unwrap() {
        Detection::Found { path, version } => {
            assert!(path.is_absolute(), "{}", path.display());
            assert!(path.exists());
            assert!(version.starts_with("5.32."));
        }
        other => panic!("expected Found, got {other:?}"),
    }
}

#[test]
fn scans_a_real_directory_into_the_shared_tree() {
    let Some(adapter) = usable_backend() else {
        return;
    };

    let fixture = Fixture::new("scan");
    let mut sink = TreeSink::new();
    let summary = adapter
        .scan(
            &fixture.0,
            &ScanOptions::default(),
            &mut sink,
            &CancelToken::new(),
        )
        .expect("scan succeeds");

    let tree = sink.finish();
    let root = tree.root().expect("a scanned tree has a root");
    assert_eq!(summary.items, 6, "unexpected entry count in {tree:?}");
    assert_eq!(summary.directories, 4);
    assert_eq!(summary.root, fixture.0.canonicalize().unwrap());
    assert!(summary
        .backend_version
        .as_deref()
        .is_some_and(|version| version.starts_with("5.32.")));
    assert!(tree.get(root).unwrap().total_bytes >= 200 * 1024);
    assert_eq!(
        tree.get(tree.children_by_size(root)[0]).unwrap().name,
        "deep"
    );
}

#[test]
fn cancelling_returns_without_waiting_for_a_whole_disk_scan() {
    let Some(adapter) = usable_backend() else {
        return;
    };

    let root = if cfg!(windows) { "C:\\" } else { "/" };
    let cancel = CancelToken::new();
    let canceller = {
        let cancel = cancel.clone();
        thread::spawn(move || {
            // Detection and spawn are fast compared with a whole-disk scan.
            // The delay puts cancellation on the running-process path even
            // though gdu emits its JSON only after analysis completes.
            thread::sleep(Duration::from_millis(250));
            cancel.cancel();
        })
    };

    let began = Instant::now();
    let error = adapter
        .scan(
            root.as_ref(),
            &ScanOptions::default(),
            &mut TreeSink::new(),
            &cancel,
        )
        .expect_err("a cancelled scan must not report success");
    canceller.join().unwrap();

    assert!(matches!(error, AdapterError::Cancelled { .. }), "{error}");
    assert!(began.elapsed() < Duration::from_secs(60));
}
