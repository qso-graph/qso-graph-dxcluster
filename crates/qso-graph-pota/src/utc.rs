// SPDX-License-Identifier: GPL-3.0-or-later

pub(super) fn seconds(text: &str) -> Option<i64> {
    let bytes = text.strip_suffix('Z').unwrap_or(text).as_bytes();
    let [y @ .., b'T', h1, h2, b':', m1, m2, b':', s1, s2] = bytes.get(..19)? else {
        return None;
    };
    match &bytes[19..] {
        [] => {}
        [b'.', fraction @ ..]
            if (1..=6).contains(&fraction.len()) && fraction.iter().all(u8::is_ascii_digit) => {}
        _ => return None,
    }
    let [y1, y2, y3, y4, b'-', mo1, mo2, b'-', d1, d2] = *y else {
        return None;
    };
    let digits = |digits: &[u8]| {
        digits.iter().try_fold(0_i64, |n, b| {
            b.is_ascii_digit().then(|| n * 10 + i64::from(b - b'0'))
        })
    };
    let year = digits(&[y1, y2, y3, y4])?;
    let month = digits(&[mo1, mo2])?;
    let day = digits(&[d1, d2])?;
    let hour = digits(&[*h1, *h2])?;
    let minute = digits(&[*m1, *m2])?;
    let second = digits(&[*s1, *s2])?;
    if !(2000..=2099).contains(&year)
        || !(1..=12).contains(&month)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    let month_days = [
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
    let month_index = month as usize - 1;
    if !(1..=month_days[month_index]).contains(&day) {
        return None;
    }
    const DAYS_TO_2000: i64 = 10_957;
    let years = year - 2000;
    let days = DAYS_TO_2000
        + years * 365
        + (years + 3) / 4
        + month_days[..month_index].iter().sum::<i64>()
        + day
        - 1;
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second)
}
