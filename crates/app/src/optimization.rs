//! Mole system-optimization review and confirmation state.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use nirmoka_adapter::{CancelToken, OptimizePreview};

pub const PREVIEW_LIFETIME: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone)]
pub struct PendingOptimize {
    pub backend: String,
    pub preview: OptimizePreview,
}

#[derive(Debug, Clone)]
struct ReviewedOptimize {
    pending: PendingOptimize,
    observed_at: Instant,
}

#[derive(Debug, Clone)]
pub struct OptimizePreparation {
    pub token: u64,
    pub pending: PendingOptimize,
    pub expires_in: Duration,
}

#[derive(Default, Debug)]
pub struct OptimizeState {
    next_preview: u64,
    active_preview: Option<(u64, CancelToken)>,
    active_execution: Option<CancelToken>,
    next_confirmation: u64,
    latest: Option<ReviewedOptimize>,
    pending: HashMap<u64, ReviewedOptimize>,
}

impl OptimizeState {
    pub fn start_preview(&mut self) -> Result<(u64, CancelToken), String> {
        if self.active_preview.is_some() {
            return Err("an optimization preview is already running".to_string());
        }
        self.next_preview = self.next_preview.saturating_add(1);
        let id = self.next_preview;
        let cancel = CancelToken::new();
        self.active_preview = Some((id, cancel.clone()));
        self.latest = None;
        self.pending.clear();
        Ok((id, cancel))
    }

    pub fn finish_preview(&mut self, id: u64) {
        if self.active_preview.as_ref().map(|(active, _)| *active) == Some(id) {
            self.active_preview = None;
        }
    }

    pub fn cancel_preview(&self) -> bool {
        let Some((_, cancel)) = &self.active_preview else {
            return false;
        };
        cancel.cancel();
        true
    }

    pub fn remember(&mut self, backend: impl Into<String>, preview: OptimizePreview) {
        self.pending.clear();
        self.latest = Some(ReviewedOptimize {
            pending: PendingOptimize {
                backend: backend.into(),
                preview,
            },
            observed_at: Instant::now(),
        });
    }

    pub fn prepare(&mut self, now: Instant) -> Option<OptimizePreparation> {
        let reviewed = self.latest.as_ref()?;
        let age = now.checked_duration_since(reviewed.observed_at)?;
        if age >= PREVIEW_LIFETIME {
            return None;
        }
        self.next_confirmation = self.next_confirmation.saturating_add(1);
        let token = self.next_confirmation;
        let reviewed = reviewed.clone();
        self.pending.clear();
        self.pending.insert(token, reviewed.clone());
        Some(OptimizePreparation {
            token,
            pending: reviewed.pending,
            expires_in: PREVIEW_LIFETIME - age,
        })
    }

    pub fn take(&mut self, token: u64, now: Instant) -> Option<PendingOptimize> {
        let reviewed = self.pending.remove(&token)?;
        let age = now.checked_duration_since(reviewed.observed_at)?;
        if age >= PREVIEW_LIFETIME {
            return None;
        }
        self.latest = None;
        self.pending.clear();
        Some(reviewed.pending)
    }

    pub fn start_execution(&mut self) -> Result<CancelToken, String> {
        if self.active_execution.is_some() {
            return Err("an optimization run is already in progress".to_string());
        }
        let cancel = CancelToken::new();
        self.active_execution = Some(cancel.clone());
        Ok(cancel)
    }

    pub fn finish_execution(&mut self) {
        self.active_execution = None;
    }

    pub fn cancel_execution(&self) -> bool {
        let Some(cancel) = &self.active_execution else {
            return false;
        };
        cancel.cancel();
        true
    }
}
