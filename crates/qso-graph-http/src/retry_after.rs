// SPDX-License-Identifier: GPL-3.0-or-later
use std::time::Duration;

/// Parse a Retry-After field as delay-seconds or an HTTP date (QGDXC-HTTP-0005).
/// `now` is UTC time since the Unix epoch. A date in the past gives a zero delay.
pub fn parse_retry_after(header: &[u8], now: Duration) -> Option<Duration> {
    let value = std::str::from_utf8(header).ok()?.trim_matches([' ', '\t']);
    if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) {
        // Saturate even when the server supplies more digits than fit in a u64.
        let seconds = value.bytes().fold(0u64, |n, b| {
            n.saturating_mul(10).saturating_add(u64::from(b - b'0'))
        });
        return Some(Duration::from_secs(seconds));
    }
    let when = http_date(value, now)?;
    // The full final second of a server's wait is preserved by rounding up.
    let remaining = when.saturating_sub(u128::from(now.as_secs()));
    Some(Duration::from_secs(u64::try_from(remaining).ok()?))
}

fn http_date(value: &str, now: Duration) -> Option<u128> {
    const SHORT_DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    const LONG_DAYS: [&str; 7] = [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ];
    let fields: Vec<_> = value.split(' ').take(7).collect();
    let (weekday, day, month, year, clock, short_year) = match fields.as_slice() {
        [weekday, day, month, year, clock, "GMT"] => (
            SHORT_DAYS
                .iter()
                .position(|d| Some(*d) == weekday.strip_suffix(','))?,
            decimal(day, 2)?,
            *month,
            decimal(year, 4)?,
            *clock,
            false,
        ),
        [weekday, date, clock, "GMT"] => {
            let (day, rest) = date.split_once('-')?;
            let (month, year) = rest.split_once('-')?;
            (
                LONG_DAYS
                    .iter()
                    .position(|d| Some(*d) == weekday.strip_suffix(','))?,
                decimal(day, 2)?,
                month,
                decimal(year, 2)?,
                *clock,
                true,
            )
        }
        [weekday, month, day, clock, year] => (
            SHORT_DAYS.iter().position(|d| d == weekday)?,
            decimal(day, 2)?,
            *month,
            decimal(year, 4)?,
            *clock,
            false,
        ),
        [weekday, month, "", day, clock, year] => (
            SHORT_DAYS.iter().position(|d| d == weekday)?,
            decimal(day, 1)?,
            *month,
            decimal(year, 4)?,
            *clock,
            false,
        ),
        _ => return None,
    };
    let (hour, clock) = clock.split_once(':')?;
    let (minute, second) = clock.split_once(':')?;
    let (hour, minute, second) = (decimal(hour, 2)?, decimal(minute, 2)?, decimal(second, 2)?);
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }

    let month = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .iter()
    .position(|m| *m == month)?;
    let seconds = hour * 3600 + minute * 60 + second;
    let year = if short_year {
        let now = now.as_secs();
        let (current_year, current_month, current_day) = utc_date(now / 86_400);
        // RFC 9110 §5.6.7 uses a 50-year window measured from the full UTC date.
        let cutoff = current_year + 50
            - u64::from((month, day, seconds) > (current_month, current_day, now % 86_400));
        cutoff - (cutoff + 100 - year) % 100
    } else {
        year
    };
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let months = [
        31,
        28 + u64::from(leap),
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
    let prior_years = year - 1;
    // Day zero is Monday, January 1 of year 1 in the Gregorian calendar.
    let days = 365 * prior_years + prior_years / 4 - prior_years / 100
        + prior_years / 400
        + months[..month].iter().sum::<u64>()
        + day
        - 1;
    if days % 7 != weekday as u64 {
        return None;
    }
    const UNIX_EPOCH_DAY: u64 = 719_162;
    Some(
        (u128::from(days) * 86_400 + u128::from(seconds))
            .saturating_sub(u128::from(UNIX_EPOCH_DAY) * 86_400),
    )
}

fn decimal(text: &str, width: usize) -> Option<u64> {
    if text.len() != width || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

// Gregorian calendar date for a day on or after the Unix epoch.
fn utc_date(days_since_epoch: u64) -> (u64, usize, u64) {
    let days = days_since_epoch + 719_468;
    let era = days / 146_097;
    let day_of_era = days % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let march_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * march_month + 2) / 5 + 1;
    let month = (march_month + 2) % 12;
    (year + u64::from(month < 2), month as usize, day)
}

#[cfg(test)]
mod tests;
