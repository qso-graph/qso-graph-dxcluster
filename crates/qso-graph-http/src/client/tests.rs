// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use std::io::{self, Cursor};
use ureq::{
    config::{AutoHeaderValue, Config},
    unversioned::{
        resolver::{ResolvedSocketAddrs, Resolver},
        transport::{DefaultConnector, NextTimeout},
    },
};

const HOST: &str = "example.invalid";
const AGENT: &str = "qg-test/1.0";

#[test]
fn https_agent_uses_verified_tls_and_requested_settings() {
    let client = Client::new(&[HOST], Roots::WebPki, AGENT, Timeouts::default()).unwrap();
    let config = client.agent.config();
    assert!(config.https_only());
    assert!(config.proxy().is_none());
    assert_eq!(config.max_redirects(), 0);
    assert!(!config.http_status_as_error());
    assert_eq!(config.timeouts().connect, Some(Duration::from_secs(10)));
    assert_eq!(config.timeouts().global, Some(Duration::from_secs(30)));
    assert!(
        matches!(config.user_agent(), AutoHeaderValue::Provided(value) if value.as_str() == AGENT)
    );
    let tls = config.tls_config();
    assert_eq!(tls.provider(), TlsProvider::Rustls);
    assert!(matches!(tls.root_certs(), RootCerts::WebPki));
    assert!(!tls.disable_verification());
    assert!(tls.use_sni());
    let provider = tls.unversioned_rustls_crypto_provider().as_ref().unwrap();
    let aws_lc = rustls::crypto::aws_lc_rs::default_provider();
    let cipher_suites = |provider: &rustls::crypto::CryptoProvider| {
        provider
            .cipher_suites
            .iter()
            .map(|suite| suite.suite())
            .collect::<Vec<_>>()
    };
    assert_eq!(cipher_suites(provider), cipher_suites(&aws_lc));
    assert_eq!(
        rustls::ALL_VERSIONS,
        &[&rustls::version::TLS13, &rustls::version::TLS12]
    );

    let timeouts = Timeouts {
        connect: Duration::from_secs(2),
        overall: Duration::from_secs(3),
    };
    let custom = Client::new(&[HOST], Roots::Der(Vec::new()), AGENT, timeouts).unwrap();
    let config = custom.agent.config();
    assert_eq!(config.timeouts().connect, Some(timeouts.connect));
    assert_eq!(config.timeouts().global, Some(timeouts.overall));
    assert!(
        matches!(config.tls_config().root_certs(), RootCerts::Specific(roots) if roots.is_empty())
    );
    assert!(!config.tls_config().disable_verification());
}

#[test]
fn refuses_user_agent_containing_at_sign() {
    assert!(matches!(
        Client::new(
            &[HOST],
            Roots::WebPki,
            "agent @ contact",
            Timeouts::default()
        ),
        Err(Error::InvalidUserAgent)
    ));
    assert!(matches!(
        Client::new(
            &[HOST],
            Roots::WebPki,
            "agent\ncontact",
            Timeouts::default()
        ),
        Err(Error::InvalidUserAgent)
    ));
}

#[test]
fn refuses_unlisted_hosts_and_non_https_before_transport() {
    #[derive(Debug)]
    struct NoNetwork;
    impl Resolver for NoNetwork {
        fn resolve(
            &self,
            _: &Uri,
            _: &Config,
            _: NextTimeout,
        ) -> Result<ResolvedSocketAddrs, ureq::Error> {
            panic!("a refused request reached name resolution");
        }
    }

    let mut client = Client::new(&[HOST], Roots::WebPki, AGENT, Timeouts::default()).unwrap();
    client.agent = ureq::Agent::with_parts(
        client.agent.config().clone(),
        DefaultConnector::new(),
        NoNetwork,
    );
    for path in [
        "https://elsewhere.invalid/",
        "https://example.invalid.elsewhere.invalid/",
        "https://example.invalid./",
    ] {
        assert!(matches!(client.get(path, 0), Err(Error::HostNotAllowed)));
    }
    assert!(matches!(
        client.get("http://example.invalid/", 0),
        Err(Error::HttpsRequired)
    ));
    assert!(matches!(
        client.get("https://local-user@/", 0),
        Err(Error::InvalidUrl)
    ));
    assert!(client.request_uri("https://EXAMPLE.INVALID:443/").is_ok());
}

#[test]
fn response_keeps_status_raw_header_and_body() {
    let body = ureq::Body::builder().data(b"busy");
    let retry_after = HeaderValue::from_bytes(&[255]).unwrap();
    let response = ureq::http::Response::builder()
        .status(503)
        .header(header::RETRY_AFTER, retry_after)
        .body(body)
        .unwrap();
    let response = read_response(response, 4, |_| true).unwrap();
    assert_eq!(response.head.status, 503);
    assert_eq!(response.head.retry_after.as_deref(), Some([255].as_slice()));
    assert_eq!(response.body, b"busy");
}

#[test]
fn retry_after_can_be_returned_without_reading_an_error_body() {
    struct UnreadableBody;
    impl Read for UnreadableBody {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            panic!("an error body was read");
        }
    }

    let response = ureq::http::Response::builder()
        .status(429)
        .header(header::RETRY_AFTER, "120")
        .body(ureq::Body::builder().reader(UnreadableBody))
        .unwrap();
    let response = read_response(response, 4, |status| (200..300).contains(&status)).unwrap();
    assert_eq!(response.head.status, 429);
    assert_eq!(
        response.head.retry_after.as_deref(),
        Some(b"120".as_slice())
    );
    assert!(response.body.is_empty());
}

#[test]
fn body_limit_stops_at_one_extra_byte() {
    let head = || ResponseHead {
        status: 200,
        retry_after: None,
    };
    let mut source = Cursor::new(b"abcdef");
    assert!(matches!(
        bounded_body(head(), &mut source, 3),
        Err(Error::BodyTooLarge(_))
    ));
    assert_eq!(source.position(), 4);
    assert_eq!(
        bounded_body(head(), b"abc".as_slice(), 3).unwrap().body,
        b"abc"
    );
    assert!(matches!(
        bounded_body(head(), b"a".as_slice(), 0),
        Err(Error::BodyTooLarge(_))
    ));
    assert!(
        bounded_body(head(), b"".as_slice(), 0)
            .unwrap()
            .body
            .is_empty()
    );
    assert_eq!(
        bounded_body(head(), b"a".as_slice(), usize::MAX)
            .unwrap()
            .body,
        b"a"
    );
}

#[test]
fn body_errors_keep_the_status_and_keep_response_text_out_of_diagnostics() {
    struct BrokenReader;
    impl Read for BrokenReader {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("response content"))
        }
    }
    let head = ResponseHead {
        status: 429,
        retry_after: Some(b"header content".to_vec()),
    };
    let Err(error) = bounded_body(head, BrokenReader, 20) else {
        panic!("the body read must fail");
    };
    assert_eq!(error.to_string(), "response body failed");
    let debug = format!("{error:?}");
    assert!(!debug.contains("content"));
    assert_eq!(
        error,
        Error::BodyReadFailed(ResponseHead {
            status: 429,
            retry_after: Some(b"header content".to_vec())
        })
    );
}
