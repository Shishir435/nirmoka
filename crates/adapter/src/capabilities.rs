//! What a backend can do.
//!
//! Backends differ enormously: ncdu scans, while Mole cleans by category,
//! uninstalls applications, and previews those operations with a dry run. The
//! UI queries these flags and hides what the active backend cannot do, rather
//! than offering a control that fails at call time.
//!
//! **New backend abilities are flags, never wire-format extensions.** Widening
//! the wire format requires its own ADR — see `docs/adr/0002`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    /// Walk a directory tree and report sizes.
    pub scan: bool,

    /// Non-interactively remove a caller-selected path.
    pub delete: bool,

    /// Recoverable mode for caller-selected path removal.
    pub trash: bool,

    /// Produce the exact list of what would be removed, without removing it.
    ///
    /// When false the UI falls back to an explicit confirmation dialog. An
    /// adapter must never fake a preview by guessing — see `docs/adapters.md`.
    pub dry_run: bool,

    /// Named cleanup targets (caches, logs, build artifacts) rather than only
    /// user-selected paths.
    pub cleanup_categories: bool,

    /// Application removal including leftover files.
    pub uninstall_apps: bool,

    /// System health metrics.
    pub system_status: bool,
}

impl Capabilities {
    /// The narrow scanner: scan and nothing else. ncdu's deletion belongs to
    /// its interactive TUI and is not a command an adapter can safely invoke.
    pub const MINIMAL: Self = Self {
        scan: true,
        delete: false,
        trash: false,
        dry_run: false,
        cleanup_categories: false,
        uninstall_apps: false,
        system_status: false,
    };
}

impl Default for Capabilities {
    fn default() -> Self {
        Self::MINIMAL
    }
}
