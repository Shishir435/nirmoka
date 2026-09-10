//! Cleanup review and confirmation state.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use nirmoka_adapter::{CancelToken, CleanupPreview};

/// Review state expires quickly because Mole performs fresh discovery during
/// execution. A preview is evidence, not an immutable delete list.
pub const PREVIEW_LIFETIME: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone)]
struct ReviewedCleanup {
    backend: String,
    backend_instead_of: Option<String>,
    preview: CleanupPreview,
    observed_at: Instant,
}

#[derive(Debug, Clone)]
pub struct PendingCleanup {
    pub backend: String,
    pub backend_instead_of: Option<String>,
    pub preview: CleanupPreview,
}

#[derive(Debug, Clone)]
pub struct CleanupPreparation {
    pub token: u64,
    pub pending: PendingCleanup,
    pub expires_in: Duration,
}

#[derive(Default, Debug)]
pub struct CleanupState {
    next_preview: u64,
    active_preview: Option<ActivePreview>,
    active_execution: Option<CancelToken>,
    next_confirmation: u64,
    latest: Option<ReviewedCleanup>,
    pending: HashMap<u64, ReviewedCleanup>,
}

#[derive(Debug)]
struct ActivePreview {
    id: u64,
    cancel: CancelToken,
}

impl CleanupState {
    pub fn start_preview(&mut self) -> Result<(u64, CancelToken), String> {
        if self.active_preview.is_some() {
            return Err("a cleanup preview is already running".to_string());
        }

        self.next_preview = self.next_preview.saturating_add(1);
        let id = self.next_preview;
        let cancel = CancelToken::new();
        self.active_preview = Some(ActivePreview {
            id,
            cancel: cancel.clone(),
        });
        Ok((id, cancel))
    }

    pub fn finish_preview(&mut self, id: u64) {
        if self.active_preview.as_ref().map(|preview| preview.id) == Some(id) {
            self.active_preview = None;
        }
    }

    pub fn cancel_preview(&self) -> bool {
        let Some(preview) = &self.active_preview else {
            return false;
        };
        preview.cancel.cancel();
        true
    }

    /// Claim the single execution slot. Claimed before the confirmation is
    /// consumed, so a second click cannot spend the token on a run that would
    /// have been refused anyway.
    pub fn start_execution(&mut self) -> Result<CancelToken, String> {
        if self.active_execution.is_some() {
            return Err("a cleanup run is already in progress".to_string());
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

    /// Drop the reviewed preview after a run, successful or not.
    ///
    /// Mole removed whatever it found, so every path in the old preview is now a
    /// claim about the past. A second run must review a fresh discovery.
    pub fn forget(&mut self) {
        self.latest = None;
        self.pending.clear();
    }

    pub fn remember(
        &mut self,
        backend: impl Into<String>,
        backend_instead_of: Option<String>,
        preview: CleanupPreview,
        observed_at: Instant,
    ) {
        self.pending.clear();
        self.latest = Some(ReviewedCleanup {
            backend: backend.into(),
            backend_instead_of,
            preview,
            observed_at,
        });
    }

    /// The review already held, if it is still current.
    ///
    /// A dry run takes minutes, and it was possible to spend them on one screen
    /// and then find another offering to spend them again: the preview was
    /// remembered here and nothing could read it back. Same freshness rule as
    /// [`Self::prepare`], because a review too old to act on is too old to show
    /// as though it could be acted on.
    pub fn reviewed(&self, now: Instant) -> Option<PendingCleanup> {
        let reviewed = self.latest.as_ref()?;
        let age = now.checked_duration_since(reviewed.observed_at)?;
        if age >= PREVIEW_LIFETIME {
            return None;
        }
        Some(PendingCleanup {
            backend: reviewed.backend.clone(),
            backend_instead_of: reviewed.backend_instead_of.clone(),
            preview: reviewed.preview.clone(),
        })
    }

    pub fn prepare(&mut self, now: Instant) -> Option<CleanupPreparation> {
        let reviewed = self.latest.as_ref()?;
        let age = now.checked_duration_since(reviewed.observed_at)?;
        if age >= PREVIEW_LIFETIME {
            return None;
        }
        let expires_in = PREVIEW_LIFETIME - age;
        if reviewed.preview.total_items == 0 {
            return None;
        }

        self.next_confirmation = self.next_confirmation.saturating_add(1);
        let token = self.next_confirmation;
        let reviewed = reviewed.clone();
        self.pending.clear();
        self.pending.insert(token, reviewed.clone());

        Some(CleanupPreparation {
            token,
            pending: PendingCleanup {
                backend: reviewed.backend,
                backend_instead_of: reviewed.backend_instead_of,
                preview: reviewed.preview,
            },
            expires_in,
        })
    }

    /// Consume one confirmation. Cleanup execution will be the only caller;
    /// taking it here makes replay impossible even when execution fails.
    pub fn take(&mut self, token: u64, now: Instant) -> Option<PendingCleanup> {
        let reviewed = self.pending.remove(&token)?;
        let age = now.checked_duration_since(reviewed.observed_at)?;
        if age >= PREVIEW_LIFETIME {
            return None;
        }
        Some(PendingCleanup {
            backend: reviewed.backend,
            backend_instead_of: reviewed.backend_instead_of,
            preview: reviewed.preview,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use nirmoka_adapter::{CleanupPreview, CleanupSystemScope};

    use super::{CleanupState, PREVIEW_LIFETIME};

    fn preview(generated_at: &str, total_items: u64) -> CleanupPreview {
        CleanupPreview {
            backend_version: "1.48.1".to_string(),
            generated_at: generated_at.to_string(),
            categories: Vec::new(),
            potential_cleanup: Some("1MB".to_string()),
            total_items,
            system_scope: CleanupSystemScope::UserOnly,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn confirmation_tokens_are_one_time_and_superseded() {
        let now = Instant::now();
        let mut state = CleanupState::default();
        state.remember("mole", None, preview("first", 1), now);

        let old = state.prepare(now).expect("first preparation").token;
        let current = state.prepare(now).expect("second preparation").token;

        assert!(state.take(old, now).is_none());
        assert!(state.take(current, now).is_some());
        assert!(state.take(current, now).is_none());
    }

    #[test]
    fn a_new_preview_invalidates_an_existing_confirmation() {
        let now = Instant::now();
        let mut state = CleanupState::default();
        state.remember("mole", None, preview("first", 1), now);
        let token = state.prepare(now).expect("preparation").token;

        state.remember("mole", None, preview("second", 1), now);

        assert!(state.take(token, now).is_none());
    }

    #[test]
    fn stale_previews_and_confirmations_expire() {
        let now = Instant::now();
        let mut state = CleanupState::default();
        state.remember("mole", None, preview("old", 1), now);
        assert!(state.prepare(now + PREVIEW_LIFETIME).is_none());

        state.remember("mole", None, preview("fresh", 1), now);
        let token = state.prepare(now).expect("preparation").token;
        assert!(state.take(token, now + PREVIEW_LIFETIME).is_none());
    }

    #[test]
    fn an_empty_preview_cannot_be_prepared_for_cleanup() {
        let now = Instant::now();
        let mut state = CleanupState::default();
        state.remember("mole", None, preview("empty", 0), now);

        assert!(state.prepare(now).is_none());
    }

    #[test]
    fn a_spent_preview_is_forgotten_so_the_next_run_reviews_a_fresh_one() {
        let now = Instant::now();
        let mut state = CleanupState::default();
        state.remember("mole", None, preview("spent", 1), now);
        let token = state.prepare(now).expect("preparation").token;

        state.forget();

        assert!(state.take(token, now).is_none());
        assert!(state.prepare(now).is_none());
    }

    #[test]
    fn only_one_cleanup_run_holds_the_execution_slot() {
        let mut state = CleanupState::default();
        assert!(!state.cancel_execution(), "nothing to stop yet");

        let cancel = state.start_execution().expect("first run");
        assert!(state.start_execution().is_err());
        assert!(state.cancel_execution());
        assert!(cancel.is_cancelled());

        state.finish_execution();
        assert!(!state.cancel_execution());
        assert!(state.start_execution().is_ok());
    }

    #[test]
    fn active_preview_token_is_reachable_until_worker_finishes() {
        let mut state = CleanupState::default();
        let (id, token) = state.start_preview().expect("first preview");

        assert!(!token.is_cancelled());
        assert!(state.start_preview().is_err());
        assert!(state.cancel_preview());
        assert!(token.is_cancelled());

        state.finish_preview(id);
        assert!(!state.cancel_preview());
        assert!(state.start_preview().is_ok());
    }

    /// The bug this exists for: a review was produced, remembered, and then
    /// unreachable, so the next screen offered to spend another two minutes
    /// answering the same question.
    #[test]
    fn a_held_review_can_be_read_back_without_running_another() {
        let mut state = CleanupState::default();
        let now = Instant::now();
        state.remember("mole", None, preview("2026-08-20", 12), now);

        let held = state.reviewed(now).expect("the review is still current");

        assert_eq!(held.backend, "mole");
        assert_eq!(held.preview.total_items, 12);
        // Reading it does not consume it: preparing to run still works after.
        assert!(state.reviewed(now).is_some());
        assert!(state.prepare(now).is_some());
    }

    /// Same freshness rule as `prepare`. A review too old to act on must not be
    /// shown as though it could be.
    #[test]
    fn a_stale_review_is_not_offered() {
        let mut state = CleanupState::default();
        let now = Instant::now();
        state.remember("mole", None, preview("2026-08-20", 12), now);

        let later = now + PREVIEW_LIFETIME;

        assert!(state.reviewed(later).is_none());
        assert!(state.prepare(later).is_none());
    }

    #[test]
    fn nothing_held_is_nothing_offered() {
        let state = CleanupState::default();

        assert!(state.reviewed(Instant::now()).is_none());
    }
}
