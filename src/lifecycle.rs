use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Clone, Debug)]
pub struct LifecycleState {
    started: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
    revision: Arc<str>,
}

impl LifecycleState {
    #[must_use]
    pub fn from_environment() -> Self {
        let revision = std::env::var("K_REVISION")
            .or_else(|_| std::env::var("GITHUB_SHA"))
            .unwrap_or_else(|_| "local".to_owned());
        Self::new(revision)
    }

    #[must_use]
    pub fn new(revision: impl Into<Arc<str>>) -> Self {
        Self {
            started: Arc::new(AtomicBool::new(false)),
            ready: Arc::new(AtomicBool::new(false)),
            revision: revision.into(),
        }
    }

    pub fn mark_started(&self) {
        self.started.store(true, Ordering::Release);
        self.ready.store(true, Ordering::Release);
    }

    pub fn begin_drain(&self) {
        self.ready.store(false, Ordering::Release);
    }

    #[must_use]
    pub fn is_started(&self) -> bool {
        self.started.load(Ordering::Acquire)
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.is_started() && self.ready.load(Ordering::Acquire)
    }

    #[must_use]
    pub fn revision(&self) -> &str {
        &self.revision
    }
}

#[cfg(test)]
mod tests {
    use super::LifecycleState;

    #[test]
    fn lifecycle_is_fail_closed_and_withdraws_for_drain() {
        let state = LifecycleState::new("test");
        assert!(!state.is_started());
        assert!(!state.is_ready());
        state.mark_started();
        assert!(state.is_started());
        assert!(state.is_ready());
        state.begin_drain();
        assert!(state.is_started());
        assert!(!state.is_ready());
    }
}
