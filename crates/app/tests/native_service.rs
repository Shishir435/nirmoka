//! Lifecycle coverage for the actual private helper process.
#![cfg(unix)]

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn closing_the_native_client_stops_its_scanner_before_the_helper_exits() {
    let root = std::env::temp_dir().join(format!("nirmoka-native-shutdown-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let backend = root.join("ncdu");
    let pid_file = root.join("scanner.pid");
    fs::write(&backend, "#!/bin/sh\nif [ \"$1\" = --version ]; then echo 'ncdu 2.8.2'; exit 0; fi\nprintf '%s' \"$$\" > \"$NIRMOKA_TEST_SCANNER_PID\"\nexec sleep 30\n").unwrap();
    fs::set_permissions(&backend, fs::Permissions::from_mode(0o755)).unwrap();
    let mut service = Command::new(env!("CARGO_BIN_EXE_nirmoka-native-service"))
        .env("PATH", format!("{}:/usr/bin:/bin", root.display()))
        .env("NIRMOKA_TEST_SCANNER_PID", &pid_file)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut input = service.stdin.take().unwrap();
    let mut output = BufReader::new(service.stdout.take().unwrap());
    writeln!(
        input,
        "{}",
        serde_json::json!({ "id": 1, "method": "startScan", "params": { "rootPath": root } })
    )
    .unwrap();
    input.flush().unwrap();
    let mut response = String::new();
    output.read_line(&mut response).unwrap();
    assert!(!response.contains("\"error\""), "{response}");
    let started = Instant::now();
    while !pid_file.exists() && started.elapsed() < Duration::from_secs(5) {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(pid_file.exists(), "the scanner must actually start");
    let pid = fs::read_to_string(&pid_file).unwrap();
    drop(input);
    let stopped = Instant::now();
    loop {
        if let Some(status) = service.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if stopped.elapsed() > Duration::from_secs(5) {
            let _ = service.kill();
            let _ = service.wait();
            panic!("the native helper did not finish cancellation");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let alive = Command::new("kill")
        .args(["-0", pid.trim()])
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(
        !alive.success(),
        "closing the native window orphaned its scanner"
    );
    fs::remove_dir_all(root).unwrap();
}
