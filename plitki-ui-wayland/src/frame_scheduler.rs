use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

/// Information about the last presentation.
#[derive(Debug, Clone)]
struct PresentationInfo {
    /// Timestamp of the last presentation.
    last_presentation: Duration,
    /// Time between successive presentations.
    refresh_time: Duration,
    /// Latency, in presentations.
    ///
    /// This is computed as the number of presentations between the first presentation after a
    /// render start time and the presentation where the frame is actually shown. For example, if
    /// this is `0`, the frame is predicted to be shown on the next presentation after the render
    /// start time. If this is `1`, the frame will be delayed to one presentation after that.
    ///
    /// Note that this latency, as it is, includes both any compositor-induced latency, and the
    /// latency arising from rendering simply taking a long time (e.g. with an FPS cap).
    latency: u32,
}

/// Scheduler that computes target rendering time.
#[derive(Debug, Clone)]
pub struct FrameScheduler {
    /// Information about the last presentation.
    presentation_info: Arc<Mutex<Option<PresentationInfo>>>,
}

impl FrameScheduler {
    /// Creates a new `FrameScheduler`.
    pub fn new() -> Self {
        Self {
            presentation_info: Arc::new(Mutex::new(None)),
        }
    }

    /// Informs the frame scheduler that a frame has been presented.
    ///
    /// A frame which started rendering at `render_start_time` has been presented at
    /// `presentation_time`, with the reported `refresh_time` until the next presentation.
    pub fn presented(
        &self,
        render_start_time: Duration,
        presentation_time: Duration,
        refresh_time: Duration,
    ) {
        let mut first_refresh_after_render_start = presentation_time - refresh_time;
        let mut latency = 1;
        while first_refresh_after_render_start > render_start_time {
            first_refresh_after_render_start -= refresh_time;
            latency += 1;
        }
        first_refresh_after_render_start += refresh_time;
        latency -= 1;

        *self.presentation_info.lock().unwrap() = Some(PresentationInfo {
            last_presentation: presentation_time,
            refresh_time,
            latency,
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
            latency,
        } = presentation_info.unwrap();

        let mut next_refresh = last_presentation;
        while next_refresh < render_start_time {
            next_refresh += refresh_time;
        }

        next_refresh + refresh_time * latency
    }
}
