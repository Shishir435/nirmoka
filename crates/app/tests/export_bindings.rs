//! Guards on the generated TypeScript.
//!
//! The generation itself is ts-rs's, driven by the `#[ts(export, export_to =
//! …)]` attributes in `crates/app/src/dto.rs` and run by `cargo test`:
//!
//! ```bash
//! cargo test -p nirmoka-app export_bindings
//! ```
//!
//! It writes one file, `packages/transport/src/generated/bindings.ts`, which is
//! committed so the frontend builds without a Rust toolchain. CI regenerates it
//! and fails on a diff, so a Rust type cannot move without the mirror moving
//! with it.
//!
//! What this file adds is the checks the generator cannot make for itself: that
//! the output landed where the frontend imports it from, and that byte counts
//! did not cross as a type the values never have.

use std::fs;
use std::path::PathBuf;

fn generated() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/transport/src/generated/bindings.ts");

    fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "no generated bindings at {}: {error}\n\
             run `cargo test -p nirmoka-app export_bindings` first — the export \
             attributes in dto.rs write this file",
            path.display()
        )
    })
}

/// The relative `export_to` path in `dto.rs` resolves from ts-rs's own base
/// directory, which is easy to get wrong by one level and produces a file
/// nobody imports rather than an error.
#[test]
fn the_bindings_land_where_the_frontend_imports_them_from() {
    let generated = generated();

    for expected in [
        "export type Backend",
        "export type ApplicationInventory",
        "export type Capabilities",
        "export type Detection",
        "export type DeveloperInventory",
        "export type NodeKind",
        "export type Row",
        "export type RowPage",
        "export type ScanFailure",
        "export type ScanProgress",
        "export type ScanSummary",
        "export type SystemStatus",
        "export type VolumeInfo",
    ] {
        assert!(
            generated.contains(expected),
            "{expected} is missing from the generated bindings"
        );
    }
}

/// Byte counts must cross as `number`.
///
/// ts-rs maps `u64` to `bigint`, which would describe a value that never
/// appears: Tauri's IPC is JSON, so these arrive as ordinary JavaScript numbers.
/// Without the `#[ts(type = "number")]` annotations the mismatch surfaces as
/// arithmetic that throws at runtime rather than as a type error.
#[test]
fn no_size_crosses_the_boundary_as_a_bigint() {
    assert!(
        !generated().contains("bigint"),
        "a u64 field in dto.rs is missing #[ts(type = \"number\")]"
    );
}
