use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(unix)]
use std::thread;
#[cfg(unix)]
use std::time::{Duration, Instant};

use nirmoka_adapter::{Adapter, CancelToken, DeleteMode};
use nirmoka_adapter_rip::RipAdapter;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let nonce = NEXT.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("nirmoka-rip-{name}-{}-{nonce}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn advertises_only_exact_undo_for_existing_receipts() {
    let caps = RipAdapter::new().capabilities();
    assert!(!caps.scan);
    assert!(!caps.delete);
    assert!(!caps.trash);
    assert!(caps.undo);
    assert!(!caps.dry_run);
}

#[test]
fn refuses_new_selected_path_deletions() {
    let dir = TempDir::new("validation");
    let target = dir.join("target");
    fs::write(&target, b"data").unwrap();

    let adapter = RipAdapter::new();
    assert!(adapter
        .prepare_delete(&dir.0, &target, DeleteMode::Trash)
        .is_err());
    assert!(adapter
        .prepare_delete(&dir.0, &target, DeleteMode::Permanent)
        .is_err());

    let plan = nirmoka_adapter::DeletePlan::new("rip", dir.0.clone(), target, DeleteMode::Trash);
    assert!(adapter.delete(&plan, &CancelToken::new()).is_err());
}

#[cfg(unix)]
#[test]
fn undoes_an_existing_receipt_through_the_backend() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new("round-trip");
    let binary = dir.join("rip");
    let recovery = dir.join("recovery");
    let root = dir.join("scan");
    let target = root.join("file.txt");
    let operation_root = recovery.join("existing-operation");
    let recovery_path = operation_root.join(target.strip_prefix("/").unwrap());
    fs::create_dir_all(recovery_path.parent().unwrap()).unwrap();
    fs::write(&recovery_path, b"important").unwrap();

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

    let adapter = RipAdapter::with_binary_and_recovery_root(binary, recovery);
    let receipt =
        nirmoka_adapter::DeleteReceipt::new("rip", target.clone(), operation_root, recovery_path);

    adapter.undo(&receipt, &CancelToken::new()).unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"important");
    assert!(!receipt.recovery_path().exists());
}

#[test]
fn real_backend_detects_when_explicitly_provided() {
    let Some(binary) = std::env::var_os("NIRMOKA_RIP_BIN") else {
        return;
    };
    let dir = TempDir::new("real-round-trip");
    let adapter =
        RipAdapter::with_binary_and_recovery_root(PathBuf::from(binary), dir.join("recovery"));
    let detection = adapter.detect().expect("real rip detects");
    assert!(detection.is_usable(), "got {detection:?}");
}

#[cfg(unix)]
#[test]
fn cancelling_undo_kills_the_destructive_subprocess() {
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};

    let dir = TempDir::new("cancel");
    let binary = dir.join("rip");
    let pid_file = dir.join("pid");
    let recovery = dir.join("recovery");
    let root = dir.join("scan");
    let target = root.join("file.txt");
    let operation_root = recovery.join("existing-operation");
    let recovery_path = operation_root.join(target.strip_prefix("/").unwrap());
    fs::create_dir_all(recovery_path.parent().unwrap()).unwrap();
    fs::write(&recovery_path, b"important").unwrap();
    fs::write(
        &binary,
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "rm-improved 0.13.1"
  exit 0
fi
echo $$ > "$(dirname "$0")/pid"
exec sleep 30
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&binary).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&binary, permissions).unwrap();

    let adapter = RipAdapter::with_binary_and_recovery_root(binary, recovery);
    let receipt = nirmoka_adapter::DeleteReceipt::new("rip", target, operation_root, recovery_path);
    let cancel = CancelToken::new();
    let worker_cancel = cancel.clone();
    let began = Instant::now();
    let worker = thread::spawn(move || adapter.undo(&receipt, &worker_cancel));

    let deadline = Instant::now() + Duration::from_secs(5);
    while !pid_file.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    let pid = fs::read_to_string(&pid_file)
        .expect("backend started")
        .trim()
        .to_string();
    cancel.cancel();

    let error = worker.join().unwrap().expect_err("undo was cancelled");
    assert!(error.is_cancellation(), "got {error}");
    assert!(began.elapsed() < Duration::from_secs(5));
    assert!(
        !Command::new("kill")
            .arg("-0")
            .arg(pid)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success(),
        "the destructive backend process survived cancellation"
    );
}
