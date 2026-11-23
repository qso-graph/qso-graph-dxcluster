// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use crate::{Client, Error, Response, ResponseHead, Roots, Timeouts};

const SOURCE: &str = "https://example.invalid/one";
const OTHER: &str = "https://example.invalid/two";

const fn secs(seconds: u64) -> Duration {
    Duration::from_secs(seconds)
}

struct FakeClock(u64);

impl Clock for FakeClock {
    fn elapsed(&self) -> Duration {
        secs(self.0)
    }

    fn unix_time(&self) -> Duration {
        secs(1_795_376_460) + self.elapsed()
    }
}

fn desktop() -> Ledger {
    // QGDXC-HTTP-0006. An unusable Retry-After uses the ordinary failure backoff.
    Ledger::new(Policy {
        floors: [(SOURCE.into(), secs(60)), (OTHER.into(), secs(120))].into(),
        wait_scope: WaitScope::Key,
        retry_after: RetryAfter {
            minimum: Duration::ZERO,
            maximum: secs(86_400),
            default: None,
            server_errors: RetryAfterStatuses::AllServerErrors,
        },
        backoff: Some(Backoff {
            first: secs(60),
            maximum: secs(3_600),
        }),
    })
}

fn noaa() -> Ledger {
    // QGDXC-HTTP-0006.
    Ledger::new(Policy {
        floors: [(SOURCE.into(), secs(900)), (OTHER.into(), secs(300))].into(),
        wait_scope: WaitScope::Provider,
        retry_after: RetryAfter {
            minimum: secs(60),
            maximum: secs(3_600),
            default: Some(secs(60)),
            server_errors: RetryAfterStatuses::ServiceUnavailable,
        },
        backoff: None,
    })
}

fn busy(status: u16, retry_after: Option<&[u8]>) -> Outcome<'_> {
    Outcome::Busy {
        status,
        retry_after,
    }
}

#[test]
fn idle_types_are_send_and_sync() {
    fn assert_traits<T: Send + Sync>() {}
    assert_traits::<Client>();
    assert_traits::<Ledger>();
    assert_traits::<Policy>();
    assert_traits::<Gate>();
    assert_traits::<Outcome<'_>>();
    assert_traits::<Response>();
    assert_traits::<ResponseHead>();
    assert_traits::<Error>();
    assert_traits::<Timeouts>();
    assert_traits::<Roots>();
}

#[test]
fn inside_its_floor_a_source_is_not_asked() {
    let mut ledger = desktop();
    assert_eq!(ledger.gate(SOURCE, &FakeClock(0)), Ok(Gate::Go));
    ledger.record(SOURCE, Outcome::Answered, &FakeClock(20));
    assert_eq!(
        ledger.gate(SOURCE, &FakeClock(30)),
        Ok(Gate::Resting(secs(60)))
    );
    assert_eq!(ledger.gate(OTHER, &FakeClock(30)), Ok(Gate::Go));
    assert_eq!(ledger.gate(SOURCE, &FakeClock(60)), Ok(Gate::Go));
    assert_eq!(
        ledger.gate(OTHER, &FakeClock(60)),
        Ok(Gate::Resting(secs(150)))
    );
    assert_eq!(ledger.gate("unknown", &FakeClock(60)), Err(UnknownKey));
}

#[test]
fn a_429s_retry_after_is_honoured_and_capped_at_a_day() {
    let mut ledger = desktop();
    assert_eq!(ledger.gate(SOURCE, &FakeClock(0)), Ok(Gate::Go));
    assert_eq!(
        ledger.record(SOURCE, busy(429, Some(b"300")), &FakeClock(0)),
        Some(secs(300))
    );
    assert_eq!(
        ledger.gate(SOURCE, &FakeClock(120)),
        Ok(Gate::Resting(secs(300)))
    );
    assert_eq!(ledger.gate(OTHER, &FakeClock(120)), Ok(Gate::Go));
    assert_eq!(ledger.gate(SOURCE, &FakeClock(300)), Ok(Gate::Go));
    assert_eq!(
        ledger.record(SOURCE, busy(429, Some(b"604800")), &FakeClock(300)),
        Some(secs(300 + 86_400))
    );
    assert_eq!(
        ledger.record(OTHER, busy(500, Some(b"5")), &FakeClock(300)),
        Some(secs(305))
    );
}

#[test]
fn failures_back_off_from_a_minute_doubling_to_an_hour() {
    let mut ledger = desktop();
    let mut now = 0;
    for delay in [60, 120, 240, 480, 960, 1_920, 3_600, 3_600, 3_600] {
        assert_eq!(ledger.gate(SOURCE, &FakeClock(now)), Ok(Gate::Go));
        assert_eq!(
            ledger.record(SOURCE, Outcome::Unreachable, &FakeClock(now)),
            Some(secs(now + delay))
        );
        now += delay;
    }
    assert_eq!(ledger.gate(SOURCE, &FakeClock(now)), Ok(Gate::Go));
    ledger.record(SOURCE, Outcome::Answered, &FakeClock(now));
    now += 60;
    assert_eq!(ledger.gate(SOURCE, &FakeClock(now)), Ok(Gate::Go));
    assert_eq!(
        ledger.record(SOURCE, busy(429, None), &FakeClock(now)),
        Some(secs(now + 60))
    );
    assert_eq!(
        ledger.record(SOURCE, busy(429, None), &FakeClock(now + 60)),
        Some(secs(now + 180))
    );
    assert_eq!(
        ledger.record(SOURCE, busy(503, Some(b"invalid")), &FakeClock(now + 180)),
        Some(secs(now + 420))
    );
}

#[test]
fn a_later_answer_never_shortens_a_wait_in_force() {
    let mut ledger = desktop();
    assert_eq!(ledger.gate(SOURCE, &FakeClock(0)), Ok(Gate::Go));
    assert_eq!(ledger.gate(SOURCE, &FakeClock(60)), Ok(Gate::Go));
    let clock = FakeClock(60);
    let block = Some(secs(360));
    assert_eq!(
        ledger.record(SOURCE, busy(429, Some(b"300")), &clock),
        block
    );
    assert_eq!(ledger.record(SOURCE, busy(429, Some(b"60")), &clock), block);
    assert_eq!(ledger.record(SOURCE, Outcome::Unreachable, &clock), block);
    assert_eq!(ledger.record(SOURCE, Outcome::Answered, &clock), block);
    assert_eq!(
        ledger.gate(SOURCE, &FakeClock(120)),
        Ok(Gate::Resting(secs(360)))
    );
    assert_eq!(
        ledger.record(SOURCE, busy(429, Some(b"900")), &FakeClock(120)),
        Some(secs(1_020))
    );
}

#[test]
fn noaa_waits_are_bounded_and_apply_across_keys() {
    for (outcome, delay) in [
        (busy(429, None), 60),
        (busy(429, Some(b"5")), 60),
        (busy(429, Some(b"invalid")), 60),
        (busy(503, Some(b"")), 60),
        (busy(503, Some(b"180")), 180),
        (busy(503, Some(b"3601")), 3_600),
        (busy(503, Some(b"Sun, 22 Nov 2026 19:43:00 GMT")), 120),
    ] {
        let mut ledger = noaa();
        assert_eq!(ledger.gate(SOURCE, &FakeClock(0)), Ok(Gate::Go));
        assert_eq!(ledger.gate(OTHER, &FakeClock(0)), Ok(Gate::Go));
        assert_eq!(
            ledger.record(SOURCE, outcome, &FakeClock(0)),
            Some(secs(delay))
        );
        assert_eq!(
            ledger.record(OTHER, Outcome::Answered, &FakeClock(0)),
            Some(secs(delay))
        );
        assert_eq!(
            ledger.gate(SOURCE, &FakeClock(1)),
            Ok(Gate::Resting(secs(900.max(delay))))
        );
        assert_eq!(
            ledger.gate(OTHER, &FakeClock(1)),
            Ok(Gate::Resting(secs(300.max(delay))))
        );
        assert_eq!(ledger.gate(OTHER, &FakeClock(300.max(delay))), Ok(Gate::Go));
    }
}

#[test]
fn noaa_retries_other_failures_at_the_request_floor() {
    for outcome in [
        busy(503, None),
        busy(500, Some(b"3600")),
        Outcome::Unreachable,
    ] {
        let mut ledger = noaa();
        assert_eq!(ledger.gate(SOURCE, &FakeClock(100)), Ok(Gate::Go));
        assert_eq!(ledger.record(SOURCE, outcome, &FakeClock(110)), None);
        assert_eq!(ledger.gate(OTHER, &FakeClock(110)), Ok(Gate::Go));
        assert_eq!(
            ledger.gate(SOURCE, &FakeClock(999)),
            Ok(Gate::Resting(secs(1_000)))
        );
        assert_eq!(ledger.gate(SOURCE, &FakeClock(1_000)), Ok(Gate::Go));
    }
}
