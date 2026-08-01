//! Backend-produced inventory of applications that can be uninstalled.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledApplication {
    pub name: String,
    pub bundle_id: String,
    pub source: String,
    pub uninstall_name: String,
    pub path: PathBuf,
    pub size: u64,
}
