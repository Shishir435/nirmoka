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
    /// Separate from [`Capabilities::uninstall_apps`] because the two are
    /// genuinely different claims: a backend can publish an inventory without
    /// being able to remove anything from it, and one that removes applications
    /// need not be the one that lists them. Keeping them apart is what let Mole
    /// offer a working inventory through the releases where its removal was not
    /// reachable at all.
    pub app_inventory: bool,

    /// Application removal including leftover files, reachable from a GUI.
    ///
    /// "Reachable from a GUI" rather than "non-interactive": the backend may own
    /// a confirmation prompt, and it may authenticate the user itself. What this
    /// flag promises is that the removal can be *previewed exactly* and then run
    /// to completion without a terminal — see ADR 0027 for what that requires of
    /// an implementor, which is considerably more than spawning a process.
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
