//! The backend adapter contract.
//!
//! An adapter teaches Nirmoka to drive one external disk tool. See
//! `docs/adapters.md` for the full contract and the reasoning behind it.
//!
//! # Current scope
//!
//! Detection, capabilities, scanning, and the shared validation required before
//! a delete target may reach a backend. Path deletion itself is deliberately
//! absent until a backend exposes a non-interactive command for it.

#![forbid(unsafe_code)]

pub mod applications;
pub mod capabilities;
pub mod cleanup;
pub mod delete;
pub mod detect;
pub mod error;
pub mod preference;
pub mod process;
pub mod registry;
pub mod scan;
pub mod status;
pub mod wire;

use std::path::Path;

pub use applications::InstalledApplication;
pub use capabilities::Capabilities;
pub use cleanup::{CleanupCategory, CleanupItem, CleanupPreview, CleanupSystemScope};
pub use delete::{validate_delete_target, DeleteMode, DeletePlan, DeleteReceipt};
pub use detect::Detection;
pub use error::AdapterError;
pub use preference::{default_order, default_order_for, Ability, Preference};
pub use process::CancelToken;
pub use registry::{Choice, Registry};
pub use scan::{validate_scan_root, ScanOptions, ScanSummary};
pub use status::SystemStatus;
pub use wire::{TreeSink, WireError, WireItem, WireSink};

/// One external disk tool, wrapped.
///
/// Implementors own binary detection, version gating, and path validation.
/// They do not own rendering, sorting, selection, or confirmation — those live
/// in `nirmoka-core` and the UI, once, for every backend.
pub trait Adapter: Send + Sync {
    /// Stable machine identifier: `"ncdu"`, `"mole"`, `"gdu"`. Used in config
    /// and logs, so it must not change once released.
    fn id(&self) -> &'static str;

    /// Name shown in the backend picker.
    fn display_name(&self) -> &'static str;

    /// Version range this adapter has been tested against, in a form fit for
    /// display in an error message.
    fn supported_versions(&self) -> &'static str;

    /// Is the backend installed, and is its version one we understand?
    ///
    /// Must never return `Found` for a version outside
    /// [`Adapter::supported_versions`]. Output formats drift silently, and a
    /// changed field on a delete path is the worst place to find out.
    fn detect(&self) -> Result<Detection, AdapterError>;

    /// What this backend can do. Only meaningful after a successful
    /// [`Adapter::detect`].
    fn capabilities(&self) -> Capabilities;

    /// Read one system-health snapshot from this backend.
    fn system_status(&self, _cancel: &CancelToken) -> Result<SystemStatus, AdapterError> {
        Err(AdapterError::Unsupported {
            backend: self.id(),
            operation: "system status",
        })
    }

    /// List applications this backend can later address for uninstall.
    fn installed_applications(
        &self,
        _cancel: &CancelToken,
    ) -> Result<Vec<InstalledApplication>, AdapterError> {
        Err(AdapterError::Unsupported {
            backend: self.id(),
            operation: "application inventory",
        })
    }

    /// Discover the backend's cleanup candidates without removing anything.
    fn cleanup_preview(&self, _cancel: &CancelToken) -> Result<CleanupPreview, AdapterError> {
        Err(AdapterError::Unsupported {
            backend: self.id(),
            operation: "cleanup preview",
        })
    }

    /// Run one backend-owned cleanup after the shell has consumed an explicit
    /// confirmation token.
    ///
    /// A cleanup preview is evidence, not an execution plan. Backends such as
    /// Mole do not accept previewed paths or categories as command arguments;
    /// they perform fresh discovery when this method runs. Implementors must
    /// never turn preview rows into delete arguments.
    fn execute_cleanup(&self, _cancel: &CancelToken) -> Result<(), AdapterError> {
        Err(AdapterError::Unsupported {
            backend: self.id(),
            operation: "cleanup execution",
        })
    }

    /// Walk `root`, streaming entries into `sink` as the backend produces them.
    ///
    /// # Requirements on implementors
    ///
    /// - Validate `root` before it becomes a subprocess argument.
    /// - Emit the wire format, whatever the backend natively speaks.
    /// - Stream. Collecting the backend's whole output before calling the sink
    ///   makes the app feel broken on exactly the disks people need it for.
    /// - Honour `cancel` by **killing the subprocess**, then return
    ///   [`AdapterError::Cancelled`] rather than a truncated success.
    fn scan(
        &self,
        root: &Path,
        options: &ScanOptions,
        sink: &mut dyn WireSink,
        cancel: &CancelToken,
    ) -> Result<ScanSummary, AdapterError>;

    /// Validate and canonicalise one selected-path deletion.
    ///
    /// This method is deliberately separate from [`Adapter::delete`]: the
    /// shell turns the returned plan into a one-time confirmation token rather
    /// than handing a raw path back to the frontend.
    fn prepare_delete(
        &self,
        _scan_root: &Path,
        _target: &Path,
        _mode: DeleteMode,
    ) -> Result<DeletePlan, AdapterError> {
        Err(AdapterError::Unsupported {
            backend: self.id(),
            operation: "selected-path deletion",
        })
    }

    /// Execute a previously prepared deletion.
    ///
    /// Implementors must validate the plan again immediately before the path
    /// becomes a subprocess argument. Confirmation belongs to the shell and is
    /// enforced before this method is reachable.
    fn delete(
        &self,
        _plan: &DeletePlan,
        _cancel: &CancelToken,
    ) -> Result<DeleteReceipt, AdapterError> {
        Err(AdapterError::Unsupported {
            backend: self.id(),
            operation: "selected-path deletion",
        })
    }

    /// Restore one receipt produced by this adapter.
    fn undo(&self, _receipt: &DeleteReceipt, _cancel: &CancelToken) -> Result<(), AdapterError> {
        Err(AdapterError::Unsupported {
            backend: self.id(),
            operation: "undo deletion",
        })
    }
}
