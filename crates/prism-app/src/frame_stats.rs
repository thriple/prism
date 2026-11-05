//! Frame counting for the debug overlay.
//!
//! The caller passes in the time, so this stays deterministic and testable.

/// Counts frames and reports a smoothed frame rate.
#[derive(Clone, Debug, Default)]
pub struct FrameStats {
    frames: u64,
    window_start: Option<f64>,
    frames_in_window: u32,
    fps: Option<f64>,
}

impl FrameStats {
    /// How long, in seconds, each frame-rate sample covers.
    pub const WINDOW: f64 = 0.5;

    pub fn new() -> Self {
        Self::default()
    }

    /// Record one presented frame at time `now_s` (seconds, any origin).
    pub fn frame(&mut self, now_s: f64) {
        self.frames += 1;
        let start = *self.window_start.get_or_insert(now_s);
        self.frames_in_window += 1;
        let elapsed = now_s - start;
        if elapsed >= Self::WINDOW {
            // Frames after the first one in the window, over the time they took.
            self.fps = Some(f64::from(self.frames_in_window - 1) / elapsed);
            self.window_start = Some(now_s);
            self.frames_in_window = 1;
        }
    }

    /// Total frames since start.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Frames per second over the last complete sample, once there is one.
    pub fn fps(&self) -> Option<f64> {
        self.fps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_frames_and_measures_rate() {
        let mut s = FrameStats::new();
        assert_eq!(s.fps(), None);
        // 60 frames per second for one second.
        for i in 0..=60 {
            s.frame(f64::from(i) / 60.0);
        }
        assert_eq!(s.frames(), 61);
        let fps = s.fps().expect("a sample after one second");
        assert!((fps - 60.0).abs() < 1e-9, "fps was {fps}");
    }

    #[test]
    fn no_rate_before_the_first_sample_ends() {
        let mut s = FrameStats::new();
        s.frame(10.0);
        s.frame(10.1);
        assert_eq!(s.frames(), 2);
        assert_eq!(s.fps(), None);
    }
}
