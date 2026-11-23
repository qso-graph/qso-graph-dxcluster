// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

#[test]
fn reads_only_ascii_delay_seconds_and_saturates() {
    let now = Duration::ZERO;
    assert_eq!(
        parse_retry_after(b" \t120 ", now),
        Some(Duration::from_secs(120))
    );
    assert_eq!(parse_retry_after(b"0", now), Some(Duration::ZERO));
    assert_eq!(
        parse_retry_after(b"9999999999999999999999999999999", now),
        Some(Duration::from_secs(u64::MAX))
    );
    for value in [
        b"".as_slice(),
        b"+10",
        b"-10",
        b"1.0",
        b"1 2",
        b"1\n",
        &[255],
    ] {
        assert_eq!(parse_retry_after(value, now), None);
    }
}

#[test]
fn reads_all_three_http_date_forms_in_utc() {
    let now = Duration::from_secs(1_795_376_460);
    for header in [
        b"Sun, 22 Nov 2026 19:43:00 GMT".as_slice(),
        b"Sunday, 22-Nov-26 19:43:00 GMT",
        b"Sun Nov 22 19:43:00 2026",
    ] {
        assert_eq!(
            parse_retry_after(header, now),
            Some(Duration::from_secs(120))
        );
        assert_eq!(
            parse_retry_after(header, now + Duration::from_millis(119_500)),
            Some(Duration::from_secs(1))
        );
        assert_eq!(
            parse_retry_after(header, now + Duration::from_secs(121)),
            Some(Duration::ZERO)
        );
    }
    assert_eq!(
        parse_retry_after(b"Sun Nov  6 08:49:37 1994", now),
        Some(Duration::ZERO)
    );
    assert_eq!(
        parse_retry_after(b"Wed, 31 Dec 1969 23:59:59 GMT", now),
        Some(Duration::ZERO)
    );
}

#[test]
fn rfc850_uses_the_full_date_for_the_fifty_year_boundary() {
    let now = Duration::from_secs(1_795_376_460);
    assert_eq!(
        parse_retry_after(b"Sunday, 22-Nov-76 19:41:00 GMT", now),
        Some(Duration::from_secs(1_577_923_200))
    );
    assert_eq!(
        parse_retry_after(b"Monday, 22-Nov-76 19:41:01 GMT", now),
        Some(Duration::ZERO)
    );
}

#[test]
fn malformed_dates_are_absent() {
    let now = Duration::from_secs(1_795_376_460);
    for header in [
        b"Sat, 22 Nov 2026 19:43:00 GMT".as_slice(),
        b"Sun, 31 Nov 2026 19:43:00 GMT",
        b"Sun, 22 Nov 2026 24:43:00 GMT",
        b"Sun, 22 Nov 2026 19:43:00 UTC",
        b"Sun,  22 Nov 2026 19:43:00 GMT",
        b"Sun, 22 Nov 2026 19:43:00 GMT trailing",
        b"Sun Nov 6 08:49:37 1994",
        b"Thu, 31 Dec 1969 23:59:59 GMT",
    ] {
        assert_eq!(parse_retry_after(header, now), None);
    }
}
