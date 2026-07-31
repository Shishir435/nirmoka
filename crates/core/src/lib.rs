//! Nirmoka core: the domain model shared by every frontend and every backend.
//!
//! # What belongs here
//!
//! The tree model, size arithmetic, sorting, filtering, and the policy that
//! decides what a user is allowed to do. Anything the GUI and the headless CLI
//! both need.
//!
//! # What does not
//!
//! - Anything from `tauri` or any other GUI framework.
//! - Anything that knows a specific backend exists (`ncdu`, `mo`, `gdu`).
//! - Platform conditionals. No `#[cfg(target_os = ...)]` in this crate.
//!
//! Those three rules are what make both sides of the architecture swappable.
//! `nirmoka-cli` exists to prove the first one holds: it links this crate with
//! no Tauri anywhere, so a violation becomes a build failure rather than a
//! documentation problem.

#![forbid(unsafe_code)]

pub mod error;
pub mod node;
pub mod size;
pub mod tree;

pub use error::{CoreError, Result};
pub use node::{Node, NodeKind};
pub use size::format_bytes;
pub use tree::{NodeId, Tree};
