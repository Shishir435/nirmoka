//! Cleanup review and confirmation state.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use nirmoka_adapter::CleanupPreview;

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

#[derive(Default)]
pub struct CleanupState {
    next_confirmation: u64,
    latest: Option<ReviewedCleanup>,
    pending: HashMap<u64, ReviewedCleanup>,
}

impl CleanupState {
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
}
