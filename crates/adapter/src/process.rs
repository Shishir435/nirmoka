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
#[derive(Debug)]
pub struct RunningProcess {
    child: Arc<Mutex<Child>>,
    stdout: Option<ChildStdout>,
    stderr: Option<JoinHandle<String>>,
    watcher: Option<JoinHandle<()>>,
    finished: Arc<AtomicBool>,
    cancel: CancelToken,
}

/// What a finished process left behind.
#[derive(Debug)]
pub struct Outcome {
    pub status: ExitStatus,
    pub stderr: String,
    /// The watcher killed it. The exit status is then meaningless.
    pub cancelled: bool,
}

impl RunningProcess {
    /// Spawn `command` with piped stdout and stderr, and nothing on stdin.
    pub fn spawn(command: &mut Command, cancel: &CancelToken) -> io::Result<Self> {
        Self::spawn_inner(command, None, cancel)
    }

    /// Spawn `command` with `input` written to its stdin, which is then closed.
    ///
    /// For a backend whose only way to accept a decision is a prompt it reads
    /// from stdin. The bytes are fixed at the call site and written once: this
    /// is not a channel, and no adapter may hold it open to converse with a
    /// backend's interactive flow.
    ///
    /// **Nothing here decides what to answer.** A caller that writes a
    /// confirmation must already hold the user's explicit approval for the exact
    /// operation being run — see `MoleAdapter::execute_uninstall`, the only
    /// caller, and ADR 0027 for why that approval is what makes this legitimate
    /// rather than an adapter answering a safety prompt on a user's behalf.
    pub fn spawn_with_input(
        command: &mut Command,
        input: &[u8],
        cancel: &CancelToken,
    ) -> io::Result<Self> {
        Self::spawn_inner(command, Some(input.to_vec()), cancel)
    }

    fn spawn_inner(
        command: &mut Command,
        input: Option<Vec<u8>>,
        cancel: &CancelToken,
    ) -> io::Result<Self> {
        let mut child = command
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Written on its own thread, for the same reason stderr is drained on
        // one: a backend that reads no stdin until after it has filled the
        // stdout pipe would deadlock against a write on this thread. Dropping
        // the handle at the end of the closure is what delivers EOF, and a
        // backend waiting on a line will not proceed without it.
        if let (Some(mut pipe), Some(input)) = (child.stdin.take(), input) {
            thread::spawn(move || {
                use std::io::Write;
                let _ = pipe.write_all(&input);
                let _ = pipe.flush();
            });
        }

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

/// Where a windowed process has to look because `PATH` will not tell it.
///
/// An application launched from Finder, Launchpad, or Spotlight inherits
/// launchd's environment, and that `PATH` is `/usr/bin:/bin:/usr/sbin:/sbin` —
/// read off the released 0.1.0 build, not assumed. Every backend Nirmoka drives
/// is installed by a package manager into a directory that list does not
/// contain, so trusting `PATH` alone means reporting "not installed" for an
/// ncdu sitting in `/opt/homebrew/bin`. From a terminal the same code works,
/// which is what makes this invisible in development.
///
/// Searched *after* `PATH`, so an explicitly set environment still decides.
///
/// Linuxbrew's prefix is deliberately absent: it lives under `/home`, which the
/// invariant check forbids as a literal, and no Linux build is packaged
/// (ADR 0023). A Linux user launching from a shell has the real `PATH` anyway.
fn package_manager_dirs() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        &[
            "/opt/homebrew/bin",                 // Homebrew, Apple silicon
            "/usr/local/bin",                    // Homebrew, Intel
            "/opt/local/bin",                    // MacPorts
            "/run/current-system/sw/bin",        // nix-darwin
            "/nix/var/nix/profiles/default/bin", // Nix, single-user
        ]
    } else if cfg!(target_os = "linux") {
        &["/usr/local/bin", "/var/lib/snapd/snap/bin"]
    } else {
        // Windows installers put themselves on the machine `PATH`, which a
        // windowed process does inherit.
        &[]
    }
}

/// `PATH` with [`package_manager_dirs`] appended, for handing to a backend.
///
/// Detection resolves an absolute path, so finding the binary does not need
/// this — but the binary's own lookups do. Mole asks `brew` whether an
/// application is a cask, and under launchd's `PATH` it cannot, so it reports
/// every cask as `"source": "App"`: wrong data, no error, nothing to notice.
pub fn augmented_path() -> OsString {
    augment(
        std::env::var_os("PATH").unwrap_or_default().as_os_str(),
        package_manager_dirs(),
    )
}

/// The appending core of [`augmented_path`], with the environment passed in so
/// it can be tested against a known `PATH` rather than the machine's.
///
/// `existing` is copied through in order, duplicates and all. What someone put
/// in their `PATH` is their business; the only promise here is that every
/// directory in `extras` ends up present, and no more often than it already was.
///
/// Empty entries are the exception, and are dropped. An empty `PATH` splits into
/// one empty component, which POSIX reads as the current directory — so passing
/// it through would hand a backend a `PATH` beginning with wherever the app
/// happened to be launched from.
fn augment(existing: &OsStr, extras: &[&str]) -> OsString {
    let mut directories: Vec<PathBuf> = std::env::split_paths(existing)
        .filter(|directory| !directory.as_os_str().is_empty())
        .collect();

    for extra in extras {
        let extra = PathBuf::from(extra);
        if !directories.contains(&extra) {
            directories.push(extra);
        }
    }

    std::env::join_paths(directories).unwrap_or_else(|_| existing.to_os_string())
}

/// A [`Command`] for a backend: `program`, with a `PATH` it can work with.
///
/// Use this rather than `Command::new` for anything that runs a backend. See
/// [`augmented_path`] for what goes wrong otherwise.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    command.env("PATH", augmented_path());
    command
}

/// Absolute path of `binary`, searched the way the shell would — plus the
/// package-manager directories a windowed process never inherits.
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
    search_dirs(binary, path, pathext, package_manager_dirs())
}

/// `search_path` with the fallback directories injected, so a test can point
/// them at a temporary directory instead of the machine's real Homebrew.
fn search_dirs(
    binary: &str,
    path: Option<&OsStr>,
    pathext: Option<&OsStr>,
    extras: &[&str],
) -> Option<PathBuf> {
    // An absent `PATH` is not a reason to skip the fallbacks. It is a reason to
    // need them.
    let from_path = path
        .map(|path| std::env::split_paths(path).collect::<Vec<_>>())
        .unwrap_or_default();

    let searched = from_path
        .into_iter()
        .chain(extras.iter().map(PathBuf::from));

    let mut seen: Vec<PathBuf> = Vec::new();
    for directory in searched {
        if directory.as_os_str().is_empty() || seen.contains(&directory) {
            continue;
        }
        seen.push(directory.clone());

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
            // PATHEXT is conventionally uppercase (".COM;.EXE;.BAT"), and the
            // filesystem is case-insensitive, so appending it verbatim opens
            // the right file — and then reports `C:\…\gdu.EXE` to a user whose
            // disk says `gdu.exe`. Detection's whole job is naming the binary
            // that will run; shouting a different spelling of it undoes that.
            let mut name = base.as_os_str().to_os_string();
            name.push(extension.to_lowercase());
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
    fn reports_nothing_when_path_is_unset_and_no_fallback_has_it() {
        assert_eq!(search_dirs("tool", None, None, &[]), None);
    }

    /// The bug this exists for: a windowed process gets launchd's
    /// `/usr/bin:/bin:/usr/sbin:/sbin`, and every backend is installed
    /// somewhere else.
    #[test]
    fn a_package_manager_directory_is_searched_when_path_misses_it() {
        let brew = TempDir::new("path-brew");
        let name = if cfg!(windows) { "tool.exe" } else { "tool" };
        let expected = write_executable(&brew.0, name);

        let launchd = std::env::join_paths(["/usr/bin", "/bin"]).unwrap();
        let extras = [brew.0.to_str().unwrap()];

        assert_eq!(
            search_dirs("tool", Some(launchd.as_os_str()), None, &extras),
            Some(expected)
        );
    }

    #[test]
    fn path_still_decides_when_both_have_the_binary() {
        let from_path = TempDir::new("path-first");
        let fallback = TempDir::new("path-fallback");

        let name = if cfg!(windows) { "tool.exe" } else { "tool" };
        let expected = write_executable(&from_path.0, name);
        write_executable(&fallback.0, name);

        let path = std::env::join_paths([&from_path.0]).unwrap();
        let extras = [fallback.0.to_str().unwrap()];

        assert_eq!(
            search_dirs("tool", Some(path.as_os_str()), None, &extras),
            Some(expected)
        );
    }

    #[test]
    fn an_absent_path_is_a_reason_to_use_the_fallbacks_rather_than_give_up() {
        let fallback = TempDir::new("path-only-fallback");
        let name = if cfg!(windows) { "tool.exe" } else { "tool" };
        let expected = write_executable(&fallback.0, name);

        let extras = [fallback.0.to_str().unwrap()];
        assert_eq!(search_dirs("tool", None, None, &extras), Some(expected));
    }

    #[test]
    fn a_directory_named_twice_is_searched_once() {
        let dir = TempDir::new("path-dupe");
        let listed = dir.0.to_str().unwrap();

        let path = std::env::join_paths([&dir.0, &dir.0]).unwrap();
        // Nothing to find, so this asserts only that duplication is tolerated
        // and the fallback list agreeing with PATH is not an error.
        assert_eq!(
            search_dirs("tool", Some(path.as_os_str()), None, &[listed]),
            None
        );
    }

    fn split(path: &OsStr) -> Vec<PathBuf> {
        std::env::split_paths(path).collect()
    }

    #[test]
    fn augmenting_appends_what_is_missing_and_keeps_the_order() {
        let augmented = augment(
            &std::env::join_paths(["/usr/bin", "/bin"]).unwrap(),
            &["/opt/homebrew/bin", "/opt/local/bin"],
        );

        assert_eq!(
            split(&augmented),
            vec![
                PathBuf::from("/usr/bin"),
                PathBuf::from("/bin"),
                PathBuf::from("/opt/homebrew/bin"),
                PathBuf::from("/opt/local/bin"),
            ]
        );
    }

    #[test]
    fn a_directory_already_in_path_is_not_appended_again() {
        let augmented = augment(
            &std::env::join_paths(["/opt/homebrew/bin", "/usr/bin"]).unwrap(),
            &["/opt/homebrew/bin"],
        );

        assert_eq!(
            split(&augmented),
            vec![
                PathBuf::from("/opt/homebrew/bin"),
                PathBuf::from("/usr/bin")
            ]
        );
    }

    /// A `PATH` that lists something twice is the operator's business. Asserting
    /// otherwise would fail on their machine rather than on a bug here — which
    /// is what the first version of this test did.
    #[test]
    fn an_inherited_duplicate_is_left_alone() {
        let inherited = std::env::join_paths(["/usr/bin", "/bin", "/usr/bin"]).unwrap();
        let augmented = augment(&inherited, &["/opt/homebrew/bin"]);

        assert_eq!(
            split(&augmented),
            vec![
                PathBuf::from("/usr/bin"),
                PathBuf::from("/bin"),
                PathBuf::from("/usr/bin"),
                PathBuf::from("/opt/homebrew/bin"),
            ]
        );
    }

    /// An empty `PATH` splits into one empty component, and an empty component
    /// means the current directory. Passing that on would let whatever directory
    /// the app was launched from supply a backend.
    #[test]
    fn an_empty_path_becomes_the_fallbacks_alone() {
        let augmented = augment(OsStr::new(""), &["/opt/homebrew/bin"]);
        assert_eq!(split(&augmented), vec![PathBuf::from("/opt/homebrew/bin")]);
    }

    #[test]
    fn an_empty_entry_anywhere_in_path_is_dropped() {
        let augmented = augment(OsStr::new("/usr/bin::/bin"), &["/opt/homebrew/bin"]);

        assert_eq!(
            split(&augmented),
            vec![
                PathBuf::from("/usr/bin"),
                PathBuf::from("/bin"),
                PathBuf::from("/opt/homebrew/bin"),
            ]
        );
    }

    #[test]
    fn every_fallback_reaches_the_real_augmented_path() {
        let augmented = augmented_path();
        let directories = split(&augmented);

        for extra in package_manager_dirs() {
            assert!(
                directories.contains(&PathBuf::from(extra)),
                "{extra} missing from {augmented:?}"
            );
        }
    }

    /// Mole asks `brew` whether an application is a cask. Under launchd's PATH
    /// it cannot, and answers `"source": "App"` for every one of them — which is
    /// why this is set on the command rather than left to the parent process.
    #[test]
    fn a_backend_command_carries_the_augmented_path() {
        let built = command("does-not-need-to-exist");
        let path = built
            .get_envs()
            .find(|(key, _)| *key == OsStr::new("PATH"))
            .and_then(|(_, value)| value)
            .expect("PATH is set on the command");

        let directories: Vec<PathBuf> = std::env::split_paths(path).collect();
        for extra in package_manager_dirs() {
            assert!(directories.contains(&PathBuf::from(extra)));
        }
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
        // PATHEXT is passed in uppercase, as Windows sets it, and the answer
        // must still match the file's own spelling rather than the variable's.
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
