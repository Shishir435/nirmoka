//! The types that cross into TypeScript.
//!
//! These are deliberately separate from the domain types in `nirmoka-core` and
//! `nirmoka-adapter`, even where they look identical today.
//!
//! Two reasons. First, invariant 1: `core` may depend on the standard library,
//! serde, and thiserror, so it cannot carry a `#[derive(TS)]`. Second, the shape
//! the UI needs and the shape the domain uses drift apart — a `Row` is a `Node`
//! plus its position in a window and its share of the parent, which is a fact
//! about a viewport, not about a file. Keeping the boundary explicit means that
//! drift shows up as a conversion, not as a domain type quietly growing a field
//! that only the frontend wanted.
//!
//! The TypeScript mirrors are generated from this file by `cargo test -p
//! nirmoka-app --test export_bindings` and committed to
//! `packages/transport/src/generated/`. CI regenerates and fails on a diff.

use nirmoka_adapter::registry::RegistryEntry;
use nirmoka_adapter::wire::TreeStats;
use nirmoka_adapter::{
    Capabilities as AdapterCapabilities, CleanupPreview as AdapterCleanupPreview,
    CleanupSystemScope as AdapterCleanupSystemScope, Detection as AdapterDetection,
    InstalledApplication as AdapterInstalledApplication, SystemStatus as AdapterSystemStatus,
};
use nirmoka_core::{Node, NodeKind as CoreNodeKind, Sort as CoreSort, Tree};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// Byte counts carry `#[ts(type = "number")]` throughout.
//
// ts-rs maps `u64` to `bigint`, which is right for a general Rust type and wrong
// here: Tauri's IPC is JSON, so these arrive as ordinary JavaScript numbers and
// a `bigint` annotation would describe a value that never appears. The precision
// ceiling is 2^53 bytes — 8 petabytes for a single entry — which is past the
// size of any disk this will run on.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub enum NodeKind {
    Directory,
    File,
    Symlink,
    Other,
}

impl From<CoreNodeKind> for NodeKind {
    fn from(kind: CoreNodeKind) -> Self {
        match kind {
            CoreNodeKind::Directory => Self::Directory,
            CoreNodeKind::File => Self::File,
            CoreNodeKind::Symlink => Self::Symlink,
            CoreNodeKind::Other => Self::Other,
        }
    }
}

/// How the frontend asked for a directory to be ordered.
///
/// This is the one DTO that travels inwards as well as out, so it derives
/// `Deserialize` too. Sorting stays in Rust because the frontend only ever holds
/// a window: sorting a few dozen rows out of a hundred thousand would reorder
/// the slice and call it a sort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub enum Sort {
    #[default]
    LargestFirst,
    SmallestFirst,
    NameAscending,
    NameDescending,
}

impl From<Sort> for CoreSort {
    fn from(sort: Sort) -> Self {
        match sort {
            Sort::LargestFirst => Self::LargestFirst,
            Sort::SmallestFirst => Self::SmallestFirst,
            Sort::NameAscending => Self::NameAscending,
            Sort::NameDescending => Self::NameDescending,
        }
    }
}

/// One step on the way back out of a directory.
///
/// The frontend holds a single node id, so without the chain there is no way to
/// name the directory it descended from — "up" would mean rescanning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct Crumb {
    pub id: u32,
    pub name: String,
}

/// Capacity of the filesystem containing a path. This is deliberately separate
/// from a scan summary: bytes reached by a scan are not disk capacity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct VolumeInfo {
    pub mount_point: String,
    #[ts(type = "number")]
    pub total_bytes: u64,
    #[ts(type = "number")]
    pub used_bytes: u64,
    #[ts(type = "number")]
    pub free_bytes: u64,
}

/// One backend-produced system-health snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct SystemStatus {
    pub backend: String,
    pub backend_instead_of: Option<String>,
    pub collected_at: String,
    pub host: String,
    pub platform: String,
    pub uptime: String,
    pub health_score: u8,
    pub health_score_message: String,
    pub hardware: HardwareStatus,
    pub cpu: CpuStatus,
    pub memory: MemoryStatus,
    pub disks: Vec<DiskStatus>,
    pub batteries: Vec<BatteryStatus>,
    pub thermal: ThermalStatus,
}

/// Exact path groups published by a backend dry run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct CleanupPreview {
    pub backend: String,
    pub backend_instead_of: Option<String>,
    pub generated_at: String,
    pub categories: Vec<CleanupCategory>,
    pub potential_cleanup: Option<String>,
    #[ts(type = "number")]
    pub total_items: u64,
    pub system_scope: CleanupSystemScope,
    pub warnings: Vec<String>,
}

/// Latest Rust-held cleanup review, bound to a short-lived one-time token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct CleanupPreparation {
    #[ts(type = "number")]
    pub confirmation_token: u64,
    pub backend: String,
    pub backend_instead_of: Option<String>,
    pub preview_generated_at: String,
    pub potential_cleanup: Option<String>,
    #[ts(type = "number")]
    pub total_items: u64,
    pub system_scope: CleanupSystemScope,
    pub warnings: Vec<String>,
    #[ts(type = "number")]
    pub expires_in_seconds: u64,
    pub requires_confirmation: bool,
    pub warning: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct CleanupCategory {
    pub name: String,
    pub items: Vec<CleanupItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct CleanupItem {
    pub path: String,
    pub reported_size: Option<String>,
    #[ts(type = "number")]
    pub item_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub enum CleanupSystemScope {
    Included,
    UserOnly,
    Unknown,
}

impl CleanupPreview {
    pub fn from_adapter(
        backend: impl Into<String>,
        backend_instead_of: Option<String>,
        preview: AdapterCleanupPreview,
    ) -> Self {
        Self {
            backend: backend.into(),
            backend_instead_of,
            generated_at: preview.generated_at,
            categories: preview
                .categories
                .into_iter()
                .map(|category| CleanupCategory {
                    name: category.name,
                    items: category
                        .items
                        .into_iter()
                        .map(|item| CleanupItem {
                            path: item.path.display().to_string(),
                            reported_size: item.reported_size,
                            item_count: item.item_count,
                        })
                        .collect(),
                })
                .collect(),
            potential_cleanup: preview.potential_cleanup,
            total_items: preview.total_items,
            system_scope: cleanup_system_scope(preview.system_scope),
            warnings: preview.warnings,
        }
    }
}

impl CleanupPreparation {
    pub fn from_state(preparation: crate::cleanup::CleanupPreparation) -> Self {
        Self {
            confirmation_token: preparation.token,
            backend: preparation.pending.backend,
            backend_instead_of: preparation.pending.backend_instead_of,
            preview_generated_at: preparation.pending.preview.generated_at,
            potential_cleanup: preparation.pending.preview.potential_cleanup,
            total_items: preparation.pending.preview.total_items,
            system_scope: cleanup_system_scope(preparation.pending.preview.system_scope),
            warnings: preparation.pending.preview.warnings,
            expires_in_seconds: preparation.expires_in.as_secs(),
            requires_confirmation: true,
            warning: "Mole will re-discover eligible candidates during execution. Results may differ from this preview."
                .to_string(),
        }
    }
}

fn cleanup_system_scope(scope: AdapterCleanupSystemScope) -> CleanupSystemScope {
    match scope {
        AdapterCleanupSystemScope::Included => CleanupSystemScope::Included,
        AdapterCleanupSystemScope::UserOnly => CleanupSystemScope::UserOnly,
        AdapterCleanupSystemScope::Unknown => CleanupSystemScope::Unknown,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct HardwareStatus {
    pub model: String,
    pub cpu_model: String,
    pub total_ram: String,
    pub disk_size: String,
    pub os_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct CpuStatus {
    pub usage: f64,
    pub load1: f64,
    pub load5: f64,
    pub load15: f64,
    pub core_count: u32,
    pub logical_cpu: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct MemoryStatus {
    #[ts(type = "number")]
    pub used: u64,
    #[ts(type = "number")]
    pub total: u64,
    #[ts(type = "number")]
    pub available: u64,
    pub used_percent: f64,
    #[ts(type = "number")]
    pub swap_used: u64,
    #[ts(type = "number")]
    pub swap_total: u64,
    pub pressure: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct DiskStatus {
    pub mount: String,
    pub device: String,
    #[ts(type = "number")]
    pub used: u64,
    #[ts(type = "number")]
    pub total: u64,
    pub used_percent: f64,
    pub filesystem: String,
    pub external: bool,
    pub smart_status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct BatteryStatus {
    pub percent: f64,
    pub status: String,
    pub time_left: String,
    pub health: String,
    pub cycle_count: u32,
    pub capacity: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct ThermalStatus {
    pub cpu_temp: Option<f64>,
    pub gpu_temp: Option<f64>,
    pub battery_temp: Option<f64>,
    pub fan_speed: Option<f64>,
    pub fan_count: Option<u32>,
}

impl SystemStatus {
    pub fn from_adapter(
        backend: impl Into<String>,
        backend_instead_of: Option<String>,
        status: AdapterSystemStatus,
    ) -> Self {
        Self {
            backend: backend.into(),
            backend_instead_of,
            collected_at: status.collected_at,
            host: status.host,
            platform: status.platform,
            uptime: status.uptime,
            health_score: status.health_score,
            health_score_message: status.health_score_msg,
            hardware: HardwareStatus {
                model: status.hardware.model,
                cpu_model: status.hardware.cpu_model,
                total_ram: status.hardware.total_ram,
                disk_size: status.hardware.disk_size,
                os_version: status.hardware.os_version,
            },
            cpu: CpuStatus {
                usage: status.cpu.usage,
                load1: status.cpu.load1,
                load5: status.cpu.load5,
                load15: status.cpu.load15,
                core_count: status.cpu.core_count,
                logical_cpu: status.cpu.logical_cpu,
            },
            memory: MemoryStatus {
                used: status.memory.used,
                total: status.memory.total,
                available: status.memory.available,
                used_percent: status.memory.used_percent,
                swap_used: status.memory.swap_used,
                swap_total: status.memory.swap_total,
                pressure: status.memory.pressure,
            },
            disks: status
                .disks
                .into_iter()
                .map(|disk| DiskStatus {
                    mount: disk.mount,
                    device: disk.device,
                    used: disk.used,
                    total: disk.total,
                    used_percent: disk.used_percent,
                    filesystem: disk.fstype,
                    external: disk.external,
                    smart_status: disk.smart_status,
                })
                .collect(),
            batteries: status
                .batteries
                .into_iter()
                .map(|battery| BatteryStatus {
                    percent: battery.percent,
                    status: battery.status,
                    time_left: battery.time_left,
                    health: battery.health,
                    cycle_count: battery.cycle_count,
                    capacity: battery.capacity,
                })
                .collect(),
            thermal: ThermalStatus {
                cpu_temp: status.thermal.cpu_temp,
                gpu_temp: status.thermal.gpu_temp,
                battery_temp: status.thermal.battery_temp,
                fan_speed: status.thermal.fan_speed,
                fan_count: status.thermal.fan_count,
            },
        }
    }
}

/// One application bundle found inside the completed scan tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct ApplicationItem {
    pub id: u32,
    pub name: String,
    pub path: String,
    #[ts(type = "number")]
    pub total_bytes: u64,
    pub size_is_partial: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct ApplicationInventory {
    #[ts(type = "number")]
    pub scan_id: u64,
    pub total: u32,
    pub rows: Vec<ApplicationItem>,
}

/// Applications addressed by a backend's own uninstall identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct InstalledApplicationInventory {
    pub backend: String,
    pub backend_instead_of: Option<String>,
    pub total: u32,
    pub rows: Vec<InstalledApplication>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct InstalledApplication {
    pub name: String,
    pub bundle_id: String,
    pub source: String,
    pub uninstall_name: String,
    pub path: String,
    #[ts(type = "number")]
    pub total_bytes: u64,
}

impl InstalledApplicationInventory {
    pub fn from_adapter(
        backend: impl Into<String>,
        backend_instead_of: Option<String>,
        applications: Vec<AdapterInstalledApplication>,
    ) -> Self {
        let total = applications.len().min(u32::MAX as usize) as u32;
        let mut rows = applications
            .into_iter()
            .map(|application| InstalledApplication {
                name: application.name,
                bundle_id: application.bundle_id,
                source: application.source,
                uninstall_name: application.uninstall_name,
                path: application.path.display().to_string(),
                total_bytes: application.size,
            })
            .collect::<Vec<_>>();
        rows.sort_unstable_by(|a, b| {
            b.total_bytes
                .cmp(&a.total_bytes)
                .then_with(|| a.path.cmp(&b.path))
        });
        Self {
            backend: backend.into(),
            backend_instead_of,
            total,
            rows,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub enum DeveloperCategory {
    XcodeDerivedData,
    SimulatorData,
    XcodeArchives,
    DeveloperCaches,
    GitRepository,
    NodeModules,
}

/// Developer data evidenced by names and locations in the completed scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct DeveloperItem {
    pub id: u32,
    pub category: DeveloperCategory,
    pub name: String,
    pub path: String,
    #[ts(type = "number")]
    pub total_bytes: u64,
    #[ts(type = "number | null")]
    pub modified_at_ms: Option<u64>,
    pub size_is_partial: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct DeveloperInventory {
    #[ts(type = "number")]
    pub scan_id: u64,
    pub total: u32,
    pub rows: Vec<DeveloperItem>,
}

/// Whether a backend is installed, and whether this build understands it.
///
/// `unsupportedVersion` stays a distinct state all the way to the UI. Collapsing
/// it into "not installed" would tell a user to install what they already have.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub enum Detection {
    #[serde(rename_all = "camelCase")]
    Found {
        path: String,
        version: String,
    },
    #[serde(rename_all = "camelCase")]
    UnsupportedVersion {
        path: String,
        version: String,
        supported: String,
    },
    NotInstalled,
}

impl From<&AdapterDetection> for Detection {
    fn from(detection: &AdapterDetection) -> Self {
        match detection {
            AdapterDetection::Found { path, version } => Self::Found {
                path: path.display().to_string(),
                version: version.clone(),
            },
            AdapterDetection::UnsupportedVersion {
                path,
                version,
                supported,
            } => Self::UnsupportedVersion {
                path: path.display().to_string(),
                version: version.clone(),
                supported: supported.clone(),
            },
            AdapterDetection::NotInstalled => Self::NotInstalled,
        }
    }
}

/// One backend as the picker sees it.
///
/// `detection` and `error` are both optional because detection itself can fail —
/// a backend that exists but whose version output could not be read is neither
/// "found" nor "not installed", and saying so is more useful than picking one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct Backend {
    pub id: String,
    pub display_name: String,
    pub supported_versions: String,
    pub detection: Option<Detection>,
    pub error: Option<String>,
    /// Usable right now: installed, and at a version this build was tested
    /// against. Necessary for the scan button, and no longer sufficient: Mole
    /// is usable on macOS and cannot scan.
    pub usable: bool,
    /// What this backend can do. Per backend rather than for the app as a
    /// whole, because the two registered backends do not overlap — the UI needs
    /// to know which one it is asking.
    pub capabilities: Capabilities,
}

impl From<&RegistryEntry> for Backend {
    fn from(entry: &RegistryEntry) -> Self {
        let (detection, error) = match &entry.detection {
            Ok(detection) => (Some(Detection::from(detection)), None),
            Err(error) => (None, Some(error.to_string())),
        };

        Self {
            id: entry.id.to_string(),
            display_name: entry.display_name.to_string(),
            supported_versions: entry.supported_versions.to_string(),
            usable: matches!(&detection, Some(Detection::Found { .. })),
            capabilities: Capabilities::from(entry.capabilities),
            detection,
            error,
        }
    }
}

/// Which backend the user picked, and which one will actually run a scan.
///
/// Two fields rather than one because they are genuinely different facts, and
/// the gap between them is the thing the picker has to explain. Choosing Mole on
/// macOS is honoured everywhere Mole can do the job — and ncdu still scans,
/// because Mole cannot. A UI showing only `chosen` would claim the scan came
/// from a backend that never ran.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct BackendSelection {
    /// The backend id the user picked, or `null` for the platform default.
    ///
    /// `null` is a real setting rather than an absent one: it keeps following
    /// the default when a later release changes it, which a value written on
    /// first run would not.
    pub chosen: Option<String>,

    /// Backend ids in this platform's default order, best first. Includes
    /// backends that are not installed and ones with no adapter yet, so the UI
    /// can show where a choice sits without guessing the ordering.
    pub default_order: Vec<String>,

    /// The backend a scan will run on, or `null` if nothing installed can scan.
    pub scanner: Option<String>,

    /// Set when `scanner` is not the backend that was chosen. Naming who was
    /// asked for is what stops a fallback from reading as the setting being
    /// ignored.
    pub scanner_instead_of: Option<String>,

    /// Whether a change outlives the process. False on a machine with no
    /// configuration directory, where the choice is honoured for the session
    /// and then forgotten — which the UI says rather than letting it surprise.
    pub persistent: bool,

    /// Why the last change could not be written down, if it could not be.
    ///
    /// A field rather than a failed call. The choice *did* take effect — it is
    /// applied in memory before the write is attempted — so returning an error
    /// would leave the picker rendering the previous backend while the process
    /// runs on the new one. Two different things happened and both have to
    /// arrive: the selection, and the fact that it will not survive a restart.
    pub save_error: Option<String>,
}

/// What the active backend can do, so the UI can hide controls it cannot honour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct Capabilities {
    pub scan: bool,
    pub delete: bool,
    pub trash: bool,
    pub undo: bool,
    pub dry_run: bool,
    pub cleanup_categories: bool,
    pub uninstall_apps: bool,
    pub system_status: bool,
}

/// Stable failure classes for destructive commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub enum DeleteFailureCode {
    NoCompletedScan,
    StaleScan,
    UnknownNode,
    NoBackend,
    ConfirmationExpired,
    AlreadyUndone,
    Backend,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct DeleteFailure {
    pub code: DeleteFailureCode,
    pub message: String,
}

impl DeleteFailure {
    pub fn new(code: DeleteFailureCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub enum DeleteDisposition {
    Trash,
    Permanent,
}

/// A validated destructive operation waiting for explicit confirmation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct DeletePreparation {
    #[ts(type = "number")]
    pub confirmation_token: u64,
    pub backend: String,
    pub backend_instead_of: Option<String>,
    pub target_path: String,
    #[ts(type = "number")]
    pub total_bytes: u64,
    pub disposition: DeleteDisposition,
    pub recoverable: bool,
    pub dry_run: bool,
    pub requires_confirmation: bool,
    pub warning: String,
}

/// One durable deletion journal entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct DeleteOperation {
    #[ts(type = "number")]
    pub id: u64,
    pub backend: String,
    pub target_path: String,
    pub disposition: DeleteDisposition,
    pub recoverable: bool,
    pub undone: bool,
    #[ts(type = "number")]
    pub deleted_at_ms: u64,
    #[ts(type = "number | null")]
    pub undone_at_ms: Option<u64>,
    pub log_error: Option<String>,
}

impl DeleteOperation {
    pub fn from_operation(operation: &crate::deletion::Operation) -> Self {
        Self {
            id: operation.id,
            backend: operation.receipt.backend().to_string(),
            target_path: operation.receipt.target().display().to_string(),
            disposition: DeleteDisposition::Trash,
            recoverable: true,
            undone: operation.undone_at_ms.is_some(),
            deleted_at_ms: operation.deleted_at_ms,
            undone_at_ms: operation.undone_at_ms,
            log_error: operation.log_error.clone(),
        }
    }
}

impl From<AdapterCapabilities> for Capabilities {
    fn from(caps: AdapterCapabilities) -> Self {
        Self {
            scan: caps.scan,
            delete: caps.delete,
            trash: caps.trash,
            undo: caps.undo,
            dry_run: caps.dry_run,
            cleanup_categories: caps.cleanup_categories,
            uninstall_apps: caps.uninstall_apps,
            system_status: caps.system_status,
        }
    }
}

/// One rendered line. The frontend never receives anything else about the tree.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct Row {
    /// Opaque handle into the Rust-side tree. Pass it back to descend.
    pub id: u32,
    pub name: String,
    pub kind: NodeKind,
    /// Disk usage for this entry alone.
    #[ts(type = "number")]
    pub own_bytes: u64,
    /// What the entry claims to be. Differs from `ownBytes` for sparse files.
    #[ts(type = "number")]
    pub apparent_bytes: u64,
    /// Disk usage for this entry plus everything under it.
    #[ts(type = "number")]
    pub total_bytes: u64,
    /// The size is a lower bound: the backend could not read this entry.
    pub read_error: bool,
    /// Counted once already, under another name. Not empty — shared.
    pub hardlink: bool,
    /// Skipped by request, so its size is unknown rather than zero.
    pub excluded: bool,
    /// Directories with children can be descended into.
    pub child_count: u32,
    /// Fraction of the parent's total, 0..1, for bar rendering.
    pub share: f64,
}

impl Row {
    fn from_node(id: u32, node: &Node, child_count: u32, parent_total: u64) -> Self {
        Self {
            id,
            name: node.name.clone(),
            kind: node.kind.into(),
            own_bytes: node.own_bytes,
            apparent_bytes: node.apparent_bytes,
            total_bytes: node.total_bytes,
            read_error: node.read_error,
            hardlink: node.hardlink,
            excluded: node.excluded,
            child_count,
            // A zero-byte parent makes every share meaningless rather than
            // infinite; report nothing rather than dividing by zero.
            share: if parent_total == 0 {
                0.0
            } else {
                node.total_bytes as f64 / parent_total as f64
            },
        }
    }
}

/// One window of one directory's children.
///
/// `total` is how many children exist, not how many are in `rows` — the caller
/// needs it to size a scrollbar without asking for the whole directory, which is
/// invariant 5 in one field.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct RowPage {
    /// Which scan these rows came from. Pass it back with any id taken from
    /// them; a page from a scan that has since been replaced is refused rather
    /// than answered from a tree that reused the same numbers.
    #[ts(type = "number")]
    pub scan_id: u64,
    pub parent_id: u32,
    /// The parent's own name, which is what the breadcrumb's last segment says.
    /// Splitting it out of `path` here avoids the frontend guessing at a
    /// separator that differs by platform.
    pub name: String,
    /// Absolute path of the parent, for the header.
    pub path: String,
    /// Root first, the parent itself excluded. Every entry is somewhere the user
    /// can click back to.
    pub ancestors: Vec<Crumb>,
    /// The parent could not be read, so an empty page means "not allowed to
    /// look" rather than "nothing here". The distinction is the difference
    /// between a bug report and a padlock icon.
    pub read_error: bool,
    /// The order these rows are in, echoed back. The frontend can render the
    /// controls from the page it is showing instead of from what it last asked
    /// for, which are different things while a request is in flight.
    pub sort: Sort,
    pub offset: u32,
    pub total: u32,
    pub rows: Vec<Row>,
}

/// Progress while a scan is running.
///
/// Emitted periodically rather than per entry: a home directory produces
/// millions of entries and an event per entry would spend more time in IPC than
/// in the scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct ScanProgress {
    #[ts(type = "number")]
    pub scanned: u64,
    /// Where the backend is now, for something truthful to put on screen.
    pub current_path: String,
}

/// A finished scan. Everything here is a fact about a completed walk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct ScanSummary {
    /// Identifies this scan for the lifetime of the process. Node ids are only
    /// meaningful together with it.
    #[ts(type = "number")]
    pub scan_id: u64,
    pub root_id: u32,
    pub root_path: String,
    #[ts(type = "number")]
    pub total_bytes: u64,
    #[ts(type = "number")]
    pub entries: u64,
    #[ts(type = "number")]
    pub directories: u64,
    pub backend_id: String,
    pub backend_version: Option<String>,
    /// Counts of what the totals could not account for. A total that quietly
    /// omits twelve unreadable directories is a lie by omission.
    #[ts(type = "number")]
    pub read_errors: u64,
    #[ts(type = "number")]
    pub excluded: u64,
    #[ts(type = "number")]
    pub hardlinks_deduplicated: u64,
    #[ts(type = "number")]
    pub hardlink_bytes_saved: u64,
}

impl ScanSummary {
    pub(crate) fn new(
        scan_id: u64,
        tree: &Tree,
        summary: &nirmoka_adapter::ScanSummary,
        stats: TreeStats,
        backend_id: &str,
    ) -> Self {
        let root = tree.root();
        let total_bytes = root
            .and_then(|id| tree.get(id).ok())
            .map(|node| node.total_bytes)
            .unwrap_or(0);

        Self {
            scan_id,
            root_id: root.map(|id| id.raw()).unwrap_or(0),
            root_path: summary.root.display().to_string(),
            total_bytes,
            entries: summary.items,
            directories: summary.directories,
            backend_id: backend_id.to_string(),
            backend_version: summary.backend_version.clone(),
            read_errors: stats.read_errors,
            excluded: stats.excluded,
            hardlinks_deduplicated: stats.hardlinks_deduplicated,
            hardlink_bytes_saved: stats.hardlink_bytes_saved,
        }
    }
}

/// A scan that ended without a result.
///
/// `cancelled` is separate from the message because a cancelled scan is not an
/// error the user needs to read — they are the one who stopped it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../../packages/transport/src/generated/bindings.ts"
)]
pub struct ScanFailure {
    pub message: String,
    pub cancelled: bool,
}

/// Build a page of rows from one directory's children.
pub(crate) fn page(
    scan_id: u64,
    tree: &Tree,
    parent: nirmoka_core::NodeId,
    sort: Sort,
    offset: u32,
    limit: u32,
) -> RowPage {
    let node = tree.get(parent).ok();
    let parent_total = node.map(|node| node.total_bytes).unwrap_or(0);
    let children = tree.children_sorted(parent, sort.into());

    let rows = children
        .iter()
        .skip(offset as usize)
        .take(limit as usize)
        .filter_map(|&id| {
            let node = tree.get(id).ok()?;
            Some(Row::from_node(
                id.raw(),
                node,
                tree.children_of(id).len() as u32,
                parent_total,
            ))
        })
        .collect();

    let ancestors = tree
        .ancestors_of(parent)
        .into_iter()
        .filter_map(|id| {
            Some(Crumb {
                id: id.raw(),
                name: tree.get(id).ok()?.name.clone(),
            })
        })
        .collect();

    RowPage {
        scan_id,
        parent_id: parent.raw(),
        name: node.map(|node| node.name.clone()).unwrap_or_default(),
        path: tree
            .path_of(parent)
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        ancestors,
        read_error: node.is_some_and(|node| node.read_error),
        sort,
        offset,
        total: children.len() as u32,
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nirmoka_core::Node;

    fn tree_with_two_children() -> Tree {
        let mut tree = Tree::new("/fixtures/root");
        let root = tree.push(None, Node::directory("root"));
        tree.push(Some(root), Node::file("big", 300));
        tree.push(Some(root), Node::file("small", 100));
        tree.rollup();
        tree
    }

    #[test]
    fn a_row_reports_its_share_of_the_parent() {
        let tree = tree_with_two_children();
        let page = page(1, &tree, tree.root().unwrap(), Sort::LargestFirst, 0, 10);

        assert_eq!(page.rows.len(), 2);
        assert_eq!(page.rows[0].name, "big");
        assert!((page.rows[0].share - 0.75).abs() < f64::EPSILON);
        assert!((page.rows[1].share - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn an_empty_parent_reports_no_share_rather_than_dividing_by_zero() {
        let mut tree = Tree::new("/fixtures/root");
        let root = tree.push(None, Node::directory("root"));
        tree.push(Some(root), Node::file("empty", 0));
        tree.rollup();

        let page = page(1, &tree, root, Sort::LargestFirst, 0, 10);
        assert_eq!(page.rows[0].share, 0.0);
    }

    #[test]
    fn a_page_is_ordered_the_way_it_was_asked_for_and_says_which_way_that_was() {
        let tree = tree_with_two_children();
        let root = tree.root().unwrap();

        let smallest = page(1, &tree, root, Sort::SmallestFirst, 0, 10);
        assert_eq!(smallest.rows[0].name, "small");
        assert_eq!(smallest.sort, Sort::SmallestFirst);

        let by_name = page(1, &tree, root, Sort::NameDescending, 0, 10);
        assert_eq!(by_name.rows[0].name, "small");
        assert_eq!(by_name.sort, Sort::NameDescending);
    }

    #[test]
    fn a_page_carries_the_way_back_out() {
        let mut tree = Tree::new("/fixtures/root");
        let root = tree.push(None, Node::directory("root"));
        let nested = tree.push(Some(root), Node::directory("nested"));
        tree.push(Some(nested), Node::file("leaf", 10));
        tree.rollup();

        let page = page(1, &tree, nested, Sort::LargestFirst, 0, 10);

        assert_eq!(page.name, "nested");
        assert_eq!(page.ancestors.len(), 1);
        assert_eq!(page.ancestors[0].name, "root");
        assert_eq!(page.ancestors[0].id, root.raw());

        let at_root = page_of_root(&tree);
        assert!(
            at_root.ancestors.is_empty(),
            "the scan root has nowhere further out to go"
        );
    }

    fn page_of_root(tree: &Tree) -> RowPage {
        page(1, tree, tree.root().unwrap(), Sort::LargestFirst, 0, 10)
    }

    /// An unreadable directory looks exactly like an empty one from the row
    /// count alone, and the two want different words on screen.
    #[test]
    fn a_directory_that_could_not_be_read_says_so_rather_than_looking_empty() {
        let mut tree = Tree::new("/fixtures/root");
        let root = tree.push(None, Node::directory("root"));
        let mut denied = Node::directory("denied");
        denied.read_error = true;
        let denied = tree.push(Some(root), denied);
        tree.rollup();

        let page = page(1, &tree, denied, Sort::LargestFirst, 0, 10);
        assert!(page.rows.is_empty());
        assert!(page.read_error);
        assert!(!page_of_root(&tree).read_error);
    }

    #[test]
    fn a_window_reports_the_full_child_count_not_the_window_size() {
        let tree = tree_with_two_children();
        let page = page(1, &tree, tree.root().unwrap(), Sort::LargestFirst, 1, 1);

        assert_eq!(page.rows.len(), 1, "asked for one row");
        assert_eq!(page.rows[0].name, "small", "offset skipped the largest");
        assert_eq!(page.total, 2, "but the scrollbar needs both");
    }

    #[test]
    fn an_offset_past_the_end_is_an_empty_page_not_an_error() {
        let tree = tree_with_two_children();
        let page = page(1, &tree, tree.root().unwrap(), Sort::LargestFirst, 99, 10);

        assert!(page.rows.is_empty());
        assert_eq!(page.total, 2);
    }

    /// The one DTO the frontend sends *in*, so the spelling is a contract rather
    /// than a display detail: a `Sort` Rust cannot deserialize is a `rows` call
    /// that fails with a deserialization error instead of returning a page.
    #[test]
    fn a_sort_arrives_spelled_the_way_the_bindings_say_it_is() {
        for (variant, wire) in [
            (Sort::LargestFirst, "\"largestFirst\""),
            (Sort::SmallestFirst, "\"smallestFirst\""),
            (Sort::NameAscending, "\"nameAscending\""),
            (Sort::NameDescending, "\"nameDescending\""),
        ] {
            assert_eq!(serde_json::to_string(&variant).unwrap(), wire);
            assert_eq!(serde_json::from_str::<Sort>(wire).unwrap(), variant);
        }
    }

    #[test]
    fn an_unsupported_version_survives_the_conversion_as_itself() {
        let detection = AdapterDetection::UnsupportedVersion {
            path: "/usr/bin/ncdu".into(),
            version: "1.19".into(),
            supported: ">=2.0, <3.0".into(),
        };

        match Detection::from(&detection) {
            Detection::UnsupportedVersion {
                path,
                version,
                supported,
            } => {
                assert_eq!(path, "/usr/bin/ncdu");
                assert_eq!(version, "1.19");
                assert_eq!(supported, ">=2.0, <3.0");
            }
            other => panic!("unsupported version collapsed into {other:?}"),
        }
    }

    #[test]
    fn backend_application_identity_survives_and_rows_sort_by_size() {
        let inventory = InstalledApplicationInventory::from_adapter(
            "mole",
            None,
            vec![
                AdapterInstalledApplication {
                    name: "Small".into(),
                    bundle_id: "example.small".into(),
                    source: "user".into(),
                    uninstall_name: "Small Command".into(),
                    path: "/Applications/Small.app".into(),
                    size: 10,
                },
                AdapterInstalledApplication {
                    name: "Large".into(),
                    bundle_id: "example.large".into(),
                    source: "system".into(),
                    uninstall_name: "Large Command".into(),
                    path: "/Applications/Large.app".into(),
                    size: 20,
                },
            ],
        );

        assert_eq!(inventory.rows[0].name, "Large");
        assert_eq!(inventory.rows[1].uninstall_name, "Small Command");
    }
}
