//! Uninstall review and confirmation state.
//!
//! The same shape as [`crate::cleanup`], for the same reason: a plan the backend
//! produced is bound to a one-time token, and the token — not a path, and not an
//! application name — is what the frontend gets back.
//!
//! One difference matters. A cleanup preview names categories the backend
//! rediscovers; an uninstall preview names *specific applications the user
//! chose*, so the identifiers are kept beside the plan and re-validated against
//! the live inventory at execution. See ADR 0027.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use nirmoka_adapter::{CancelToken, UninstallPreview};

/// Review state expires quickly, because everything it describes is a claim about
/// the disk a moment ago. Same lifetime as a cleanup review, deliberately: two
/// numbers here would be two things to explain and one to get wrong.
pub const PREVIEW_LIFETIME: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone)]
struct ReviewedUninstall {
    backend: String,
    preview: UninstallPreview,
    observed_at: Instant,
}

#[derive(Debug, Clone)]
pub struct PendingUninstall {
    pub backend: String,
    pub preview: UninstallPreview,
}

#[derive(Debug, Clone)]
pub struct UninstallPreparation {
    pub token: u64,
    pub pending: PendingUninstall,
    pub expires_in: Duration,
}

#[derive(Default)]
pub struct UninstallState {
    next_preview: u64,
    active_preview: Option<ActivePreview>,
    active_execution: Option<CancelToken>,
    next_confirmation: u64,
    latest: Option<ReviewedUninstall>,
    pending: HashMap<u64, ReviewedUninstall>,
}

struct ActivePreview {
    id: u64,
    cancel: CancelToken,
}

impl UninstallState {
    pub fn start_preview(&mut self) -> Result<(u64, CancelToken), String> {
        if self.active_preview.is_some() {
            return Err("an uninstall preview is already running".to_string());
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
    /// consumed, so a double click cannot spend the token on a run that would
    /// have been refused anyway.
    pub fn start_execution(&mut self) -> Result<CancelToken, String> {
        if self.active_execution.is_some() {
            return Err("an uninstall is already in progress".to_string());
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

    /// Drop the reviewed plan after a run, successful or not.
    ///
    /// Every path in it is now a claim about the past, and the application it
    /// named may no longer exist. A second run reviews a fresh plan.
    pub fn forget(&mut self) {
        self.latest = None;
        self.pending.clear();
    }

    pub fn remember(
        &mut self,
        backend: impl Into<String>,
        preview: UninstallPreview,
        observed_at: Instant,
    ) {
        self.pending.clear();
        self.latest = Some(ReviewedUninstall {
            backend: backend.into(),
            preview,
            observed_at,
        });
    }

    pub fn prepare(&mut self, now: Instant) -> Option<UninstallPreparation> {
        let reviewed = self.latest.as_ref()?;
        let age = now.checked_duration_since(reviewed.observed_at)?;
        if age >= PREVIEW_LIFETIME {
            return None;
        }
        let expires_in = PREVIEW_LIFETIME - age;
        // A plan with no paths in it is not something to confirm. Offering a
        // button here would run a removal whose review said it removes nothing.
        if reviewed.preview.total_items() == 0 {
            return None;
        }

        self.next_confirmation = self.next_confirmation.saturating_add(1);
        let token = self.next_confirmation;
        let reviewed = reviewed.clone();
        self.pending.clear();
        self.pending.insert(token, reviewed.clone());

        Some(UninstallPreparation {
            token,
            pending: PendingUninstall {
                backend: reviewed.backend,
                preview: reviewed.preview,
            },
            expires_in,
        })
    }

    /// Consume one confirmation. Taking it here makes replay impossible even when
    /// the execution that follows fails.
    pub fn take(&mut self, token: u64, now: Instant) -> Option<PendingUninstall> {
        let reviewed = self.pending.remove(&token)?;
        let age = now.checked_duration_since(reviewed.observed_at)?;
        if age >= PREVIEW_LIFETIME {
            return None;
        }
        Some(PendingUninstall {
            backend: reviewed.backend,
            preview: reviewed.preview,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use nirmoka_adapter::{UninstallApp, UninstallItem, UninstallItemScope, UninstallPreview};

    use super::{UninstallState, PREVIEW_LIFETIME};

    fn preview(name: &str, items: usize) -> UninstallPreview {
        UninstallPreview {
            backend_version: "1.48.1".to_string(),
            requested: vec![name.to_string()],
            apps: vec![UninstallApp {
                name: name.to_string(),
                homebrew_cask: false,
                reported_size: Some("1MB".to_string()),
                items: (0..items)
                    .map(|index| UninstallItem {
                        display_path: format!("/Applications/{name}-{index}.app"),
                        reported_size: None,
                        scope: UninstallItemScope::Removed,
                    })
                    .collect(),
            }],
            reported_total: Some("1MB".to_string()),
            warnings: Vec::new(),
            notes: Vec::new(),
            transcript: String::new(),
        }
    }

    #[test]
    fn confirmation_tokens_are_one_time_and_superseded() {
        let now = Instant::now();
        let mut state = UninstallState::default();
        state.remember("mole", preview("first", 1), now);

        let old = state.prepare(now).expect("first preparation").token;
        let current = state.prepare(now).expect("second preparation").token;

        assert!(state.take(old, now).is_none());
        assert!(state.take(current, now).is_some());
        assert!(state.take(current, now).is_none());
    }

    /// The case this guards is a user reviewing one application and confirming a
    /// different one. A new review has to invalidate the old token, or the token
    /// names a plan nobody is looking at any more.
    #[test]
    fn reviewing_another_application_invalidates_the_existing_confirmation() {
        let now = Instant::now();
        let mut state = UninstallState::default();
        state.remember("mole", preview("first", 1), now);
        let token = state.prepare(now).expect("preparation").token;

        state.remember("mole", preview("second", 1), now);

        assert!(state.take(token, now).is_none());
        let current = state.prepare(now).expect("a new preparation");
        assert_eq!(current.pending.preview.apps[0].name, "second");
    }

    #[test]
    fn stale_previews_and_confirmations_expire() {
        let now = Instant::now();
        let mut state = UninstallState::default();
        state.remember("mole", preview("old", 1), now);
        assert!(state.prepare(now + PREVIEW_LIFETIME).is_none());

        state.remember("mole", preview("fresh", 1), now);
        let token = state.prepare(now).expect("preparation").token;
        assert!(state.take(token, now + PREVIEW_LIFETIME).is_none());
    }

    #[test]
    fn a_plan_with_no_paths_cannot_be_prepared() {
        let now = Instant::now();
        let mut state = UninstallState::default();
        state.remember("mole", preview("empty", 0), now);

        assert!(state.prepare(now).is_none());
    }

    #[test]
    fn a_spent_plan_is_forgotten_so_the_next_run_reviews_a_fresh_one() {
        let now = Instant::now();
        let mut state = UninstallState::default();
        state.remember("mole", preview("spent", 1), now);
        let token = state.prepare(now).expect("preparation").token;

        state.forget();

        assert!(state.take(token, now).is_none());
        assert!(state.prepare(now).is_none());
    }

    #[test]
    fn only_one_uninstall_holds_the_execution_slot() {
        let mut state = UninstallState::default();
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
        let mut state = UninstallState::default();
        let (id, token) = state.start_preview().expect("first preview");

        assert!(!token.is_cancelled());
        assert!(state.start_preview().is_err());
        assert!(state.cancel_preview());
        assert!(token.is_cancelled());

        state.finish_preview(id);
        assert!(!state.cancel_preview());
        assert!(state.start_preview().is_ok());
    }
}
