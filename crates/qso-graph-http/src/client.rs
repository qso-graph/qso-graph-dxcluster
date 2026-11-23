// SPDX-License-Identifier: GPL-3.0-or-later
use std::{fmt, io::Read, sync::Arc, time::Duration};

use ureq::{
    http::{HeaderValue, Uri, header},
    tls::{Certificate, RootCerts, TlsConfig, TlsProvider},
};

/// Trust roots for server certificate verification (QGDXC-HTTP-0010).
pub enum Roots {
    WebPki,
    /// DER-encoded root certificates supplied by the application.
    Der(Vec<Vec<u8>>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Timeouts {
    pub connect: Duration,
    pub overall: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(10),
            overall: Duration::from_secs(30),
        }
    }
}

pub struct Response {
    pub head: ResponseHead,
    pub body: Vec<u8>,
}

#[derive(Eq, PartialEq)]
pub struct ResponseHead {
    pub status: u16,
    /// The field value as received, including non-UTF-8 bytes if present.
    pub retry_after: Option<Vec<u8>>,
}

impl fmt::Debug for ResponseHead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResponseHead")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    InvalidUserAgent,
    InvalidUrl,
    HttpsRequired,
    HostNotAllowed,
    RequestFailed,
    BodyReadFailed(ResponseHead),
    BodyTooLarge(ResponseHead),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidUserAgent => "invalid User-Agent",
            Self::InvalidUrl => "invalid request URL",
            Self::HttpsRequired => "request URL must use HTTPS",
            Self::HostNotAllowed => "request host is not allowed",
            Self::RequestFailed => "request failed",
            Self::BodyReadFailed(_) => "response body failed",
            Self::BodyTooLarge(_) => "byte limit exceeded",
        })
    }
}

impl std::error::Error for Error {}

/// A synchronous HTTPS client for an explicit set of hosts.
pub struct Client {
    allowed_hosts: Vec<String>,
    agent: ureq::Agent,
}

impl Client {
    pub fn new(
        allowed_hosts: &[&str],
        roots: Roots,
        user_agent: &str,
        timeouts: Timeouts,
    ) -> Result<Self, Error> {
        if user_agent.contains('@') || HeaderValue::from_str(user_agent).is_err() {
            return Err(Error::InvalidUserAgent);
        }
        let roots = match roots {
            Roots::WebPki => RootCerts::WebPki,
            Roots::Der(certs) => RootCerts::from(
                certs
                    .iter()
                    .map(|der| Certificate::from_der(der).to_owned()),
            ),
        };
        let tls = TlsConfig::builder()
            .provider(TlsProvider::Rustls)
            .root_certs(roots)
            .unversioned_rustls_crypto_provider(Arc::new(
                rustls::crypto::aws_lc_rs::default_provider(),
            ))
            .build();
        let agent = ureq::Agent::config_builder()
            .tls_config(tls)
            .https_only(true)
            .proxy(None)
            .max_redirects(0)
            .http_status_as_error(false)
            .timeout_connect(Some(timeouts.connect))
            .timeout_global(Some(timeouts.overall))
            .user_agent(user_agent)
            .build()
            .new_agent();
        Ok(Self {
            allowed_hosts: allowed_hosts
                .iter()
                .map(|host| (*host).to_owned())
                .collect(),
            agent,
        })
    }

    /// Check whether a host name matches an allowlist entry.
    pub fn allows_host(&self, host: &str) -> bool {
        self.allowed_hosts
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(host))
    }

    /// Send a request and read a bounded response (QGDXC-HTTP-0010–0012).
    pub fn get(&self, url: &str, limit: usize) -> Result<Response, Error> {
        self.get_with_body(url, limit, |_| true)
    }

    /// Read a bounded body on success. For other statuses, return the headers
    /// immediately with an empty body.
    pub fn get_success_body(&self, url: &str, limit: usize) -> Result<Response, Error> {
        self.get_with_body(url, limit, |status| (200..300).contains(&status))
    }

    fn get_with_body(
        &self,
        url: &str,
        limit: usize,
        read_body: impl FnOnce(u16) -> bool,
    ) -> Result<Response, Error> {
        let uri = self.request_uri(url)?;
        let response = self
            .agent
            .get(uri)
            .call()
            .map_err(|_| Error::RequestFailed)?;
        read_response(response, limit, read_body)
    }

    fn request_uri(&self, url: &str) -> Result<Uri, Error> {
        let uri: Uri = url.parse().map_err(|_| Error::InvalidUrl)?;
        if uri.scheme() != Some(&ureq::http::uri::Scheme::HTTPS) {
            return Err(Error::HttpsRequired);
        }
        let authority = uri.authority().ok_or(Error::InvalidUrl)?;
        if authority.as_str().contains('@') {
            return Err(Error::InvalidUrl);
        }
        if !self.allows_host(authority.host()) {
            return Err(Error::HostNotAllowed);
        }
        Ok(uri)
    }
}

fn read_response(
    mut response: ureq::http::Response<ureq::Body>,
    limit: usize,
    read_body: impl FnOnce(u16) -> bool,
) -> Result<Response, Error> {
    let head = ResponseHead {
        status: response.status().as_u16(),
        retry_after: response
            .headers()
            .get(header::RETRY_AFTER)
            .map(|value| value.as_bytes().to_vec()),
    };
    if read_body(head.status) {
        bounded_body(head, response.body_mut().as_reader(), limit)
    } else {
        Ok(Response {
            head,
            body: Vec::new(),
        })
    }
}

fn bounded_body(head: ResponseHead, reader: impl Read, limit: usize) -> Result<Response, Error> {
    let mut body = Vec::new();
    if reader
        .take((limit as u64).saturating_add(1))
        .read_to_end(&mut body)
        .is_err()
    {
        return Err(Error::BodyReadFailed(head));
    }
    if body.len() > limit {
        return Err(Error::BodyTooLarge(head));
    }
    Ok(Response { head, body })
}

#[cfg(test)]
mod tests;
