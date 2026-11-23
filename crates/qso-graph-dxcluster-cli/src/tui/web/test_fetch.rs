// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use std::{cell::Cell, collections::VecDeque};

#[derive(Default)]
pub(in crate::tui) struct FakeClock(Cell<Duration>);

impl FakeClock {
    pub(in crate::tui) fn at(&self, seconds: u64) {
        self.0.set(Duration::from_secs(seconds));
    }
}

impl Clock for FakeClock {
    fn elapsed(&self) -> Duration {
        self.0.get()
    }

    fn unix_time(&self) -> Duration {
        Duration::from_secs(1_795_376_460) + self.elapsed()
    }
}

#[derive(Default)]
pub(in crate::tui) struct Script {
    pub(in crate::tui) responses: VecDeque<Outcome>,
    pub(in crate::tui) calls: Vec<(&'static str, usize)>,
}

impl Fetch for Script {
    fn get(&mut self, url: &'static str, limit: usize) -> Outcome {
        self.calls.push((url, limit));
        self.responses.pop_front().expect("unexpected request")
    }
}
