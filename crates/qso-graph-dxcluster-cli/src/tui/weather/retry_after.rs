// SPDX-License-Identifier: GPL-3.0-or-later
//! Interpret Retry-After as delay seconds or an HTTP date (RFC 9110 §10.2.3).

use std::time::Duration;

pub(super) fn delay(header: Option<&str>, now: Duration) -> Duration {
    let seconds = header.and_then(|header| {
        let value = header.trim_matches([' ', '\t']);
        if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) {
            // Saturate even when the server supplies more digits than fit in a u64.
            return Some(value.bytes().fold(0u64, |n, b| {
                n.saturating_mul(10).saturating_add(u64::from(b - b'0'))
            }));
        }
        let when = http_date(value, now.as_secs())?;
        let remaining = Duration::from_secs(when).saturating_sub(now);
        Some(remaining.as_secs() + u64::from(remaining.subsec_nanos() != 0))
    });
    Duration::from_secs(seconds.unwrap_or(60).clamp(60, 3_600))
}

fn http_date(value: &str, now: u64) -> Option<u64> {
    const SHORT_DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const LONG_DAYS: [&str; 7] = [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ];
    let fields: Vec<_> = value.split_ascii_whitespace().take(7).collect();
    let (weekday, day, month, year, clock) = match fields.as_slice() {
        [weekday, day, month, year, clock, "GMT"] => (
            SHORT_DAYS
                .iter()
                .position(|d| Some(*d) == weekday.strip_suffix(','))?,
            decimal(day, 2)?,
            *month,
            decimal(year, 4)?,
            *clock,
        ),
        [weekday, date, clock, "GMT"] => {
            let (day, rest) = date.split_once('-')?;
            let (month, year) = rest.split_once('-')?;
            let short_year = decimal(year, 2)?;
            let current = super::super::render::utc_parts(now).0 as u64;
            let year = current / 100 * 100 + short_year;
            let year = if year > current + 50 {
                year - 100
            } else if year + 50 < current {
                year + 100
            } else {
                year
            };
            (
                LONG_DAYS
                    .iter()
                    .position(|d| Some(*d) == weekday.strip_suffix(','))?,
                decimal(day, 2)?,
                month,
                year,
                *clock,
            )
        }
        [weekday, month, day, clock, year] if (1..=2).contains(&day.len()) => (
            SHORT_DAYS.iter().position(|d| d == weekday)?,
            decimal(day, day.len())?,
            *month,
            decimal(year, 4)?,
            *clock,
        ),
        _ => return None,
    };
    let mut clock = clock.split(':');
    let hour = decimal(clock.next()?, 2)?;
    let minute = decimal(clock.next()?, 2)?;
    let second = decimal(clock.next()?, 2)?;
    if clock.next().is_some() || hour > 23 || minute > 59 || second > 60 {
        return None;
    }

    let month = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .iter()
    .position(|m| *m == month)?;
    let leap = |year: u64| {
        year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
    };
    let months = [
        31,
        28 + u64::from(leap(year)),
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if year == 0 || !(1..=months[month]).contains(&day) {
        return None;
    }
    if year < 1970 {
        return Some(0);
    }
    let leap_days = |year: u64| year / 4 - year / 100 + year / 400;
    let days = 365 * (year - 1970) + leap_days(year - 1) - leap_days(1969)
        + months[..month].iter().sum::<u64>()
        + day
        - 1;
    if (days + 4) % 7 != weekday as u64 {
        return None;
    }
    Some(days * 86_400 + hour * 3600 + minute * 60 + second)
}

fn decimal(text: &str, width: usize) -> Option<u64> {
    if text.len() != width || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}
