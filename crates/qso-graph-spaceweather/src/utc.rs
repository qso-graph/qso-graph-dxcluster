// SPDX-License-Identifier: GPL-3.0-or-later
//! Calendar validation for QGDXC-SWX-0003 and QGDXC-SWX-0009.

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
    if !(2000..=2099).contains(&year) || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    Some(midnight(year, month, day)? + hour * 3600 + minute * 60 + second)
}

pub(super) fn month(name: &str) -> Option<i64> {
    [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .iter()
    .position(|&month| month == name)
    .map(|index| index as i64 + 1)
}

pub(super) fn midnight(year: i64, month: i64, day: i64) -> Option<i64> {
    if !(1..=9999).contains(&year) || !(1..=12).contains(&month) {
        return None;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let months = [
        31,
        28 + i64::from(leap),
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
    let previous_year = year - 1;
    let prior_leaps = previous_year / 4 - previous_year / 100 + previous_year / 400;
    let days = previous_year * 365 + prior_leaps + months[..month].iter().sum::<i64>() + day - 1;
    const UNIX_EPOCH_DAY: i64 = 719_162;
    Some((days - UNIX_EPOCH_DAY) * 86_400)
}

pub(super) fn day(text: &str) -> Option<i64> {
    (text.len() == 2).then(|| digits(text.as_bytes()))?
}

pub(super) fn message_time(text: &str) -> Option<(i64, i64, i64)> {
    let mut fields = text.split_ascii_whitespace();
    let year = fields.next()?.as_bytes();
    if year.len() != 4 {
        return None;
    }
    let year = digits(year)?;
    let month = month(fields.next()?)?;
    let day = day(fields.next()?)?;
    let [h1, h2, m1, m2] = *fields.next()?.as_bytes() else {
        return None;
    };
    let hour = digits(&[h1, h2])?;
    let minute = digits(&[m1, m2])?;
    if hour > 23 || minute > 59 || fields.next()? != "UTC" || fields.next().is_some() {
        return None;
    }
    let time = midnight(year, month, day)? + hour * 3600 + minute * 60;
    Some((time, year, month))
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
