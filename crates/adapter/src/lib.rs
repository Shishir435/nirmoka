//! The backend adapter contract.
//!
//! An adapter teaches Nirmoka to drive one external disk tool. See
//! `docs/adapters.md` for the full contract and the reasoning behind it.
//!
//! # Current scope
//!
//! Detection, capabilities, and scanning. `delete` lands in step 10 with its
//! own validation and tests; it is deliberately absent rather than stubbed, so
//! no caller can depend on a signature that has not been designed yet.

#![forbid(unsafe_code)]

pub mod capabilities;
pub mod detect;
pub mod error;
pub mod preference;
pub mod process;
pub mod registry;
pub mod scan;
pub mod wire;

use std::path::Path;

pub use capabilities::Capabilities;
pub use detect::Detection;
pub use error::AdapterError;
pub use preference::{default_order, default_order_for, Ability, Preference};
pub use process::CancelToken;
pub use registry::{Choice, Registry};
pub use scan::{validate_scan_root, ScanOptions, ScanSummary};
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
}
