//! Presentation time selects recorded poses; it never advances the living world.
/// Keep the last world animation pose across pause/resume. Recorded inspector
/// playback and live arena preview have their own independent clocks.
pub fn advance_world_clock(seconds: f32, delta: f32, moving: bool) -> f32 {
    if moving && delta.is_finite() && delta > 0.0 {
        seconds + delta
    } else {
        seconds
    }
}
pub fn frame_index(seconds: f32, dt: f32, frames: usize) -> usize {
    if frames == 0 || !seconds.is_finite() || !dt.is_finite() || dt <= 0.0 {
        return 0;
    }
    ((seconds.max(0.0) / dt).round() as usize) % frames
}

pub fn frame_time(start: f32, frame: usize, fps: u32) -> f32 {
    start + frame as f32 / fps.max(1) as f32
}

pub fn clamped_frame_index(seconds: f32, dt: f32, frames: usize) -> usize {
    if frames == 0 || !seconds.is_finite() || !dt.is_finite() || dt <= 0.0 {
        return 0;
    }
    ((seconds.max(0.0) / dt).round() as usize).min(frames - 1)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipFrame {
    pub index: usize,
    pub seconds: f32,
}

/// A frame must select its poses before it can be captured. Asynchronous GPU
/// completion advances this schedule only once and cannot skip pose preparation.
pub struct ClipSchedule {
    start: f32,
    frames: usize,
    fps: u32,
    next: usize,
    prepared: bool,
    in_flight: bool,
}

impl ClipSchedule {
    /// Include the final recorded pose, holding it until the next 30 fps sample
    /// when its timestamp does not fall exactly on an output frame boundary.
    pub fn recorded(frames: usize, dt: f32, fps: u32) -> Result<Self, &'static str> {
        if frames == 0 || !dt.is_finite() || dt <= 0.0 || fps == 0 {
            return Err("replay must contain frames with valid timing");
        }
        let last_time = (frames - 1) as f32 * dt;
        let samples = (f64::from(last_time) * f64::from(fps)).ceil() + 1.0;
        if samples >= usize::MAX as f64 {
            return Err("recorded replay is too long to export");
        }
        let mut schedule = Self::new(0.0, 1.0, fps)?;
        schedule.frames = samples as usize;
        Ok(schedule)
    }
    pub fn new(start: f32, seconds: f32, fps: u32) -> Result<Self, &'static str> {
        if !start.is_finite() || start < 0.0 || !seconds.is_finite() || seconds <= 0.0 || fps == 0 {
            return Err("clip start, duration and frame rate must be finite and valid");
        }
        let frames = (f64::from(seconds) * f64::from(fps)).round().max(1.0);
        if frames >= usize::MAX as f64 || !(start + seconds).is_finite() {
            return Err("clip time range is too large");
        }
        Ok(Self {
            start,
            frames: frames as usize,
            fps,
            next: 0,
            prepared: false,
            in_flight: false,
        })
    }

    pub fn prepare(&mut self) -> Option<ClipFrame> {
        if self.in_flight || self.is_complete() {
            return None;
        }
        self.prepared = true;
        Some(self.frame())
    }

    pub fn begin_capture(&mut self) -> Option<ClipFrame> {
        if !self.prepared || self.in_flight || self.is_complete() {
            return None;
        }
        self.prepared = false;
        self.in_flight = true;
        Some(self.frame())
    }

    pub fn complete(&mut self, index: usize) -> bool {
        if !self.in_flight || index != self.next {
            return false;
        }
        self.in_flight = false;
        self.next += 1;
        true
    }

    pub fn is_complete(&self) -> bool {
        self.next == self.frames
    }

    pub fn completed_frames(&self) -> usize {
        self.next
    }

    pub fn frame_count(&self) -> usize {
        self.frames
    }

    fn frame(&self) -> ClipFrame {
        ClipFrame {
            index: self.next,
            seconds: frame_time(self.start, self.next, self.fps),
        }
    }

    pub fn metadata(&self, tick: u64, genome: &str, trial: &str) -> serde_json::Value {
        serde_json::json!({
            "world_tick": tick,
            "start_seconds": self.start,
            "last_frame_seconds": frame_time(self.start, self.frames - 1, self.fps),
            "frames": self.frames,
            "fps": self.fps,
            "genome": genome,
            "trial": trial,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paused_and_saved_worlds_do_not_sprint_against_stationary_terrain() {
        let mut seconds = advance_world_clock(0.0, 0.3, true);
        assert_eq!(frame_index(seconds, 0.01, 200), 30);
        for delta in [0.0, 0.2, 4.8, 2000.0] {
            seconds = advance_world_clock(seconds, delta, false);
            assert_eq!(frame_index(seconds, 0.01, 200), 30);
            assert_eq!(advance_world_clock(0.0, delta, false), 0.0);
        }
        seconds = advance_world_clock(seconds, 0.1, true);
        assert_eq!(frame_index(seconds, 0.01, 200), 40);
    }
    #[test]
    fn replay_scrub_and_clip_export_use_recorded_time_independent_of_render_rate() {
        assert_eq!(frame_index(0.5, 1.0 / 240.0, 480), 120);
        assert_eq!(frame_index(2.0, 0.01, 100), 0);
        assert_eq!(frame_index(-1.0, 0.01, 100), 0);
        assert_eq!(frame_index(f32::NAN, 0.01, 100), 0);
        assert_eq!(frame_index(1.0, 0.0, 0), 0);
        assert_eq!(frame_time(1.5, 30, 30), 2.5);
        let slow_capture = (0..120).map(|i| frame_time(0.0, i, 30)).collect::<Vec<_>>();
        assert!((slow_capture[119] - 119.0 / 30.0).abs() < 1.0e-6);
        assert_eq!(slow_capture.len(), 120);
        for (i, time) in slow_capture.iter().enumerate() {
            assert_eq!(frame_index(*time, 1.0 / 240.0, 480), (i * 8) % 480);
        }
    }

    #[test]
    fn clip_first_frame_and_async_completion_require_prepared_recorded_poses() {
        let mut schedule = ClipSchedule::new(1.5, 0.1, 30).unwrap();
        assert!(schedule.begin_capture().is_none());
        for index in 0..3 {
            let frame = schedule.prepare().unwrap();
            assert_eq!(frame.index, index);
            assert_eq!(frame.seconds, 1.5 + index as f32 / 30.0);
            assert_eq!(schedule.begin_capture(), Some(frame));
            // Rendering can take any number of updates without advancing time.
            for _ in 0..(index + 1) * 7 {
                assert!(schedule.prepare().is_none());
                assert!(schedule.begin_capture().is_none());
                assert_eq!(schedule.completed_frames(), index);
            }
            assert!(!schedule.complete(index + 1));
            assert!(schedule.complete(index));
            assert!(!schedule.complete(index));
            // A GPU completion cannot capture the next frame's old poses.
            assert!(schedule.begin_capture().is_none());
        }
        assert!(schedule.is_complete());
        assert!(schedule.prepare().is_none());
    }

    #[test]
    fn clip_metadata_describes_the_exact_fixed_time_frame_sequence() {
        let mut schedule = ClipSchedule::new(2.0, 0.25, 30).unwrap();
        let metadata = schedule.metadata(1234, "abc", "Strike");
        assert_eq!(metadata["world_tick"], 1234);
        assert_eq!(metadata["genome"], "abc");
        assert_eq!(metadata["trial"], "Strike");
        assert_eq!(metadata["fps"], 30);
        assert_eq!(metadata["frames"], 8);
        let mut times = Vec::new();
        while let Some(frame) = schedule.prepare() {
            times.push(frame.seconds);
            schedule.begin_capture().unwrap();
            assert!(schedule.complete(frame.index));
        }
        assert_eq!(times.len(), metadata["frames"].as_u64().unwrap() as usize);
        assert_eq!(
            f64::from(times[0]),
            metadata["start_seconds"].as_f64().unwrap()
        );
        assert_eq!(
            f64::from(*times.last().unwrap()),
            metadata["last_frame_seconds"].as_f64().unwrap()
        );
        assert_eq!(schedule.metadata(1234, "abc", "Strike"), metadata);
    }

    #[test]
    fn invalid_clip_time_ranges_do_not_start_exports() {
        for (start, seconds, fps) in [
            (f32::NAN, 1.0, 30),
            (-1.0, 1.0, 30),
            (0.0, f32::INFINITY, 30),
            (0.0, 0.0, 30),
            (0.0, 1.0, 0),
            (f32::MAX, f32::MAX, 30),
        ] {
            assert!(ClipSchedule::new(start, seconds, fps).is_err());
        }
        assert_eq!(ClipSchedule::new(0.0, 0.001, 30).unwrap().frame_count(), 1);
    }

    #[test]
    fn entire_recorded_range_includes_final_pose_without_looping() {
        let mut schedule = ClipSchedule::recorded(480, 1.0 / 240.0, 30).unwrap();
        let mut indices = Vec::new();
        while let Some(frame) = schedule.prepare() {
            indices.push(clamped_frame_index(frame.seconds, 1.0 / 240.0, 480));
            schedule.begin_capture().unwrap();
            schedule.complete(frame.index);
        }
        assert_eq!(indices.len(), 61);
        assert_eq!(indices[0], 0);
        assert_eq!(*indices.last().unwrap(), 479);
        assert!(indices.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(
            ClipSchedule::recorded(1, 0.01, 30).unwrap().frame_count(),
            1
        );
        assert_eq!(
            ClipSchedule::recorded(481, 1.0 / 240.0, 30)
                .unwrap()
                .frame_count(),
            61
        );
        assert!(ClipSchedule::recorded(0, 0.01, 30).is_err());
    }
}
