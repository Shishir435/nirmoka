/**
 * Types crossing the Rust/TypeScript boundary.
 *
 * They are GENERATED from the Rust definitions in `crates/app/src/dto.rs` by
 * ts-rs, into `./generated/bindings.ts`, and committed so the frontend builds
 * without a Rust toolchain. CI regenerates them and fails on a diff, so a Rust
 * type cannot move without its mirror moving too — which is the whole problem
 * worth solving here.
 *
 * This file stays as the import path so the generator owns exactly one file.
 * Anything hand-written that belongs beside these types goes here rather than
 * being edited into a file the next `cargo test` overwrites.
 *
 * @see docs/monorepo.md — "Types across the language boundary"
 */

export type * from "./generated/bindings.js";
