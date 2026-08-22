use std::time::{Duration, Instant};

pub struct Clock {
    instant: Instant
}

impl Clock {
    pub fn new() -> Self {
        Self {
            instant: Instant::now()
        }
    }

    pub fn restart(&mut self) -> Duration {
        let elapsed = self.elapsed();
        self.instant = Instant::now();
        elapsed
    }

    pub fn elapsed(&self) -> Duration {
        Instant::now() - self.instant
    }
}