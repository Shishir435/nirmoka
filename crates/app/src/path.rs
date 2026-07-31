//! Turning what a user typed into a path.
//!
//! `~/Downloads` is not a path. It is shell syntax that a shell expands before
//! any program sees it, which is why `nrmk scan ~/Downloads` works and typing
//! the same thing into a window does not: nothing between the text field and
//! `canonicalize` is a shell.
//!
//! The window is the only place in the project where a human types a path
//! rather than passing one, so the expansion belongs here rather than in the
//! adapters. An adapter receives a path; this is what makes one.

use std::path::PathBuf;

use directories::UserDirs;

/// Expand a leading `~` against the user's home directory.
///
/// Only a leading `~` alone or followed by a separator is expanded. `~alice` is
/// another user's home, which this cannot resolve and will not guess at — it is
/// passed through and fails as the literal directory name it is, which is what
/// it means to a program that is not a shell.
///
/// If the home directory cannot be determined, the input is returned unchanged
/// so that path validation reports the real problem rather than this one.
pub fn expand_home(input: &str) -> PathBuf {
    let Some(rest) = input.strip_prefix('~') else {
        return PathBuf::from(input);
    };

    // `~alice/…` — a user this process cannot look up. Left alone.
    if !rest.is_empty() && !starts_with_separator(rest) {
        return PathBuf::from(input);
    }

    let Some(home) = home_dir() else {
        return PathBuf::from(input);
    };

    // `push` on an absolute-looking `/Downloads` would replace the home
    // directory rather than extend it, so the separator is trimmed first.
    let relative = rest.trim_start_matches(std::path::is_separator);
    if relative.is_empty() {
        return home;
    }

    home.join(relative)
}

fn starts_with_separator(rest: &str) -> bool {
    rest.starts_with(std::path::is_separator)
}

/// The user's home directory, from the platform's own answer.
///
/// Never a literal: `/Users/<name>` is macOS-only and `~/Library` is a path
/// that exists on the machine it was written on.
fn home_dir() -> Option<PathBuf> {
    UserDirs::new().map(|dirs| dirs.home_dir().to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> PathBuf {
        home_dir().expect("this test machine has a home directory")
    }

    #[test]
    fn a_bare_tilde_is_the_home_directory() {
        assert_eq!(expand_home("~"), home());
    }

    #[test]
    fn a_tilde_path_is_joined_onto_the_home_directory() {
        assert_eq!(expand_home("~/Downloads"), home().join("Downloads"));
    }

    /// The bug this module exists for: the separator has to be dropped before
    /// the join, or `push` treats the rest as absolute and discards the home
    /// directory entirely.
    #[test]
    fn expansion_extends_the_home_directory_rather_than_replacing_it() {
        let expanded = expand_home("~/Downloads/nested");

        assert!(
            expanded.starts_with(home()),
            "{} escaped the home directory",
            expanded.display()
        );
        assert_eq!(expanded, home().join("Downloads").join("nested"));
    }

    #[test]
    fn another_users_home_is_not_guessed_at() {
        assert_eq!(
            expand_home("~alice/Downloads"),
            PathBuf::from("~alice/Downloads")
        );
    }

    #[test]
    fn a_tilde_anywhere_but_the_front_is_part_of_the_name() {
        assert_eq!(expand_home("/tmp/~backup"), PathBuf::from("/tmp/~backup"));
    }

    #[test]
    fn an_ordinary_path_is_left_exactly_as_it_was() {
        for input in ["/tmp", "relative/dir", ""] {
            assert_eq!(expand_home(input), PathBuf::from(input));
        }
    }
}
