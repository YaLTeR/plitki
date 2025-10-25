use std::time::{Duration, Instant};

use circular_queue::CircularQueue;

pub struct FrameClock {
    last_frame: Option<Instant>,
    frame_times: CircularQueue<Duration>,
}

impl FrameClock {
    pub fn new() -> Self {
        Self {
            last_frame: None,
            frame_times: CircularQueue::with_capacity(10),
        }
    }

    pub fn frame(&mut self) {
        let now = Instant::now();
        if let Some(last) = self.last_frame {
            let passed = now - last;
            self.frame_times.push(passed);
        }
        self.last_frame = Some(now);
    }

    pub fn fps(&self) -> Option<f32> {
        let mut iter = self.frame_times.iter();
        let mut fps = 1. / iter.next()?.as_secs_f32();
        for frame_time in iter {
            fps = 0.8 * fps + 0.2 / frame_time.as_secs_f32();
        }
        Some(fps)
    }
}
