//! Backend-neutral system health snapshot.
//!
//! Mole currently supplies this data, but callers ask for the capability rather
//! than the backend. Keeping the shape here lets another adapter satisfy the
//! same UI without importing Mole into the shell.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemStatus {
    pub collected_at: String,
    pub host: String,
    pub platform: String,
    pub uptime: String,
    pub health_score: u8,
    pub health_score_msg: String,
    pub hardware: HardwareStatus,
    pub cpu: CpuStatus,
    pub memory: MemoryStatus,
    pub disks: Vec<DiskStatus>,
    #[serde(default)]
    pub batteries: Vec<BatteryStatus>,
    #[serde(default)]
    pub thermal: ThermalStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HardwareStatus {
    pub model: String,
    pub cpu_model: String,
    pub total_ram: String,
    pub disk_size: String,
    pub os_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CpuStatus {
    pub usage: f64,
    pub load1: f64,
    pub load5: f64,
    pub load15: f64,
    pub core_count: u32,
    pub logical_cpu: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryStatus {
    pub used: u64,
    pub total: u64,
    pub available: u64,
    pub used_percent: f64,
    pub swap_used: u64,
    pub swap_total: u64,
    pub pressure: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiskStatus {
    pub mount: String,
    pub device: String,
    pub used: u64,
    pub total: u64,
    pub used_percent: f64,
    pub fstype: String,
    pub external: bool,
    pub smart_status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatteryStatus {
    pub percent: f64,
    pub status: String,
    pub time_left: String,
    pub health: String,
    pub cycle_count: u32,
    pub capacity: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ThermalStatus {
    #[serde(default)]
    pub cpu_temp: Option<f64>,
    #[serde(default)]
    pub gpu_temp: Option<f64>,
    #[serde(default)]
    pub battery_temp: Option<f64>,
    #[serde(default)]
    pub fan_speed: Option<f64>,
    #[serde(default)]
    pub fan_count: Option<u32>,
}
