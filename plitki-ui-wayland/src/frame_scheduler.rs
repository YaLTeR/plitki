use std::{
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

/// Information about the last presentation.
#[derive(Debug, Clone)]
struct PresentationInfo {
    /// Timestamp of the last presentation.
    last_presentation: Duration,
    /// Time between successive presentations.
    refresh_time: Duration,
}

/// Scheduler that computes target rendering time.
#[derive(Debug, Clone)]
pub struct FrameScheduler {
    /// Information about the last presentation.
    presentation_info: Arc<Mutex<Option<PresentationInfo>>>,
    /// Number of commits that haven't received presentation feedback yet.
    pending_commits: Arc<AtomicU32>,
}

impl FrameScheduler {
    /// Creates a new `FrameScheduler`.
    pub fn new() -> Self {
        Self {
            presentation_info: Arc::new(Mutex::new(None)),
            pending_commits: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Informs the frame scheduler that a frame has been discarded.
    pub fn discarded(&self) {
        self.pending_commits.fetch_sub(1, Ordering::Relaxed);
    }

    /// Informs the frame scheduler that a frame has been presented.
    ///
    /// A frame has been presented at `presentation_time`, with the reported `refresh_time` until
    /// the next presentation.
    pub fn presented(&self, presentation_time: Duration, refresh_time: Duration) {
        self.pending_commits.fetch_sub(1, Ordering::Relaxed);

        let mut guard = self.presentation_info.lock().unwrap();
        *guard = Some(PresentationInfo {
            last_presentation: presentation_time,
            refresh_time,
        });
    }

    /// Computes and returns the target time for rendering.
    ///
    /// Returns the predicted presentation time for a frame which starts rendering at
    /// `render_start_time`.
    pub fn get_target_time(&self, render_start_time: Duration) -> Duration {
        let presentation_info = self.presentation_info.lock().unwrap().as_ref().cloned();
        if presentation_info.is_none() {
            // Haven't presented yet, no information about the latency.
            return render_start_time;
        }

        let PresentationInfo {
            last_presentation,
            refresh_time,
        } = presentation_info.unwrap();

        let pending_commits = self.pending_commits.load(Ordering::Relaxed);
        let mut next_refresh = last_presentation + refresh_time * (pending_commits + 1);
        while next_refresh < render_start_time {
            next_refresh += refresh_time;
        }

        next_refresh
    }

    /// Informs the frame scheduler that a frame has been committed.
    pub fn commit(&self) {
        self.pending_commits.fetch_add(1, Ordering::Relaxed);
    }
}
