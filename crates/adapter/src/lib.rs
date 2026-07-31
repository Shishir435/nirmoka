//! The backend adapter contract.
//!
//! An adapter teaches Nirmoka to drive one external disk tool. See
//! `docs/adapters.md` for the full contract and the reasoning behind it.
//!
//! # Current scope
//!
//! Step 2 of the roadmap defines detection and capabilities. `scan` and
//! `delete` land in steps 4 and 9 respectively, once the ncdu wire format
//! parser exists. They are deliberately absent rather than stubbed, so no
//! caller can depend on a signature that has not been designed yet.

#![forbid(unsafe_code)]

pub mod capabilities;
pub mod detect;
pub mod error;
pub mod registry;

pub use capabilities::Capabilities;
pub use detect::Detection;
pub use error::AdapterError;
pub use registry::Registry;

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
}
