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
    /// The backend's own size label, verbatim — Mole 1.48.1 reports `"410.9MB"`,
    /// not a byte count.
    ///
    /// Kept as text because that is what was published. Parsing it back into
    /// bytes would invent six significant figures out of a rounded string and
    /// then let the UI add those inventions together.
    #[serde(rename = "size")]
    pub reported_size: String,
}
