/**
 * Types crossing the Rust/TypeScript boundary.
 *
 * These are hand-written for step 0. From step 7 they are GENERATED from the
 * Rust definitions with `ts-rs` into `./generated/`, and this file re-exports
 * them. Hand-maintained mirrors of Rust structs drift silently, which is the
 * whole problem worth solving here.
 *
 * @see docs/monorepo.md — "Types across the language boundary"
 */

/** Mirrors `nirmoka_core::node::NodeKind`. */
export type NodeKind = "directory" | "file" | "symlink" | "other";

/** Mirrors `nirmoka_core::node::Node`. */
export interface Node {
  /** File or directory name, never a path. */
  name: string;
  kind: NodeKind;
  /** Bytes for this entry alone, excluding children. */
  ownBytes: number;
  /** Bytes for this entry plus its whole subtree. */
  totalBytes: number;
  /** Size is a lower bound — the backend could not fully read this entry. */
  readError: boolean;
}

/** A row ready to render. The frontend only ever receives the visible window. */
export interface Row extends Node {
  id: number;
  depth: number;
  /** Fraction of the parent's total, 0..1. For bar rendering. */
  share: number;
}

/** Mirrors `nirmoka_adapter::capabilities::Capabilities`. */
export interface Capabilities {
  scan: boolean;
  delete: boolean;
  trash: boolean;
  dryRun: boolean;
  cleanupCategories: boolean;
  uninstallApps: boolean;
  systemStatus: boolean;
}

/** Mirrors `nirmoka_adapter::detect::Detection`. */
export type Detection =
  | { state: "found"; path: string; version: string }
  | { state: "unsupportedVersion"; path: string; version: string; supported: string }
  | { state: "notInstalled" };

export interface Backend {
  id: string;
  displayName: string;
  supportedVersions: string;
  detection?: Detection;
  error?: string;
}

export interface ScanProgress {
  scanned: number;
  currentPath: string;
  done: boolean;
}
