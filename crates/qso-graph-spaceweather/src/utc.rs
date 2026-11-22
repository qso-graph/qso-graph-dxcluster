// SPDX-License-Identifier: GPL-3.0-or-later
//! Calendar validation for QGDXC-SWX-0003.

fn digits(bytes: &[u8]) -> Option<i64> {
    bytes.iter().try_fold(0, |n, b| match b {
        b'0'..=b'9' => Some(n * 10 + i64::from(b - b'0')),
        _ => None,
    })
}

pub(super) fn datetime(date: &[u8], clock: &[u8]) -> Option<i64> {
    let [y1, y2, y3, y4, b'-', m1, m2, b'-', d1, d2] = *date else {
        return None;
    };
    let [h1, h2, b':', min1, min2, b':', s1, s2] = *clock else {
        return None;
    };
    let year = digits(&[y1, y2, y3, y4])?;
    let month = digits(&[m1, m2])?;
    let day = digits(&[d1, d2])?;
    let hour = digits(&[h1, h2])?;
    let minute = digits(&[min1, min2])?;
    let second = digits(&[s1, s2])?;
    if !(2000..=2099).contains(&year)
        || !(1..=12).contains(&month)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    let months = [
        31,
        28 + i64::from(year % 4 == 0),
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
    let month = month as usize - 1;
    if !(1..=months[month]).contains(&day) {
        return None;
    }
    // The validated range never crosses a non-leap century.
    let prior_years = year - 1970;
    let prior_leaps = (year - 1969) / 4;
    let days = prior_years * 365 + prior_leaps + months[..month].iter().sum::<i64>() + day - 1;
    Some(days * 86_400 + hour * 3600 + minute * 60 + second)
}

pub(super) fn iso(text: &str) -> Option<i64> {
    let bytes = text.strip_suffix('Z').unwrap_or(text).as_bytes();
    if bytes.len() != 19 || bytes[10] != b'T' {
        return None;
    }
    datetime(&bytes[..10], &bytes[11..])
}

pub(super) fn issue(text: &str) -> Option<(i64, i64)> {
    let bytes = text.as_bytes();
    if bytes.len() < 19 || bytes[10] != b' ' {
        return None;
    }
    let time = datetime(&bytes[..10], &bytes[11..19])?;
    let fraction = match &bytes[19..] {
        [] => 0,
        [b'.', rest @ ..] if (1..=6).contains(&rest.len()) => {
            digits(rest)? * 10_i64.pow(6 - rest.len() as u32)
        }
        _ => return None,
    };
    Some((time, fraction))
}
