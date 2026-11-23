// SPDX-License-Identifier: GPL-3.0-or-later
//! POTA polling and spot mapping (QGDXC-TUI-0130–0135).

use super::{
    sources::{Envelope, Message},
    web::{Fetch, Https, Outcome, SystemClock},
};
use qso_graph_dxcluster::{Event, Profile, Value, privacy};
use qso_graph_http::{
    Backoff, Clock, Gate, Ledger, Outcome as RequestOutcome, Policy, RetryAfter,
    RetryAfterStatuses, WaitScope,
};
use qso_graph_pota::{MAX_BYTES, Spot, parse_activator_spots};
use std::{
    collections::{HashSet, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
    },
    thread,
    time::{Duration, Instant},
};

const HOST: &str = "api.pota.app";
const URL: &str = "https://api.pota.app/spot/activator";
const MINUTE: Duration = Duration::from_secs(60);
const SEEN_LIMIT: usize = 10_000;

fn policy() -> Policy {
    Policy {
        floors: [(URL.into(), MINUTE)].into(),
        wait_scope: WaitScope::Provider,
        retry_after: RetryAfter {
            minimum: MINUTE,
            maximum: Duration::from_secs(3_600),
            default: Some(MINUTE),
            server_errors: RetryAfterStatuses::ServiceUnavailable,
        },
        backoff: Some(Backoff {
            first: MINUTE,
            maximum: Duration::from_secs(900),
        }),
    }
}

#[derive(Default)]
struct Seen {
    ids: HashSet<u64>,
    order: VecDeque<u64>,
}

impl Seen {
    fn insert(&mut self, id: u64) -> bool {
        if !self.ids.insert(id) {
            return false;
        }
        self.order.push_back(id);
        if self.order.len() > SEEN_LIMIT
            && let Some(oldest) = self.order.pop_front()
        {
            self.ids.remove(&oldest);
        }
        true
    }
}

struct Scheduler {
    ledger: Ledger,
    seen: Seen,
}

impl Default for Scheduler {
    fn default() -> Self {
        Self {
            ledger: Ledger::new(policy()),
            seen: Seen::default(),
        }
    }
}

pub(crate) enum Status {
    Ready {
        skipped: usize,
    },
    Failed {
        reason: String,
        wait: Option<Duration>,
    },
}

struct Poll {
    status: Status,
    spots: Vec<Event>,
}

impl Poll {
    fn send(self, tx: &SyncSender<Envelope>) -> bool {
        let mut messages = std::iter::once(Message::PotaStatus(self.status))
            .chain(self.spots.into_iter().map(Message::Event));
        messages.all(|message| tx.send(Envelope { order: 0, message }).is_ok())
    }
}

impl Scheduler {
    fn next(&self) -> Duration {
        self.ledger
            .refresh_due(URL)
            .expect("POTA URL has a floor")
            .max(self.ledger.provider_wait().unwrap_or_default())
    }

    fn step(&mut self, fetcher: &mut impl Fetch, clock: &impl Clock) -> Option<Poll> {
        if self.ledger.gate(URL, clock).expect("POTA URL has a floor") != Gate::Go {
            return None;
        }

        let outcome = fetcher.get(URL, MAX_BYTES);
        let (result, request) = match &outcome {
            Outcome::Bytes(bytes) => match parse_activator_spots(bytes) {
                Ok(spots) => (Ok(spots), RequestOutcome::Answered),
                Err(error) => (Err(error.to_string()), RequestOutcome::Unreachable),
            },
            Outcome::Status(code, retry_after) => {
                let request = match code {
                    429 | 500..=599 => RequestOutcome::Busy {
                        status: *code,
                        // POTA uses the fixed fallback even after earlier failures.
                        retry_after: retry_after
                            .as_deref()
                            .map(str::as_bytes)
                            .or_else(|| (*code == 429).then_some(b"".as_slice())),
                    },
                    _ => RequestOutcome::Unreachable,
                };
                (Err(format!("HTTP {code}")), request)
            }
            Outcome::Error(reason) => (Err((*reason).into()), RequestOutcome::Unreachable),
        };
        let wait = self.ledger.record(URL, request, clock);
        Some(match result {
            Ok(spots) => Poll {
                status: Status::Ready {
                    skipped: spots.skipped,
                },
                spots: spots
                    .spots
                    .into_iter()
                    .filter(|spot| self.seen.insert(spot.spot_id))
                    .map(event)
                    .collect(),
            },
            Err(reason) => Poll {
                status: Status::Failed { reason, wait },
                spots: Vec::new(),
            },
        })
    }
}

fn event(spot: Spot) -> Event {
    let mut event = Event::new(Profile::Pota, "spot");
    event.set(
        "spot_kind",
        if spot.is_skimmer() {
            "skimmer"
        } else {
            "human"
        },
    );
    event.set("call", spot.activator);
    event.set("spotter_call", spot.spotter);
    event.set("frequency_hz", Value::Integer(spot.frequency_hz as i64));
    let minute = spot.spot_time / 60;
    event.set(
        "time_of_day",
        format!("{:02}:{:02}", minute / 60 % 24, minute % 60),
    );
    event.set("pota_reference", spot.reference);
    event.set("comment", spot.comments.unwrap_or_default());
    if let Some(mode) = spot.mode {
        event.set("mode", mode);
    }
    if let Some(grid) = spot.grid4 {
        event.set("grid", grid);
    }
    privacy::apply(&mut event, false);
    event
}

pub(super) fn run(origin: Instant, stop: &AtomicBool, tx: &SyncSender<Envelope>) {
    let clock = SystemClock(origin);
    let mut fetcher = Https::new(HOST, [URL]);
    let mut scheduler = Scheduler::default();
    while !stop.load(Ordering::Relaxed) {
        if let Some(poll) = scheduler.step(&mut fetcher, &clock)
            && !poll.send(tx)
        {
            break;
        }
        thread::park_timeout(scheduler.next().saturating_sub(clock.elapsed()));
    }
}

#[cfg(test)]
mod tests;
