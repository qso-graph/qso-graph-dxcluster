// SPDX-License-Identifier: GPL-3.0-or-later
//! Parse Parks on the Air activator snapshots (QGDXC-POTA-0001–0004).
#![forbid(unsafe_code)]

mod json;
mod number;
mod utc;

use std::fmt;

/// Maximum size of an activator snapshot in bytes.
pub const MAX_BYTES: usize = 1_048_576;
const MAX_ENTRIES: usize = 1_000;

/// A refusal of the whole snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    TooLarge,
    Utf8,
    Json,
    Depth,
    DuplicateMember,
    NotArray,
    TooManyEntries,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::TooLarge => "byte limit exceeded",
            Self::Utf8 => "invalid UTF-8",
            Self::Json => "invalid JSON",
            Self::Depth => "JSON depth limit exceeded",
            Self::DuplicateMember => "duplicate JSON member",
            Self::NotArray => "expected a JSON array",
            Self::TooManyEntries => "entry limit exceeded",
        })
    }
}

impl std::error::Error for Error {}

/// Valid spots in snapshot order and the number of invalid entries skipped.
#[derive(Clone, Debug, PartialEq)]
pub struct Spots {
    pub spots: Vec<Spot>,
    pub skipped: usize,
}

/// The typed fields of one activator spot.
#[derive(Clone, Debug, PartialEq)]
pub struct Spot {
    pub spot_id: u64,
    pub activator: String,
    pub spotter: String,
    pub frequency_hz: u64,
    pub reference: String,
    /// Whole UTC seconds since the Unix epoch.
    pub spot_time: i64,
    pub mode: Option<String>,
    pub name: Option<String>,
    pub comments: Option<String>,
    pub source: Option<String>,
    pub location_desc: Option<String>,
    pub grid4: Option<String>,
    pub count: Option<u64>,
}

impl Spot {
    /// Classify the spot by the reported source (QGDXC-POTA-0004).
    pub fn is_skimmer(&self) -> bool {
        self.source
            .as_deref()
            .is_some_and(|s| s.eq_ignore_ascii_case("RBN"))
    }
}

/// Parse a snapshot, retaining valid entries and counting invalid entries.
pub fn parse_activator_spots(bytes: &[u8]) -> Result<Spots, Error> {
    if bytes.len() > MAX_BYTES {
        return Err(Error::TooLarge);
    }
    let json = json::read(bytes)?;
    let mut spots = Vec::with_capacity(json.len());
    let mut skipped = 0;
    for value in json {
        if let Some(spot) = spot(value) {
            spots.push(spot);
        } else {
            skipped += 1;
        }
    }
    Ok(Spots { spots, skipped })
}

fn spot(value: json::Entry<'_>) -> Option<Spot> {
    let json = value?;
    let text = |key, max| {
        let text = serde_json::from_str::<String>(json.get(key)?.get()).ok()?;
        (text.len() <= max).then_some(text)
    };
    let call = |key| {
        text(key, 32).filter(|s| {
            !s.is_empty()
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'#'))
        })
    };
    let number = |key| number::Decimal::parse(json.get(key)?.get());
    Some(Spot {
        spot_id: number("spotId")?
            .whole()
            .filter(|n| (1..=1 << 53).contains(n))?,
        activator: call("activator")?,
        spotter: call("spotter")?,
        frequency_hz: frequency(&text("frequency", MAX_BYTES)?)?,
        reference: text("reference", 10).filter(|s| reference(s))?,
        spot_time: utc::seconds(&text("spotTime", 27)?)?,
        mode: text("mode", 16).filter(|s| s.bytes().all(|b| (b' '..=b'~').contains(&b))),
        name: text("name", 200),
        comments: text("comments", 512),
        source: text("source", 64),
        location_desc: text("locationDesc", 64),
        grid4: text("grid4", 4).filter(|s| {
            matches!(
                s.to_ascii_uppercase().as_bytes(),
                [b'A'..=b'R', b'A'..=b'R', b'0'..=b'9', b'0'..=b'9']
            )
        }),
        count: number("count")
            .and_then(number::Decimal::whole)
            .filter(|&n| n <= 100_000),
    })
}

fn frequency(text: &str) -> Option<u64> {
    let hz = match text.split_once('.') {
        Some((whole, fraction)) if (1..=3).contains(&fraction.len()) => digits(whole)?
            .checked_mul(1_000)?
            .checked_add(digits(fraction)? * 10_u64.pow(3 - fraction.len() as u32))?,
        None => digits(text)?.checked_mul(1_000)?,
        _ => return None,
    };
    (1..=300_000_000_000).contains(&hz).then_some(hz)
}

fn digits(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

fn reference(text: &str) -> bool {
    text.split_once('-').is_some_and(|(prefix, suffix)| {
        (1..=4).contains(&prefix.len())
            && prefix
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
            && (4..=5).contains(&suffix.len())
            && suffix.bytes().all(|b| b.is_ascii_digit())
    })
}
