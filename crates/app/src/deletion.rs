//! Confirmation state and the durable, human-readable operation journal.
//!
//! Raw paths never cross back into an execute command. A prepared adapter plan
//! is held here behind a one-time token, which makes explicit confirmation an
//! enforced backend property rather than a convention in a future button.
//!
//! One journal file holds every destructive operation — selected-path deletions
//! and backend cleanup runs — because they share an id space and a reader. Two
//! files with two writers would eventually disagree about what happened first.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use nirmoka_adapter::{
    CleanupCompletion, CleanupExecution, CleanupSystemScope, DeletePlan, DeleteReceipt,
};
use serde::{Deserialize, Serialize};

use crate::state::ScanId;

#[derive(Debug, Clone)]
pub struct PendingDelete {
    pub scan_id: ScanId,
    pub plan: DeletePlan,
    pub total_bytes: u64,
}

/// A validated move to the platform Trash, waiting for its confirmation.
///
/// The resolved target is kept so the dialog names the path that was checked
/// rather than the one that was clicked, and the scan root is kept because the
/// validator runs again before the move and needs both.
#[derive(Debug, Clone)]
pub struct PendingTrash {
    pub scan_id: ScanId,
    pub scan_root: PathBuf,
    pub target: PathBuf,
    pub total_bytes: u64,
}

/// The one destructive operation currently awaiting confirmation.
///
/// One slot, not one per kind. Two prepared operations would mean two open
/// dialogs, and the second confirmation would execute whichever the map
/// happened to still hold.
#[derive(Debug, Clone)]
pub enum Pending {
    Delete(PendingDelete),
    Trash(PendingTrash),
}

#[derive(Debug, Clone)]
pub struct Operation {
    pub id: u64,
    pub receipt: DeleteReceipt,
    pub deleted_at_ms: u64,
    pub undone_at_ms: Option<u64>,
    pub log_error: Option<String>,
}

/// One item moved to the platform Trash.
///
/// There is no receipt and no undo here, for the reason ADR 0025 gives: macOS
/// exposes no supported way to name where the item landed, and a receipt that
/// does not resolve is worse than none. Recovery is Put Back, in the Finder. The
/// original path is recorded because it is what the user needs to recognise the
/// item — and, if Put Back is ever unavailable, where to put it back to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashOperation {
    pub id: u64,
    pub target: PathBuf,
    pub total_bytes: u64,
    pub trashed_at_ms: u64,
    pub log_error: Option<String>,
}

/// One completed cleanup run, recorded exactly as the backend reported it.
///
/// There is no per-path result here because Mole publishes none: it re-discovers
/// candidates at execution time and reports scope, completion, and warnings.
/// Journalling the reviewed preview rows as if they had been removed would
/// record a guess as a fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupOperation {
    pub id: u64,
    pub backend: String,
    pub backend_version: String,
    pub preview_generated_at: String,
    pub reviewed_items: u64,
    pub reviewed_potential_cleanup: Option<String>,
    pub system_scope: CleanupSystemScope,
    pub completion: CleanupCompletion,
    pub warnings: Vec<String>,
    pub executed_at_ms: u64,
    pub log_error: Option<String>,
}

/// What a finished cleanup run knows before it has an id or a timestamp.
#[derive(Debug, Clone)]
pub struct CleanupRecord {
    pub backend: String,
    pub backend_version: String,
    pub preview_generated_at: String,
    pub reviewed_items: u64,
    pub reviewed_potential_cleanup: Option<String>,
    pub execution: CleanupExecution,
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
    /// Recoverable, but not by Nirmoka: the item is in the platform Trash and
    /// the platform owns putting it back. Nothing supersedes this event.
    Trashed {
        id: u64,
        target: PathBuf,
        total_bytes: u64,
        at_ms: u64,
    },
    /// A cleanup run is terminal: the backend removed what it found, and there
    /// is no receipt to undo. Nothing ever supersedes this event.
    Cleaned {
        id: u64,
        backend: String,
        backend_version: String,
        preview_generated_at: String,
        reviewed_items: u64,
        reviewed_potential_cleanup: Option<String>,
        system_scope: CleanupSystemScope,
        completion: CleanupCompletion,
        warnings: Vec<String>,
        at_ms: u64,
    },
}

pub struct DeletionState {
    next_confirmation: u64,
    next_operation: u64,
    pending: HashMap<u64, Pending>,
    operations: Vec<Operation>,
    cleanups: Vec<CleanupOperation>,
    trashed: Vec<TrashOperation>,
    log_path: Option<PathBuf>,
}

impl DeletionState {
    pub fn new(log_path: Option<PathBuf>) -> Self {
        let journal = log_path.as_deref().map(load_journal).unwrap_or_default();
        let Journal {
            operations,
            cleanups,
            trashed,
        } = journal;
        // One id space across every kind, so "operation 4" in the journal names
        // one event rather than three.
        let next_operation = operations
            .iter()
            .map(|op| op.id)
            .chain(cleanups.iter().map(|op| op.id))
            .chain(trashed.iter().map(|op| op.id))
            .max()
            .unwrap_or(0);

        Self {
            next_confirmation: 0,
            next_operation,
            pending: HashMap::new(),
            operations,
            cleanups,
            trashed,
            log_path,
        }
    }

    pub fn prepare(&mut self, pending: Pending) -> u64 {
        self.next_confirmation = self.next_confirmation.saturating_add(1);
        let token = self.next_confirmation;
        // A newly prepared operation supersedes any dialog left open. This
        // bounds memory and prevents confirming an old selection after the user
        // has moved on to another row.
        self.pending.clear();
        self.pending.insert(token, pending);
        token
    }

    pub fn take_pending(&mut self, token: u64) -> Option<Pending> {
        self.pending.remove(&token)
    }

    pub fn record_delete(&mut self, receipt: DeleteReceipt) -> io::Result<Operation> {
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
        // A recoverable operation is not successful until its undo receipt is
        // durable. Never convert this failure into metadata on an operation the
        // caller could mistake for safely recorded deletion.
        self.append(&event)?;
        let operation = Operation {
            id,
            receipt,
            deleted_at_ms: at_ms,
            undone_at_ms: None,
            log_error: None,
        };
        self.operations.push(operation.clone());
        Ok(operation)
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

    /// Record one completed cleanup run.
    ///
    /// This cannot fail the operation the way `record_delete` does. A deletion
    /// receipt is what makes recovery possible, so an unwritable journal means
    /// the operation was not safely performed. A cleanup run has already
    /// happened inside the backend and cannot be undone, so hiding it behind a
    /// write error would lose the only record the user has. The failure travels
    /// beside the result instead.
    pub fn record_cleanup(&mut self, record: CleanupRecord) -> CleanupOperation {
        self.next_operation = self.next_operation.saturating_add(1);
        let id = self.next_operation;
        let at_ms = now_ms();
        let log_error = self
            .append(&LogEvent::Cleaned {
                id,
                backend: record.backend.clone(),
                backend_version: record.backend_version.clone(),
                preview_generated_at: record.preview_generated_at.clone(),
                reviewed_items: record.reviewed_items,
                reviewed_potential_cleanup: record.reviewed_potential_cleanup.clone(),
                system_scope: record.execution.system_scope,
                completion: record.execution.completion,
                warnings: record.execution.warnings.clone(),
                at_ms,
            })
            .err()
            .map(|error| error.to_string());

        let operation = CleanupOperation {
            id,
            backend: record.backend,
            backend_version: record.backend_version,
            preview_generated_at: record.preview_generated_at,
            reviewed_items: record.reviewed_items,
            reviewed_potential_cleanup: record.reviewed_potential_cleanup,
            system_scope: record.execution.system_scope,
            completion: record.execution.completion,
            warnings: record.execution.warnings,
            executed_at_ms: at_ms,
            log_error,
        };
        self.cleanups.push(operation.clone());
        operation
    }

    /// Record one item moved to the Trash.
    ///
    /// This follows [`record_cleanup`](Self::record_cleanup) rather than
    /// [`record_delete`](Self::record_delete), and the difference is not
    /// stylistic. Those two rules disagree on one question: does recovery
    /// depend on our record? For rip it did — the receipt was the only route
    /// back, so an unwritable journal meant the deletion was not safely
    /// performed. For the Trash it does not; the Trash is its own record.
    /// Failing here would hide a move that already happened.
    pub fn record_trash(&mut self, target: PathBuf, total_bytes: u64) -> TrashOperation {
        self.next_operation = self.next_operation.saturating_add(1);
        let id = self.next_operation;
        let at_ms = now_ms();
        let log_error = self
            .append(&LogEvent::Trashed {
                id,
                target: target.clone(),
                total_bytes,
                at_ms,
            })
            .err()
            .map(|error| error.to_string());

        let operation = TrashOperation {
            id,
            target,
            total_bytes,
            trashed_at_ms: at_ms,
            log_error,
        };
        self.trashed.push(operation.clone());
        operation
    }

    pub fn operations(&self) -> Vec<Operation> {
        self.operations.iter().rev().cloned().collect()
    }

    pub fn trashed(&self) -> Vec<TrashOperation> {
        self.trashed.iter().rev().cloned().collect()
    }

    pub fn cleanups(&self) -> Vec<CleanupOperation> {
        self.cleanups.iter().rev().cloned().collect()
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

/// Everything one journal file holds, reloaded.
#[derive(Debug, Default)]
struct Journal {
    operations: Vec<Operation>,
    cleanups: Vec<CleanupOperation>,
    trashed: Vec<TrashOperation>,
}

fn load_journal(path: &Path) -> Journal {
    let Ok(text) = fs::read_to_string(path) else {
        return Journal::default();
    };

    let mut operations = Vec::<Operation>::new();
    let mut cleanups = Vec::<CleanupOperation>::new();
    let mut trashed = Vec::<TrashOperation>::new();
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
            LogEvent::Cleaned {
                id,
                backend,
                backend_version,
                preview_generated_at,
                reviewed_items,
                reviewed_potential_cleanup,
                system_scope,
                completion,
                warnings,
                at_ms,
            } => cleanups.push(CleanupOperation {
                id,
                backend,
                backend_version,
                preview_generated_at,
                reviewed_items,
                reviewed_potential_cleanup,
                system_scope,
                completion,
                warnings,
                executed_at_ms: at_ms,
                // A reloaded entry is durable by definition — it was read back
                // off disk.
                log_error: None,
            }),
            LogEvent::Trashed {
                id,
                target,
                total_bytes,
                at_ms,
            } => trashed.push(TrashOperation {
                id,
                target,
                total_bytes,
                trashed_at_ms: at_ms,
                log_error: None,
            }),
        }
    }
    Journal {
        operations,
        cleanups,
        trashed,
    }
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
        let pending = |target: &str| {
            Pending::Delete(PendingDelete {
                scan_id: 1,
                plan: DeletePlan::new(
                    "rip",
                    PathBuf::from("/scan"),
                    PathBuf::from(target),
                    nirmoka_adapter::DeleteMode::Trash,
                ),
                total_bytes: 1,
            })
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
        let operation = state.record_delete(receipt).unwrap();
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

    fn cleanup_record() -> CleanupRecord {
        CleanupRecord {
            backend: "mole".to_string(),
            backend_version: "1.48.1".to_string(),
            preview_generated_at: "2026-08-02 12:00:00".to_string(),
            reviewed_items: 6,
            reviewed_potential_cleanup: Some("At least 192.00MB".to_string()),
            execution: CleanupExecution {
                system_scope: CleanupSystemScope::UserOnly,
                completion: CleanupCompletion::Partial,
                warnings: vec!["System-level cleanup was skipped.".to_string()],
            },
        }
    }

    #[test]
    fn a_cleanup_run_survives_reload_and_shares_the_deletion_id_space() {
        let path = std::env::temp_dir().join(format!(
            "nirmoka-cleanup-journal-{}.jsonl",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);

        let mut state = DeletionState::new(Some(path.clone()));
        let deleted = state
            .record_delete(DeleteReceipt::new(
                "rip",
                PathBuf::from("/scan/file"),
                PathBuf::from("/recovery/1"),
                PathBuf::from("/recovery/1/scan/file"),
            ))
            .unwrap();
        let cleaned = state.record_cleanup(cleanup_record());
        assert_eq!(cleaned.id, deleted.id + 1);
        assert!(cleaned.log_error.is_none());

        let reloaded = DeletionState::new(Some(path.clone()));
        let cleanups = reloaded.cleanups();
        assert_eq!(cleanups.len(), 1);
        assert_eq!(cleanups[0], cleaned);
        assert_eq!(reloaded.operations().len(), 1);

        // A later run must not reuse an id either kind already took.
        let mut reloaded = reloaded;
        assert_eq!(reloaded.record_cleanup(cleanup_record()).id, cleaned.id + 1);
        let _ = fs::remove_file(path);
    }

    /// The mirror image of the deletion rule. A cleanup run cannot be undone, so
    /// a failed journal write must report the result it could not record rather
    /// than swallowing it.
    #[test]
    fn a_failed_cleanup_journal_write_still_reports_the_run() {
        let directory = std::env::temp_dir().join(format!(
            "nirmoka-cleanup-journal-directory-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir(&directory).unwrap();

        let mut state = DeletionState::new(Some(directory.clone()));
        let operation = state.record_cleanup(cleanup_record());

        assert!(operation.log_error.is_some(), "the write failure is stated");
        assert_eq!(operation.completion, CleanupCompletion::Partial);
        assert_eq!(state.cleanups().len(), 1, "and the run is still known");
        let _ = fs::remove_dir_all(directory);
    }

    /// One slot for the pending operation, whichever kind it is. Preparing a
    /// move to the Trash must retire a deletion dialog the user left open, and
    /// the reverse, or a stale confirmation stays live behind the new one.
    #[test]
    fn preparing_either_kind_supersedes_the_other() {
        let mut state = DeletionState::new(None);

        let delete = state.prepare(Pending::Delete(PendingDelete {
            scan_id: 1,
            plan: DeletePlan::new(
                "rip",
                PathBuf::from("/scan"),
                PathBuf::from("/scan/old"),
                nirmoka_adapter::DeleteMode::Trash,
            ),
            total_bytes: 1,
        }));
        let trash = state.prepare(Pending::Trash(PendingTrash {
            scan_id: 1,
            scan_root: PathBuf::from("/scan"),
            target: PathBuf::from("/scan/current"),
            total_bytes: 2,
        }));

        assert!(state.take_pending(delete).is_none());
        assert!(matches!(state.take_pending(trash), Some(Pending::Trash(_))));
    }

    #[test]
    fn a_trashed_item_survives_reload_and_shares_the_id_space() {
        let path = std::env::temp_dir().join(format!(
            "nirmoka-trash-journal-{}.jsonl",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);

        let mut state = DeletionState::new(Some(path.clone()));
        let cleaned = state.record_cleanup(cleanup_record());
        let trashed = state.record_trash(PathBuf::from("/scan/big"), 4096);

        assert_eq!(trashed.id, cleaned.id + 1, "one id space, not three");
        assert!(trashed.log_error.is_none());

        let reloaded = DeletionState::new(Some(path.clone())).trashed();
        assert_eq!(reloaded.len(), 1);
        assert_eq!(reloaded[0], trashed);
        let _ = fs::remove_file(path);
    }

    /// The mirror of the cleanup rule, and the opposite of the rip receipt
    /// rule. The item is already in the Trash; refusing to report it because
    /// the journal could not be written would lose the only account of it the
    /// window can show.
    #[test]
    fn a_failed_trash_journal_write_still_reports_the_move() {
        let directory = std::env::temp_dir().join(format!(
            "nirmoka-trash-journal-directory-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir(&directory).unwrap();

        let mut state = DeletionState::new(Some(directory.clone()));
        let operation = state.record_trash(PathBuf::from("/scan/big"), 4096);

        assert!(operation.log_error.is_some(), "the write failure is stated");
        assert_eq!(state.trashed().len(), 1, "and the move is still known");
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn a_failed_delete_journal_write_records_no_success() {
        let directory = std::env::temp_dir().join(format!(
            "nirmoka-operation-log-directory-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir(&directory).unwrap();

        let receipt = DeleteReceipt::new(
            "rip",
            PathBuf::from("/scan/file"),
            PathBuf::from("/recovery/1"),
            PathBuf::from("/recovery/1/scan/file"),
        );
        let mut state = DeletionState::new(Some(directory.clone()));

        assert!(state.record_delete(receipt).is_err());
        assert!(state.operations().is_empty());
        let _ = fs::remove_dir_all(directory);
    }
}
