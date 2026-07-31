//! What a backend can do.
//!
//! Backends differ enormously: ncdu browses and deletes, Mole additionally
//! cleans by category, uninstalls applications, routes to Trash, and previews
//! with a dry run. The UI queries these flags and hides what the active backend
//! cannot do, rather than offering a control that fails at call time.
//!
//! **New backend abilities are flags, never wire-format extensions.** Widening
//! the wire format requires its own ADR — see `docs/adr/0002`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    /// Walk a directory tree and report sizes. Every backend has this.
    pub scan: bool,

    /// Remove a path. Every backend worth adapting has this.
    pub delete: bool,

    /// Recoverable removal rather than permanent.
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
    /// The floor: scan and delete, nothing else. A useful starting point for a
    /// new adapter, and what ncdu actually provides.
    pub const MINIMAL: Self = Self {
        scan: true,
        delete: true,
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
