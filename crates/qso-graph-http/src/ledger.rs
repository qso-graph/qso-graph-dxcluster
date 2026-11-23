// SPDX-License-Identifier: GPL-3.0-or-later
use std::{collections::BTreeMap, fmt, time::Duration};

use crate::parse_retry_after;

/// A monotonic clock and a UTC clock supplied by the application.
pub trait Clock {
    /// Time since a fixed origin for the lifetime of the ledger.
    fn elapsed(&self) -> Duration;

    /// UTC time since the Unix epoch.
    fn unix_time(&self) -> Duration;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaitScope {
    Key,
    Provider,
}

/// Which server errors can impose a wait through a Retry-After header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetryAfterStatuses {
    ServiceUnavailable,
    AllServerErrors,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryAfter {
    pub minimum: Duration,
    pub maximum: Duration,
    /// A fixed fallback for a malformed 429 or 503 header, or a headerless 429
    /// when backoff is off. With no fixed fallback, use backoff or the floor.
    pub default: Option<Duration>,
    /// Both settings honor the header on HTTP 429.
    pub server_errors: RetryAfterStatuses,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Backoff {
    pub first: Duration,
    pub maximum: Duration,
}

/// Scheduling settings for one provider (QGDXC-HTTP-0002).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Policy {
    pub floors: BTreeMap<String, Duration>,
    pub wait_scope: WaitScope,
    pub retry_after: RetryAfter,
    pub backoff: Option<Backoff>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gate {
    Go,
    Resting(Duration),
}

/// The result of a request. Supply HTTP 429 and 5xx responses as busy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome<'a> {
    Answered,
    Busy {
        status: u16,
        retry_after: Option<&'a [u8]>,
    },
    Unreachable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnknownKey;

impl fmt::Display for UnknownKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("request key has no refresh floor")
    }
}

impl std::error::Error for UnknownKey {}

struct Entry {
    floor: Duration,
    refresh_due: Duration,
    wait_until: Option<Duration>,
    next_backoff: Option<Duration>,
}

/// Request state for one provider. Deadlines use the clock's monotonic origin.
pub struct Ledger {
    entries: BTreeMap<String, Entry>,
    wait_scope: WaitScope,
    retry_after: RetryAfter,
    backoff: Option<Backoff>,
    provider_wait: Option<Duration>,
}

impl Ledger {
    pub fn new(policy: Policy) -> Self {
        Self {
            entries: policy
                .floors
                .into_iter()
                .map(|(key, floor)| {
                    let entry = Entry {
                        floor,
                        refresh_due: Duration::ZERO,
                        wait_until: None,
                        next_backoff: None,
                    };
                    (key, entry)
                })
                .collect(),
            wait_scope: policy.wait_scope,
            retry_after: policy.retry_after,
            backoff: policy.backoff,
            provider_wait: None,
        }
    }

    /// The floor deadline, independent of any waits imposed by a response.
    pub fn refresh_due(&self, key: &str) -> Option<Duration> {
        self.entries.get(key).map(|entry| entry.refresh_due)
    }

    /// The last provider-wide wait deadline, which may already have passed.
    pub fn provider_wait(&self) -> Option<Duration> {
        self.provider_wait
    }

    /// Reserve a request at the current time if its key is ready (QGDXC-HTTP-0003).
    pub fn gate(&mut self, key: &str, clock: &impl Clock) -> Result<Gate, UnknownKey> {
        let entry = self.entries.get_mut(key).ok_or(UnknownKey)?;
        let now = clock.elapsed();
        let until = entry
            .refresh_due
            .max(entry.wait_until.unwrap_or_default())
            .max(self.provider_wait.unwrap_or_default());
        if now < until {
            return Ok(Gate::Resting(until));
        }
        entry.refresh_due = now.saturating_add(entry.floor);
        Ok(Gate::Go)
    }

    /// Report an outcome and return the resulting active wait (QGDXC-HTTP-0004).
    pub fn record(
        &mut self,
        key: &str,
        outcome: Outcome<'_>,
        clock: &impl Clock,
    ) -> Option<Duration> {
        let entry = self.entries.get_mut(key)?;
        let now = clock.elapsed();
        let wait = if outcome == Outcome::Answered {
            entry.next_backoff = None;
            None
        } else {
            let backoff = self.backoff.map(|backoff| {
                let wait = entry
                    .next_backoff
                    .unwrap_or(backoff.first.min(backoff.maximum));
                entry.next_backoff = Some(wait.saturating_mul(2).min(backoff.maximum));
                wait
            });
            self.retry_after
                .delay(outcome, clock)
                .or(backoff)
                .or_else(|| {
                    if matches!(outcome, Outcome::Busy { status: 429, .. }) {
                        self.retry_after
                            .default
                            .map(|delay| self.retry_after.bound(delay))
                    } else {
                        None
                    }
                })
        };
        let until = match self.wait_scope {
            WaitScope::Key => &mut entry.wait_until,
            WaitScope::Provider => &mut self.provider_wait,
        };
        *until = (*until).max(wait.map(|delay| now.saturating_add(delay)));
        until.filter(|&deadline| deadline > now)
    }
}

impl RetryAfter {
    fn delay(self, outcome: Outcome<'_>, clock: &impl Clock) -> Option<Duration> {
        let Outcome::Busy {
            status,
            retry_after,
        } = outcome
        else {
            return None;
        };
        let delay = match (status, retry_after) {
            // A headerless 429 uses backoff when the policy enables it.
            (429, None) => return None,
            (429 | 503, Some(header)) => {
                parse_retry_after(header, clock.unix_time()).or(self.default)?
            }
            (500..=599, Some(header))
                if self.server_errors == RetryAfterStatuses::AllServerErrors =>
            {
                parse_retry_after(header, clock.unix_time())?
            }
            _ => return None,
        };
        Some(self.bound(delay))
    }

    fn bound(self, delay: Duration) -> Duration {
        delay.max(self.minimum).min(self.maximum)
    }
}

#[cfg(test)]
mod tests;
