// SPDX-License-Identifier: GPL-3.0-or-later
//! SWPC transport and refresh scheduling (QGDXC-TUI-0111–0113, QGDXC-TUI-0116).

use qso_graph_http::{
    Client, Clock, Error as HttpError, Gate, Ledger, Outcome as RequestOutcome, Policy,
    ResponseHead, RetryAfter, RetryAfterStatuses, Roots, Timeouts, WaitScope,
};
use qso_graph_spaceweather::{self as swx, Product};
use std::{
    io,
    sync::mpsc,
    time::{Duration, Instant, SystemTime},
};

#[derive(Clone, Copy)]
pub(super) struct Feed {
    pub product: Product,
    pub label: &'static str,
    pub url: &'static str,
    pub floor: Duration,
}

const FIFTEEN_MINUTES: Duration = Duration::from_secs(900);
const FIVE_MINUTES: Duration = Duration::from_secs(300);
const FAILURE_NOTICE: Duration = Duration::from_secs(15);
const NOAA_HOST: &str = "services.swpc.noaa.gov";
const USER_AGENT: &str = concat!("qg-dxc/", env!("CARGO_PKG_VERSION"));

pub(super) const FEEDS: [Feed; 8] = [
    Feed {
        product: Product::SolarFlux,
        label: "SFI",
        url: "https://services.swpc.noaa.gov/products/summary/10cm-flux.json",
        floor: FIFTEEN_MINUTES,
    },
    Feed {
        product: Product::PlanetaryK,
        label: "Kp",
        url: "https://services.swpc.noaa.gov/products/noaa-planetary-k-index.json",
        floor: FIFTEEN_MINUTES,
    },
    Feed {
        product: Product::Scales,
        label: "R/S/G",
        url: "https://services.swpc.noaa.gov/products/noaa-scales.json",
        floor: FIFTEEN_MINUTES,
    },
    Feed {
        product: Product::Xrays,
        label: "X-ray",
        url: "https://services.swpc.noaa.gov/json/goes/primary/xrays-6-hour.json",
        floor: FIVE_MINUTES,
    },
    Feed {
        product: Product::WindSpeed,
        label: "Wind",
        url: "https://services.swpc.noaa.gov/products/summary/solar-wind-speed.json",
        floor: FIVE_MINUTES,
    },
    Feed {
        product: Product::WindField,
        label: "Bt/Bz",
        url: "https://services.swpc.noaa.gov/products/summary/solar-wind-mag-field.json",
        floor: FIVE_MINUTES,
    },
    Feed {
        product: Product::Alerts,
        label: "Alerts",
        url: "https://services.swpc.noaa.gov/products/alerts.json",
        floor: FIVE_MINUTES,
    },
    Feed {
        product: Product::SolarFluxHistory,
        label: "Flux history",
        url: "https://services.swpc.noaa.gov/json/f107_cm_flux.json",
        floor: Duration::from_secs(3_600),
    },
];
pub(super) const FLUX_HISTORY: usize = FEEDS.len() - 1;
pub(super) const ALERTS: usize = FLUX_HISTORY - 1;

pub(super) enum Data {
    SolarFlux(swx::SolarFlux),
    SolarFluxHistory(Vec<swx::SolarFluxHistory>),
    PlanetaryK(Vec<swx::PlanetaryK>),
    Scales(swx::Scales),
    Xrays(Vec<swx::Xray>),
    WindSpeed(swx::WindSpeed),
    WindField(swx::WindField),
    Alerts {
        values: Vec<swx::Alert>,
        groups: Vec<usize>,
    },
}

impl Data {
    fn parse(product: Product, bytes: &[u8]) -> Result<Self, swx::Error> {
        match product {
            Product::SolarFlux => swx::solar_flux(bytes).map(Self::SolarFlux),
            Product::SolarFluxHistory => swx::solar_flux_history(bytes).map(Self::SolarFluxHistory),
            Product::PlanetaryK => swx::planetary_k(bytes).map(Self::PlanetaryK),
            Product::Scales => swx::scales(bytes).map(Self::Scales),
            Product::Xrays => swx::xrays(bytes).map(Self::Xrays),
            Product::WindSpeed => swx::wind_speed(bytes).map(Self::WindSpeed),
            Product::WindField => swx::wind_field(bytes).map(Self::WindField),
            Product::Alerts => {
                // Keep the grouping identity even if two IDs redact to the same text.
                let mut ids = std::collections::HashMap::new();
                let mut values = swx::alerts(bytes)?;
                let mut groups = Vec::with_capacity(values.len());
                for alert in &mut values {
                    let next = ids.len();
                    groups.push(*ids.entry(alert.product_id.clone()).or_insert(next));
                    redact(&mut alert.product_id);
                    if let Some(headline) = &mut alert.headline {
                        redact(headline);
                    }
                }
                Ok(Self::Alerts { values, groups })
            }
        }
    }
}

// QGDXC-SEC-0005: redact before alerts enter the update channel. The
// replacement preserves UTF-8 because the matched address bytes are ASCII.
fn redact(text: &mut String) {
    let (bytes, changed) = qso_graph_dxcluster::privacy::redact(text.as_bytes());
    if changed {
        *text = String::from_utf8(bytes).expect("redaction preserves UTF-8");
    }
}

pub(super) trait Fetch {
    fn get(&mut self, url: &'static str, limit: usize) -> Outcome;
}

pub(super) enum Outcome {
    Bytes(Vec<u8>),
    Status(u16, Option<String>),
    Error(&'static str),
}

fn https() -> Client {
    Client::new(&[NOAA_HOST], Roots::WebPki, USER_AGENT, Timeouts::default())
        .expect("valid space-weather client settings")
}

fn status(head: ResponseHead) -> Outcome {
    let retry_after = head
        .retry_after
        .map(|value| String::from_utf8(value).unwrap_or_default());
    Outcome::Status(head.status, retry_after)
}

impl Fetch for Client {
    fn get(&mut self, url: &'static str, limit: usize) -> Outcome {
        // QGDXC-TUI-0112: only the fixed feed table can supply a request URI.
        assert!(FEEDS.iter().any(|feed| feed.url == url));
        match self.get_success_body(url, limit) {
            Ok(response) if (200..300).contains(&response.head.status) => {
                Outcome::Bytes(response.body)
            }
            Ok(response) => status(response.head),
            Err(HttpError::BodyReadFailed(_)) => Outcome::Error("response body failed"),
            Err(HttpError::BodyTooLarge(_)) => Outcome::Error("byte limit exceeded"),
            Err(_) => Outcome::Error("request failed"),
        }
    }
}

struct WeatherClock(Instant);

impl Clock for WeatherClock {
    fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }

    fn unix_time(&self) -> Duration {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
    }
}

// QGDXC-TUI-0111, QGDXC-TUI-0113, QGDXC-TUI-0123.
fn noaa_policy() -> Policy {
    Policy {
        floors: FEEDS
            .iter()
            .map(|feed| (feed.url.into(), feed.floor))
            .collect(),
        wait_scope: WaitScope::Provider,
        retry_after: RetryAfter {
            minimum: Duration::from_secs(60),
            maximum: Duration::from_secs(3_600),
            default: Some(Duration::from_secs(60)),
            server_errors: RetryAfterStatuses::ServiceUnavailable,
        },
        backoff: None,
    }
}

pub(super) struct Scheduler {
    ledger: Ledger,
    history_activated_at: Option<Duration>,
}

impl Default for Scheduler {
    fn default() -> Self {
        Self {
            ledger: Ledger::new(noaa_policy()),
            history_activated_at: None,
        }
    }
}

pub(super) struct Update {
    index: usize,
    complete: Duration,
    next: Duration,
    pause: Duration,
    result: Result<Data, String>,
}

impl Update {
    pub(super) fn completed_at(&self) -> Duration {
        self.complete
    }

    pub(super) fn data(&self) -> Option<&Data> {
        self.result.as_ref().ok()
    }
}

impl Scheduler {
    // QGDXC-TUI-0123: activation is a one-way transition for this session.
    fn activate_history(&mut self, now: Duration) {
        self.history_activated_at.get_or_insert(now);
    }

    fn earliest(&self) -> (usize, Duration) {
        let (index, due) = FEEDS
            .iter()
            .enumerate()
            .filter_map(|(index, feed)| {
                let active_since = if index == FLUX_HISTORY {
                    self.history_activated_at?
                } else {
                    Duration::ZERO
                };
                let due = self.ledger.refresh_due(feed.url).expect("feed has a floor");
                Some((index, due.max(active_since)))
            })
            .min_by_key(|&(_, due)| due)
            .expect("feed table is nonempty");
        (
            index,
            due.max(self.ledger.provider_wait().unwrap_or_default()),
        )
    }

    pub(super) fn step(&mut self, fetcher: &mut impl Fetch, clock: &impl Clock) -> Option<Update> {
        let (index, next) = self.earliest();
        if clock.elapsed() < next {
            return None;
        }
        let feed = FEEDS[index];
        if let Gate::Resting(_) = self.ledger.gate(feed.url, clock).expect("feed has a floor") {
            return None;
        }
        let outcome = fetcher.get(feed.url, feed.product.limit());
        let complete = clock.elapsed();
        let request_outcome = match &outcome {
            Outcome::Status(code @ (429 | 500..=599), retry_after) => RequestOutcome::Busy {
                status: *code,
                retry_after: retry_after.as_deref().map(str::as_bytes),
            },
            Outcome::Error(_) => RequestOutcome::Unreachable,
            _ => RequestOutcome::Answered,
        };
        self.ledger.record(feed.url, request_outcome, clock);

        let result = match outcome {
            Outcome::Bytes(bytes) => {
                Data::parse(feed.product, &bytes).map_err(|e| e.reason.to_string())
            }
            Outcome::Status(code, _) => Err(format!("HTTP {code}")),
            Outcome::Error(why) => Err(why.to_owned()),
        };
        Some(Update {
            index,
            complete,
            next: self.earliest().1,
            pause: self.ledger.provider_wait().unwrap_or_default(),
            result,
        })
    }
}

#[derive(Default)]
pub(super) struct Sample {
    pub good: Option<(Data, Duration)>,
    failure: Option<(String, Duration)>,
}

impl Sample {
    pub(super) fn stale(&self, feed: Feed, now: Duration) -> bool {
        self.good
            .as_ref()
            .is_some_and(|(_, at)| now.saturating_sub(*at) > feed.floor * 3)
    }

    pub(super) fn failure(&self, now: Duration) -> Option<&str> {
        let (reason, until) = self.failure.as_ref()?;
        (now < *until).then_some(reason.as_str())
    }
}

#[derive(Default)]
pub(super) struct Cache {
    pub samples: [Sample; FEEDS.len()],
    pub next: Duration,
    pub pause: Duration,
}

impl Cache {
    pub(super) fn sample(&self, product: Product) -> (Feed, &Sample) {
        FEEDS
            .into_iter()
            .zip(&self.samples)
            .find(|(feed, _)| feed.product == product)
            .expect("every product has a feed")
    }

    pub(super) fn apply(&mut self, update: Update) {
        self.next = update.next;
        self.pause = update.pause;
        let sample = &mut self.samples[update.index];
        match update.result {
            Ok(data) => {
                sample.good = Some((data, update.complete));
                sample.failure = None;
            }
            Err(why) => sample.failure = Some((why, update.complete + FAILURE_NOTICE)),
        }
    }
}

pub(super) struct Running {
    control: mpsc::Sender<()>,
    history_requested: bool,
    updates: mpsc::Receiver<Update>,
}

#[derive(Default)]
pub(super) struct Worker(Option<Running>);

impl Worker {
    pub(super) fn poll(
        &mut self,
        visible: bool,
        flux_chart_drawn: bool,
        mut apply: impl FnMut(Update),
        start: impl FnOnce() -> io::Result<Running>,
    ) -> io::Result<()> {
        if visible && self.0.is_none() {
            self.0 = Some(start()?);
        }
        if let Some(running) = &mut self.0 {
            if flux_chart_drawn && !running.history_requested {
                let _ = running.control.send(());
                running.history_requested = true;
            }
            for update in running.updates.try_iter() {
                apply(update);
            }
        }
        Ok(())
    }
}

pub(super) fn start(origin: Instant) -> io::Result<Running> {
    let (send, updates) = mpsc::sync_channel(FEEDS.len());
    let (control, commands) = mpsc::channel();
    std::thread::Builder::new()
        .name("space-weather".into())
        .spawn(move || {
            let clock = WeatherClock(origin);
            let mut fetcher = https();
            let mut scheduler = Scheduler::default();
            // Dropping Running wakes an idle worker. An in-flight request can finish
            // on its own deadline without delaying terminal restoration on exit.
            loop {
                let wait = scheduler.earliest().1.saturating_sub(origin.elapsed());
                match commands.recv_timeout(wait) {
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Ok(()) => scheduler.activate_history(origin.elapsed()),
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
                if let Some(update) = scheduler.step(&mut fetcher, &clock)
                    && send.send(update).is_err()
                {
                    break;
                }
            }
        })?;
    Ok(Running {
        control,
        history_requested: false,
        updates,
    })
}

#[cfg(test)]
mod tests;
