//! Confirmation state and the durable, human-readable deletion journal.
//!
//! Raw paths never cross back into an execute command. A prepared adapter plan
//! is held here behind a one-time token, which makes explicit confirmation an
//! enforced backend property rather than a convention in a future button.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use nirmoka_adapter::{DeletePlan, DeleteReceipt};
use serde::{Deserialize, Serialize};

use crate::state::ScanId;

#[derive(Debug, Clone)]
pub struct PendingDelete {
    pub scan_id: ScanId,
    pub plan: DeletePlan,
    pub total_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct Operation {
    pub id: u64,
    pub receipt: DeleteReceipt,
    pub deleted_at_ms: u64,
    pub undone_at_ms: Option<u64>,
    pub log_error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "camelCase")]
enum LogEvent {
    Deleted {
        id: u64,
        backend: String,
        target: PathBuf,
        recovery_root: PathBuf,
        recovery_path: PathBuf,
        at_ms: u64,
    },
    Undone {
        id: u64,
        at_ms: u64,
    },
}

pub struct DeletionState {
    next_confirmation: u64,
    next_operation: u64,
    pending: HashMap<u64, PendingDelete>,
    operations: Vec<Operation>,
    log_path: Option<PathBuf>,
}

impl DeletionState {
    pub fn new(log_path: Option<PathBuf>) -> Self {
        let operations = log_path.as_deref().map(load_operations).unwrap_or_default();
        let next_operation = operations.iter().map(|op| op.id).max().unwrap_or(0);

        Self {
            next_confirmation: 0,
            next_operation,
            pending: HashMap::new(),
            operations,
            log_path,
        }
    }

    pub fn prepare(&mut self, pending: PendingDelete) -> u64 {
        self.next_confirmation = self.next_confirmation.saturating_add(1);
        let token = self.next_confirmation;
        // A newly prepared operation supersedes any dialog left open. This
        // bounds memory and prevents confirming an old selection after the user
        // has moved on to another row.
        self.pending.clear();
        self.pending.insert(token, pending);
        token
    }

    pub fn take_pending(&mut self, token: u64) -> Option<PendingDelete> {
        self.pending.remove(&token)
    }

    pub fn record_delete(&mut self, receipt: DeleteReceipt) -> Operation {
        self.next_operation = self.next_operation.saturating_add(1);
        let id = self.next_operation;
        let at_ms = now_ms();
        let event = LogEvent::Deleted {
            id,
            backend: receipt.backend().to_string(),
            target: receipt.target().to_path_buf(),
            recovery_root: receipt.recovery_root().to_path_buf(),
            recovery_path: receipt.recovery_path().to_path_buf(),
            at_ms,
        };
        let log_error = self.append(&event).err().map(|error| error.to_string());
        let operation = Operation {
            id,
            receipt,
            deleted_at_ms: at_ms,
            undone_at_ms: None,
            log_error,
        };
        self.operations.push(operation.clone());
        operation
    }

    pub fn operation(&self, id: u64) -> Option<Operation> {
        self.operations
            .iter()
            .find(|operation| operation.id == id)
            .cloned()
    }

    pub fn mark_undone(&mut self, id: u64) -> Result<Operation, String> {
        let at_ms = now_ms();
        let log_error = self
            .append(&LogEvent::Undone { id, at_ms })
            .err()
            .map(|error| error.to_string());
        let operation = self
            .operations
            .iter_mut()
            .find(|operation| operation.id == id)
            .ok_or_else(|| format!("unknown deletion operation {id}"))?;
        operation.undone_at_ms = Some(at_ms);
        operation.log_error = log_error;
        Ok(operation.clone())
    }

    pub fn operations(&self) -> Vec<Operation> {
        self.operations.iter().rev().cloned().collect()
    }

    fn append(&self, event: &LogEvent) -> io::Result<()> {
        let Some(path) = &self.log_path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        serde_json::to_writer(&mut file, event)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        file.write_all(b"\n")?;
        file.sync_data()
    }
}

fn load_operations(path: &Path) -> Vec<Operation> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };

    let mut operations = Vec::<Operation>::new();
    for event in text
        .lines()
        .filter_map(|line| serde_json::from_str::<LogEvent>(line).ok())
    {
        match event {
            LogEvent::Deleted {
                id,
                backend,
                target,
                recovery_root,
                recovery_path,
                at_ms,
            } => operations.push(Operation {
                id,
                receipt: DeleteReceipt::new(backend, target, recovery_root, recovery_path),
                deleted_at_ms: at_ms,
                undone_at_ms: None,
                log_error: None,
            }),
            LogEvent::Undone { id, at_ms } => {
                if let Some(operation) = operations.iter_mut().find(|operation| operation.id == id)
                {
                    operation.undone_at_ms = Some(at_ms);
                }
            }
        }
    }
    operations
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_tokens_are_one_time_and_superseded() {
        let mut state = DeletionState::new(None);
        let pending = |target: &str| PendingDelete {
            scan_id: 1,
            plan: DeletePlan::new(
                "rip",
                PathBuf::from("/scan"),
                PathBuf::from(target),
                nirmoka_adapter::DeleteMode::Trash,
            ),
            total_bytes: 1,
        };

        let old = state.prepare(pending("/scan/old"));
        let current = state.prepare(pending("/scan/current"));
        assert!(state.take_pending(old).is_none());
        assert!(state.take_pending(current).is_some());
        assert!(state.take_pending(current).is_none());
    }

    #[test]
    fn json_lines_survive_reload_and_corrupt_lines_are_ignored() {
        let path = std::env::temp_dir().join(format!(
            "nirmoka-operation-log-{}.jsonl",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);

        let receipt = DeleteReceipt::new(
            "rip",
            PathBuf::from("/scan/file"),
            PathBuf::from("/recovery/1"),
            PathBuf::from("/recovery/1/scan/file"),
        );
        let mut state = DeletionState::new(Some(path.clone()));
        let operation = state.record_delete(receipt);
        state.mark_undone(operation.id).unwrap();
        fs::write(
            &path,
            format!("{}not-json\n", fs::read_to_string(&path).unwrap()),
        )
        .unwrap();

        let loaded = DeletionState::new(Some(path.clone())).operations();
        assert_eq!(loaded.len(), 1);
        assert!(loaded[0].undone_at_ms.is_some());
        let _ = fs::remove_file(path);
    }
}
