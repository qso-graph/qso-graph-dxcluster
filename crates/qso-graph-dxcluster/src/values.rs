// SPDX-License-Identifier: GPL-3.0-or-later
pub const MAX_INTEGER: u64 = 9_007_199_254_740_991;
pub fn received_call(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && s.bytes()
            .all(|b| (0x21..=0x7e).contains(&b) && b != b':' && b != b'^')
}
pub fn login(s: &str) -> bool {
    if !(1..=64).contains(&s.len())
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'/' || b == b'-')
        || !s.bytes().any(|b| b.is_ascii_alphabetic())
        || !s.bytes().any(|b| b.is_ascii_digit())
    {
        return false;
    }
    if let Some((_, tail)) = s.rsplit_once('-')
        && (tail.is_empty()
            || tail.starts_with('0')
            || !tail.bytes().all(|b| b.is_ascii_digit())
            || !tail.parse::<u8>().is_ok_and(|v| v <= 99))
    {
        return false;
    }
    true
}
pub fn frequency(s: &str) -> Option<i64> {
    if s.len() > 16 {
        return None;
    }
    let (whole, frac) = s.split_once('.').map_or((s, ""), |(a, b)| (a, b));
    if !(1..=12).contains(&whole.len())
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || frac.len() > 3
        || !frac.bytes().all(|b| b.is_ascii_digit())
        || (s.contains('.') && frac.is_empty())
    {
        return None;
    }
    let n = whole
        .parse::<i64>()
        .ok()?
        .checked_mul(1000)?
        .checked_add(if frac.is_empty() {
            0
        } else {
            frac.parse::<i64>().ok()? * 10i64.pow(3 - frac.len() as u32)
        })?;
    (1..=999_999_999_999_999).contains(&n).then_some(n)
}
pub fn time(s: &str, z: bool) -> Option<String> {
    let s = if z {
        s.strip_suffix('Z')?
    } else {
        s.strip_suffix('Z').unwrap_or(s)
    };
    if s.len() != 4 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let h = s.get(..2)?.parse::<u8>().ok()?;
    let m = s.get(2..)?.parse::<u8>().ok()?;
    (h < 24 && m < 60).then(|| format!("{h:02}:{m:02}"))
}
pub fn month(s: &str) -> Option<u32> {
    [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ]
    .iter()
    .position(|m| s.eq_ignore_ascii_case(m))
    .map(|n| n as u32 + 1)
}
pub fn date(s: &str) -> Option<String> {
    let p: Vec<_> = s.trim_matches(' ').split('-').collect();
    if p.len() != 3
        || p[2].len() != 4
        || !(1..=2).contains(&p[0].len())
        || !p[0].bytes().all(|b| b.is_ascii_digit())
        || !p[2].bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let d = p[0].parse::<u32>().ok()?;
    let m = month(p[1])?;
    let y = p[2].parse::<u32>().ok()?;
    if y == 0 || y > 9999 || d == 0 || d > days(m, y) {
        return None;
    }
    Some(format!("{y:04}-{m:02}-{d:02}"))
}
fn days(m: u32, y: u32) -> u32 {
    match m {
        2 => {
            if y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400)) {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}
pub fn short_date(s: &str) -> bool {
    s.rsplit_once('-').is_some_and(|(d, m)| {
        month(m).is_some_and(|m| d.parse::<u32>().is_ok_and(|d| d > 0 && d <= days(m, 2000)))
    })
}
pub fn grid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 4
        && b[..2]
            .iter()
            .all(|b| (b'A'..=b'R').contains(&b.to_ascii_uppercase()))
        && b[2..].iter().all(u8::is_ascii_digit)
}
pub fn decode(b: &[u8]) -> (String, &'static str) {
    match std::str::from_utf8(b) {
        Ok(s) => (s.to_owned(), "utf8"),
        Err(_) => (b.iter().map(|b| char::from(*b)).collect(), "latin1"),
    }
}
pub fn token(s: &str) -> bool {
    s.len() == 21
        && s.starts_with("QGDXC")
        && s.as_bytes()[5..]
            .iter()
            .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(b))
}
