//! Backend detection result.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Outcome of looking for a backend on this machine.
///
/// `UnsupportedVersion` is a distinct state on purpose. Silently treating an
/// unknown version as usable is how a format change becomes a data-loss bug;
/// the UI needs to be able to say "found ncdu 3.1, this build understands 2.x"
/// rather than failing mysteriously later.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Detection {
    Found {
        path: PathBuf,
        version: String,
    },
    UnsupportedVersion {
        path: PathBuf,
        version: String,
        supported: String,
    },
    NotInstalled,
}

impl Detection {
    pub fn is_usable(&self) -> bool {
        matches!(self, Detection::Found { .. })
    }

    pub fn version(&self) -> Option<&str> {
        match self {
            Detection::Found { version, .. } | Detection::UnsupportedVersion { version, .. } => {
                Some(version)
            }
            Detection::NotInstalled => None,
        }
    }
}
