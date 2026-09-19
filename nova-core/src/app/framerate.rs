use std::time::{Duration, Instant};

use winit::window::Window;

use crate::time::Clock;

#[derive(Clone, Copy)]
pub enum FramerateLimit {
    Auto,
    Fixed(u64),
}

pub(crate) struct FramerateHandler {
    frame_clock: Clock,
    frame_time: Option<Duration>,
    next_render_instant: Instant
}

impl FramerateHandler {
    pub(crate) fn new(framerate_limit: FramerateLimit, window: &Window) -> Self {
        let framerate = match framerate_limit {
            FramerateLimit::Auto => Self::get_window_framerate(window),
            FramerateLimit::Fixed(framerate_limit) =>
                if framerate_limit > 0 { Some(framerate_limit as f64) } else { Self::get_window_framerate(window) }
        };

        Self {
            frame_clock: Clock::new(),
            frame_time: framerate.map(|framerate| Duration::from_secs_f64(1.0 / framerate)),
            next_render_instant: Instant::now(),
        }
    }

    pub(crate) fn elapsed(&mut self) -> (Duration, Duration) {
        let dt = self.frame_clock.restart();
        let frame_time = self.frame_time.unwrap_or(dt);

        (frame_time, dt)   
    }

    pub(crate) fn reset_next_render_instant(&mut self, frame_time: Duration) -> bool {
        if self.next_render_instant <= Instant::now() {
            self.next_render_instant = Instant::now() + frame_time;
            true
        }
        else {
            false
        }
    }

    pub(crate) fn get_next_render_instant(&self) -> Instant {
        self.next_render_instant
    }

    pub(crate) fn update_next_render_instant(&mut self) -> bool {
        if self.next_render_instant <= Instant::now() {
            self.next_render_instant += self.frame_time.unwrap_or(self.frame_clock.elapsed());
            true
        }
        else {
            false
        }
    }

    fn get_window_framerate(window: &Window) -> Option<f64> {
        let framerate = window
            .current_monitor()?
            .refresh_rate_millihertz()? as f64
            / 1000.0 - 0.5;

        Some(framerate)
    }
}

