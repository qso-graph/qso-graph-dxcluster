// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared HTTPS adapter and test seam for TUI feeds.

use qso_graph_http::{Client, Clock, Error, Roots, Timeouts};
use std::time::{Duration, Instant, SystemTime};

pub(super) const USER_AGENT: &str = concat!("qg-dxc/", env!("CARGO_PKG_VERSION"));

pub(super) trait Fetch {
    fn get(&mut self, url: &'static str, limit: usize) -> Outcome;
}

pub(super) enum Outcome {
    Bytes(Vec<u8>),
    Status(u16, Option<String>),
    Error(&'static str),
}

pub(super) struct Https<const N: usize> {
    client: Client,
    urls: [&'static str; N],
}

impl<const N: usize> Https<N> {
    pub(super) fn new(host: &str, urls: [&'static str; N]) -> Self {
        let client = Client::new(&[host], Roots::WebPki, USER_AGENT, Timeouts::default())
            .expect("valid TUI feed client settings");
        Self { client, urls }
    }

    #[cfg(test)]
    pub(super) fn allows_host(&self, host: &str) -> bool {
        self.client.allows_host(host)
    }
}

impl<const N: usize> Fetch for Https<N> {
    fn get(&mut self, url: &'static str, limit: usize) -> Outcome {
        // QGDXC-TUI-0112 and QGDXC-TUI-0131: only configured feed URLs can be asked.
        assert!(self.urls.contains(&url));
        match self.client.get_success_body(url, limit) {
            Ok(response) if (200..300).contains(&response.head.status) => {
                Outcome::Bytes(response.body)
            }
            Ok(response) => {
                let retry_after = response
                    .head
                    .retry_after
                    .map(|value| String::from_utf8(value).unwrap_or_default());
                Outcome::Status(response.head.status, retry_after)
            }
            Err(Error::BodyReadFailed(_)) => Outcome::Error("response body failed"),
            Err(Error::BodyTooLarge(_)) => Outcome::Error("byte limit exceeded"),
            Err(_) => Outcome::Error("request failed"),
        }
    }
}

pub(super) struct SystemClock(pub(super) Instant);

impl Clock for SystemClock {
    fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }

    fn unix_time(&self) -> Duration {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
    }
}

#[cfg(test)]
pub(super) mod test_fetch;
