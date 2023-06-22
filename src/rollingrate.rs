use std::collections::VecDeque;
use tokio::time::{Duration, Instant};
pub struct RollingRate {
    list: VecDeque<(Instant, u64)>,
    min_elapsed: Duration,
    max_elapsed: Duration,
}

impl RollingRate {
    pub fn new(min_elapsed: Duration, max_elapsed: Duration) -> Self {
        Self {
            list: VecDeque::new(),
            min_elapsed,
            max_elapsed,
        }
    }
    pub fn push(&mut self, item: u64) {
        // guarantee sample values increase from back to front
        if let Some((_, b)) = self.list.back() {
            if item < *b {
                self.reset();
            }
        }
        self.list.push_back((Instant::now(), item));
    }
    pub fn rate(&mut self) -> Option<u64> {
        while let Some((t, b)) = self.list.front() {
            if t.elapsed() > self.max_elapsed {
                // remove old samples
                self.list.pop_front();
            } else {
                // two samples minimum
                if self.list.len() < 2 { return None; }
                // not old enough?
                if t.elapsed() < self.min_elapsed { return None; }
                let (t2, b2) = self.list.back()?;
                let eb = b2 - b;
                // use elapsed time between the oldest and newest samples
                let et = t2.duration_since(*t);
                let r = eb as f64 / et.as_secs_f64();
                return Some(r as u64)
            }
        }
        None
    }
    pub fn reset(&mut self) {
        self.list.clear();
    }
    
}
