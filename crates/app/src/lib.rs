//! Application service for the native macOS shell.
//! Scanning, adapters, and confirmation state remain in Rust.

#![forbid(unsafe_code)]

pub mod attribution;
pub mod categories;
pub mod cleanup;
pub mod commands;
pub mod deletion;
pub mod dto;
pub mod icons;
pub mod inventory;
pub mod native;
pub mod optimization;
pub mod path;
pub mod reveal;
pub mod scan;
pub mod settings;
pub mod state;
pub mod trash;
pub mod uninstall;
pub mod volume;
