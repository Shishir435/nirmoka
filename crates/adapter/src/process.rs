//! Subprocess plumbing shared by every adapter.
//!
//! Two problems live here, both of which every adapter would otherwise solve
//! slightly differently:
//!
//! - **Where is the binary?** Detection must report a real path, not the
//!   command name it hoped to run.
//! - **How does a scan stop?** Cancellation has to kill the child. A backend
//!   left churning a disk after the user pressed stop is worse than a slow
//!   scan, because nothing on screen explains it.

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// How often the watcher checks for cancellation. Small enough that stopping a
/// scan feels immediate, large enough to be free.
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// A stop signal shared between whoever runs a scan and whoever cancels it.
///
/// Cloning shares the flag, so a UI can hand a clone to a worker thread.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    flag: Arc<AtomicBool>,
}

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}

/// A spawned backend, with a watcher thread that kills it on cancellation.
///
/// The child is wrapped in a mutex because killing it happens on the watcher
/// thread while the caller is blocked reading its stdout on this one.
pub struct RunningProcess {
    child: Arc<Mutex<Child>>,
    stdout: Option<ChildStdout>,
    stderr: Option<JoinHandle<String>>,
    watcher: Option<JoinHandle<()>>,
    finished: Arc<AtomicBool>,
    cancel: CancelToken,
}

/// What a finished process left behind.
pub struct Outcome {
    pub status: ExitStatus,
    pub stderr: String,
    /// The watcher killed it. The exit status is then meaningless.
    pub cancelled: bool,
}

impl RunningProcess {
    /// Spawn `command` with piped stdout and stderr.
    pub fn spawn(command: &mut Command, cancel: &CancelToken) -> io::Result<Self> {
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take();

        // Drained on its own thread. A backend that fills the stderr pipe while
        // nobody reads it blocks forever, and the symptom is a scan that hangs
        // at a random percentage.
        let stderr = child.stderr.take().map(|mut pipe| {
            thread::spawn(move || {
                let mut buffer = String::new();
                use std::io::Read;
                let _ = pipe.read_to_string(&mut buffer);
                buffer
            })
        });

        let child = Arc::new(Mutex::new(child));
        let finished = Arc::new(AtomicBool::new(false));

        let watcher = {
            let child = Arc::clone(&child);
            let finished = Arc::clone(&finished);
            let cancel = cancel.clone();
            thread::spawn(move || loop {
                if cancel.is_cancelled() {
                    if let Ok(mut child) = child.lock() {
                        // Already-exited is not an error worth reporting: the
                        // race between "user cancelled" and "backend finished"
                        // is expected.
                        let _ = child.kill();
                    }
                    return;
                }
                if finished.load(Ordering::SeqCst) {
                    return;
                }
                thread::sleep(POLL_INTERVAL);
            })
        };

        Ok(Self {
            child,
            stdout,
            stderr,
            watcher: Some(watcher),
            finished,
            cancel: cancel.clone(),
        })
    }

    /// The child's stdout, once. The caller streams the wire format out of it.
    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.stdout.take()
    }

    /// Wait for the child, stop the watcher, and collect stderr.
    ///
    /// Must be called even on the error path — dropping a `RunningProcess`
    /// without it leaves a zombie until this process exits.
    pub fn finish(mut self) -> io::Result<Outcome> {
        // Release the pipe first: a backend blocked writing to stdout never
        // exits, so waiting before dropping it would deadlock.
        drop(self.stdout.take());

        // Polled rather than a blocking `wait`, because `wait` needs the child
        // mutex for its whole duration and the watcher needs that same mutex to
        // kill. Holding it here would deadlock the two threads against each
        // other, and the symptom is a cancel button that waits for the scan it
        // was supposed to stop.
        let status = loop {
            let waited = self
                .child
                .lock()
                .expect("child mutex poisoned")
                .try_wait()?;

            match waited {
                Some(status) => break status,
                None => thread::sleep(POLL_INTERVAL),
            }
        };

        self.finished.store(true, Ordering::SeqCst);
        if let Some(watcher) = self.watcher.take() {
            let _ = watcher.join();
        }

        let stderr = self
            .stderr
            .take()
            .and_then(|handle| handle.join().ok())
            .unwrap_or_default();

        Ok(Outcome {
            status,
            stderr: stderr.trim().to_string(),
            cancelled: self.cancel.is_cancelled(),
        })
    }

    #[cfg(test)]
    fn id(&self) -> u32 {
        self.child.lock().expect("child mutex poisoned").id()
    }
}

impl Drop for RunningProcess {
    fn drop(&mut self) {
        // Only reached when `finish` was skipped, which is a bug on a path that
        // returned early. Kill rather than orphan.
        if self.watcher.is_some() {
            if let Ok(mut child) = self.child.lock() {
                let _ = child.kill();
                let _ = child.wait();
            }
            self.finished.store(true, Ordering::SeqCst);
            if let Some(watcher) = self.watcher.take() {
                let _ = watcher.join();
            }
        }
    }
}

/// Absolute path of `binary`, searched the way the shell would.
///
/// Detection reporting `"ncdu"` instead of `/opt/homebrew/bin/ncdu` is not good
/// enough: on a machine with several copies installed, the user needs to know
/// which one is about to be handed a delete command.
pub fn find_in_path(binary: &str) -> Option<PathBuf> {
    search_path(
        binary,
        std::env::var_os("PATH").as_deref(),
        std::env::var_os("PATHEXT").as_deref(),
    )
}

/// The searchable core of [`find_in_path`], with the environment passed in so
/// it can be tested without mutating process-global state.
fn search_path(binary: &str, path: Option<&OsStr>, pathext: Option<&OsStr>) -> Option<PathBuf> {
    let path = path?;

    for directory in std::env::split_paths(path) {
        if directory.as_os_str().is_empty() {
            continue;
        }

        for candidate in candidates(&directory.join(binary), pathext) {
            if is_executable_file(&candidate) {
                return Some(candidate);
            }
        }
    }

    None
}

/// On Windows an executable is `ncdu.exe`, or whatever `PATHEXT` says. On Unix
/// the name is the name.
fn candidates(base: &Path, pathext: Option<&OsStr>) -> Vec<PathBuf> {
    let mut candidates = vec![base.to_path_buf()];

    if cfg!(windows) {
        let extensions = pathext
            .map(OsString::from)
            .unwrap_or_else(|| OsString::from(".COM;.EXE;.BAT;.CMD"));

        for extension in extensions.to_string_lossy().split(';') {
            let extension = extension.trim();
            if extension.is_empty() {
                continue;
            }
            let mut name = base.as_os_str().to_os_string();
            name.push(extension);
            candidates.push(PathBuf::from(name));
        }
    }

    candidates
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };

    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }

    // Windows has no execute bit; the extension is the whole test, and
    // `candidates` already applied it.
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// A child that will not exit on its own, so cancellation is the only way
    /// the test can finish.
    #[cfg(unix)]
    fn sleeper() -> Command {
        let mut command = Command::new("sleep");
        command.arg("60");
        command
    }

    #[cfg(windows)]
    fn sleeper() -> Command {
        let mut command = Command::new("cmd");
        command.args(["/C", "ping", "-n", "60", "127.0.0.1"]);
        command
    }

    #[cfg(unix)]
    fn echoer() -> Command {
        let mut command = Command::new("echo");
        command.arg("hello");
        command
    }

    #[cfg(windows)]
    fn echoer() -> Command {
        let mut command = Command::new("cmd");
        command.args(["/C", "echo", "hello"]);
        command
    }

    #[cfg(unix)]
    fn process_exists(pid: u32) -> bool {
        // Signal 0 checks for existence without delivering anything. A reaped
        // child fails with ESRCH; a zombie would still be found, which is the
        // failure this is looking for.
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    #[test]
    fn cancelling_kills_the_subprocess() {
        let cancel = CancelToken::new();
        let process = RunningProcess::spawn(&mut sleeper(), &cancel).unwrap();
        let pid = process.id();

        let started = Instant::now();
        cancel.cancel();
        let outcome = process.finish().unwrap();

        assert!(outcome.cancelled);
        // The child sleeps for a minute. Returning promptly is the whole point.
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "cancellation took {:?}",
            started.elapsed()
        );

        #[cfg(unix)]
        {
            // Killed by a signal, not exited: proof it did not finish on its own.
            assert_eq!(outcome.status.code(), None);
            assert!(!process_exists(pid), "pid {pid} survived cancellation");
        }
        #[cfg(not(unix))]
        {
            let _ = pid;
            assert!(!outcome.status.success());
        }
    }

    #[test]
    fn dropping_without_finishing_still_kills_the_subprocess() {
        let cancel = CancelToken::new();
        let pid = {
            let process = RunningProcess::spawn(&mut sleeper(), &cancel).unwrap();
            let pid = process.id();
            drop(process);
            pid
        };

        #[cfg(unix)]
        assert!(
            !process_exists(pid),
            "pid {pid} was orphaned by an early return"
        );
        #[cfg(not(unix))]
        let _ = pid;
    }

    #[test]
    fn an_uncancelled_run_reports_its_output_and_status() {
        use std::io::Read;

        let cancel = CancelToken::new();
        let mut process = RunningProcess::spawn(&mut echoer(), &cancel).unwrap();

        let mut stdout = String::new();
        process
            .take_stdout()
            .unwrap()
            .read_to_string(&mut stdout)
            .unwrap();

        let outcome = process.finish().unwrap();
        assert!(!outcome.cancelled);
        assert!(outcome.status.success());
        assert_eq!(stdout.trim(), "hello");
    }

    #[test]
    fn a_missing_binary_is_an_error_not_a_hang() {
        let cancel = CancelToken::new();
        let mut command = Command::new("nirmoka-no-such-binary");
        match RunningProcess::spawn(&mut command, &cancel) {
            Ok(_) => panic!("a nonexistent binary must not spawn"),
            Err(error) => assert_eq!(error.kind(), io::ErrorKind::NotFound),
        }
    }

    // -- path search --------------------------------------------------------

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!("nirmoka-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn write_executable(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, b"#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    #[test]
    fn finds_a_binary_in_the_first_matching_directory() {
        let first = TempDir::new("path-a");
        let second = TempDir::new("path-b");

        let name = if cfg!(windows) { "tool.exe" } else { "tool" };
        let expected = write_executable(&first.0, name);
        write_executable(&second.0, name);

        let path = std::env::join_paths([&first.0, &second.0]).unwrap();
        assert_eq!(
            search_path("tool", Some(path.as_os_str()), None),
            Some(expected)
        );
    }

    #[test]
    fn ignores_directories_and_non_executables() {
        let dir = TempDir::new("path-c");
        std::fs::create_dir_all(dir.0.join("tool")).unwrap();

        let path = std::env::join_paths([&dir.0]).unwrap();
        assert_eq!(search_path("tool", Some(path.as_os_str()), None), None);
    }

    #[test]
    fn reports_nothing_when_path_is_unset() {
        assert_eq!(search_path("tool", None, None), None);
    }

    #[test]
    #[cfg(unix)]
    fn a_non_executable_file_is_not_a_binary() {
        let dir = TempDir::new("path-d");
        std::fs::write(dir.0.join("tool"), b"not executable").unwrap();

        let path = std::env::join_paths([&dir.0]).unwrap();
        assert_eq!(search_path("tool", Some(path.as_os_str()), None), None);
    }

    #[test]
    #[cfg(windows)]
    fn applies_pathext_on_windows() {
        let dir = TempDir::new("path-e");
        let expected = write_executable(&dir.0, "tool.cmd");

        let path = std::env::join_paths([&dir.0]).unwrap();
        assert_eq!(
            search_path(
                "tool",
                Some(path.as_os_str()),
                Some(OsStr::new(".EXE;.CMD"))
            ),
            Some(expected)
        );
    }
}
