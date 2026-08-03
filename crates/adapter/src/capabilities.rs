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

    /// Restore a durable receipt non-interactively. This may remain true when
    /// new recoverable deletion is withdrawn, so older receipts stay usable.
    pub undo: bool,

    /// Produce the exact list of what would be removed, without removing it.
    ///
    /// When false the UI falls back to an explicit confirmation dialog. An
    /// adapter must never fake a preview by guessing — see `docs/adapters.md`.
    pub dry_run: bool,

    /// Named cleanup targets (caches, logs, build artifacts) rather than only
    /// user-selected paths.
    pub cleanup_categories: bool,

    /// List installed applications with the identifier the backend's own
    /// uninstall command accepts.
    ///
    /// Separate from [`Capabilities::uninstall_apps`] because Mole 1.48.1 can do
    /// this and cannot do that: `mo uninstall --list` is a machine-readable
    /// one-shot, while every named uninstall stops at an interactive prompt.
    /// One flag for both would either hide the inventory or promise a removal
    /// that fails at the prompt — see ADR 0021.
    pub app_inventory: bool,

    /// Application removal including leftover files, driven non-interactively.
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
        undo: false,
        dry_run: false,
        cleanup_categories: false,
        app_inventory: false,
        uninstall_apps: false,
        system_status: false,
    };
}

impl Default for Capabilities {
    fn default() -> Self {
        Self::MINIMAL
    }
}
