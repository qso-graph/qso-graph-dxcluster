// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
//! Sans-I/O parsers for NOAA SWPC products (QGDXC-SWX-0001).

mod json;
mod utc;

use serde_json::Value;
use std::{collections::BTreeMap, error, fmt};

/// Products and their bounds from Annex H.1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Product {
    SolarFlux,
    PlanetaryK,
    Scales,
    Xrays,
    WindSpeed,
    WindField,
    Alerts,
}

impl Product {
    pub const fn name(self) -> &'static str {
        match self {
            Self::SolarFlux => "solar flux",
            Self::PlanetaryK => "planetary K",
            Self::Scales => "NOAA scales",
            Self::Xrays => "X-rays",
            Self::WindSpeed => "solar wind speed",
            Self::WindField => "solar wind field",
            Self::Alerts => "alerts",
        }
    }

    pub const fn limit(self) -> usize {
        match self {
            Self::SolarFlux | Self::WindSpeed | Self::WindField => 4 * 1024,
            Self::Scales => 16 * 1024,
            Self::PlanetaryK => 64 * 1024,
            Self::Alerts => 256 * 1024,
            Self::Xrays => 512 * 1024,
        }
    }
}

/// The reason an entire product was refused (QGDXC-SWX-0002).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Reason {
    ByteLimit,
    Utf8,
    Json,
    Depth,
    DuplicateMember,
    Missing(&'static str),
    Type(&'static str),
    Range(&'static str),
    Time,
    EntryLimit,
    Empty,
    MessageLimit,
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ByteLimit => f.write_str("byte limit exceeded"),
            Self::Utf8 => f.write_str("invalid UTF-8"),
            Self::Json => f.write_str("invalid JSON"),
            Self::Depth => f.write_str("JSON nested deeper than 16 levels"),
            Self::DuplicateMember => f.write_str("duplicate JSON member"),
            Self::Missing(member) => write!(f, "missing {member}"),
            Self::Type(member) => write!(f, "wrong JSON type for {member}"),
            Self::Range(member) => write!(f, "out of range: {member}"),
            Self::Time => f.write_str("invalid UTC timestamp"),
            Self::EntryLimit => f.write_str("entry limit exceeded"),
            Self::Empty => f.write_str("no usable values"),
            Self::MessageLimit => f.write_str("alert message exceeds 16 KiB"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub product: Product,
    pub reason: Reason,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.product.name(), self.reason)
    }
}

impl error::Error for Error {}

/// Each time is a UTC timestamp in seconds since the Unix epoch.
#[derive(Clone, Debug, PartialEq)]
pub struct SolarFlux {
    pub time: i64,
    pub flux: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanetaryK {
    pub time: i64,
    pub kp: f64,
    pub a_running: f64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Scales {
    pub time: i64,
    pub r: u8,
    pub s: u8,
    pub g: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Xray {
    pub time: i64,
    pub flux: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindSpeed {
    pub time: i64,
    pub proton_speed: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindField {
    pub time: i64,
    pub bt: f64,
    pub bz_gsm: f64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Alert {
    pub product_id: String,
    pub issue_time: i64,
    /// Escape this text before display.
    pub headline: Option<String>,
}

fn parse<T>(
    data: &[u8],
    product: Product,
    extract: impl FnOnce(&Value) -> Result<T, Reason>,
) -> Result<T, Error> {
    let result = if data.len() > product.limit() {
        Err(Reason::ByteLimit)
    } else {
        json::read(data).and_then(|value| extract(&value))
    };
    result.map_err(|reason| Error { product, reason })
}

fn member<'a>(value: &'a Value, key: &'static str) -> Result<&'a Value, Reason> {
    value
        .as_object()
        .ok_or(Reason::Type("object"))?
        .get(key)
        .ok_or(Reason::Missing(key))
}

fn string<'a>(value: &'a Value, key: &'static str) -> Result<&'a str, Reason> {
    member(value, key)?.as_str().ok_or(Reason::Type(key))
}

fn number(value: &Value, key: &'static str, min: f64, max: f64) -> Result<f64, Reason> {
    let number = member(value, key)?.as_f64().ok_or(Reason::Type(key))?;
    if !(min..=max).contains(&number) {
        return Err(Reason::Range(key));
    }
    Ok(number)
}

fn entries(value: &Value) -> Result<&[Value], Reason> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or(Reason::Type("array"))
}

// The summary products each publish one record in a JSON array (Annex H.1).
fn summary(value: &Value) -> Result<&Value, Reason> {
    match entries(value)? {
        [] => Err(Reason::Empty),
        [record] => Ok(record),
        _ => Err(Reason::EntryLimit),
    }
}

fn timestamp(value: &Value) -> Result<i64, Reason> {
    let text = string(value, "time_tag")?;
    utc::iso(text).ok_or(Reason::Time)
}

pub fn solar_flux(data: &[u8]) -> Result<SolarFlux, Error> {
    parse(data, Product::SolarFlux, |value| {
        let value = summary(value)?;
        Ok(SolarFlux {
            time: timestamp(value)?,
            flux: number(value, "flux", 0.0, 999.0)?,
        })
    })
}

pub fn planetary_k(data: &[u8]) -> Result<Vec<PlanetaryK>, Error> {
    parse(data, Product::PlanetaryK, |value| {
        let entries = entries(value)?;
        if entries.is_empty() {
            return Err(Reason::Empty);
        }
        if entries.len() > 200 {
            return Err(Reason::EntryLimit);
        }
        let mut sorted = BTreeMap::new();
        for item in entries {
            let time = timestamp(item)?;
            sorted.insert(
                time,
                PlanetaryK {
                    time,
                    kp: number(item, "Kp", 0.0, 9.0)?,
                    a_running: number(item, "a_running", 0.0, 400.0)?,
                },
            );
        }
        Ok(sorted.into_values().collect())
    })
}

pub fn scales(data: &[u8]) -> Result<Scales, Error> {
    parse(data, Product::Scales, |value| {
        let current = member(value, "0")?;
        let date = string(current, "DateStamp")?;
        let clock = string(current, "TimeStamp")?;
        let scale = |key| match string(member(current, key)?, "Scale")?.as_bytes() {
            &[n @ b'0'..=b'5'] => Ok(n - b'0'),
            _ => Err(Reason::Range("Scale")),
        };
        Ok(Scales {
            time: utc::datetime(date.as_bytes(), clock.as_bytes()).ok_or(Reason::Time)?,
            r: scale("R")?,
            s: scale("S")?,
            g: scale("G")?,
        })
    })
}

pub fn xrays(data: &[u8]) -> Result<Vec<Xray>, Error> {
    parse(data, Product::Xrays, |value| {
        let mut sorted = BTreeMap::new();
        let mut count = 0;
        for item in entries(value)? {
            if string(item, "energy")? != "0.1-0.8nm" {
                continue;
            }
            count += 1;
            if count > 1000 {
                return Err(Reason::EntryLimit);
            }
            let time = timestamp(item)?;
            sorted.insert(
                time,
                Xray {
                    time,
                    flux: number(item, "flux", 1e-10, 1e-1)?,
                },
            );
        }
        if sorted.is_empty() {
            return Err(Reason::Empty);
        }
        Ok(sorted.into_values().collect())
    })
}

/// Format the class of a parsed flux (QGDXC-SWX-0005).
pub fn xray_class(flux: f64) -> String {
    let (class, tenths_per_unit) = match flux {
        x if x >= 1e-4 => ('X', 1e5),
        x if x >= 1e-5 => ('M', 1e6),
        x if x >= 1e-6 => ('C', 1e7),
        x if x >= 1e-7 => ('B', 1e8),
        _ => ('A', 1e9),
    };
    let tenths = (flux * tenths_per_unit) as u64;
    format!("{class}{}.{}", tenths / 10, tenths % 10)
}

pub fn wind_speed(data: &[u8]) -> Result<WindSpeed, Error> {
    parse(data, Product::WindSpeed, |value| {
        let value = summary(value)?;
        Ok(WindSpeed {
            time: timestamp(value)?,
            proton_speed: number(value, "proton_speed", 0.0, 3000.0)?,
        })
    })
}

pub fn wind_field(data: &[u8]) -> Result<WindField, Error> {
    parse(data, Product::WindField, |value| {
        let value = summary(value)?;
        Ok(WindField {
            time: timestamp(value)?,
            bt: number(value, "bt", 0.0, 200.0)?,
            bz_gsm: number(value, "bz_gsm", -200.0, 200.0)?,
        })
    })
}

pub fn alerts(data: &[u8]) -> Result<Vec<Alert>, Error> {
    parse(data, Product::Alerts, |value| {
        let entries = entries(value)?;
        if entries.len() > 100 {
            return Err(Reason::EntryLimit);
        }
        let mut sorted = Vec::with_capacity(entries.len());
        for item in entries {
            let id = string(item, "product_id")?;
            if !(1..=16).contains(&id.len()) || !id.bytes().all(|b| (b' '..=b'~').contains(&b)) {
                return Err(Reason::Range("product_id"));
            }
            let time = utc::issue(string(item, "issue_datetime")?).ok_or(Reason::Time)?;
            let message = string(item, "message")?;
            if message.len() > 16 * 1024 {
                return Err(Reason::MessageLimit);
            }
            let headline = message.lines().find(|line| {
                let line = line.trim_start_matches(' ');
                [
                    "ALERT:",
                    "WARNING:",
                    "WATCH:",
                    "SUMMARY:",
                    "CANCEL WARNING:",
                    "CANCEL WATCH:",
                    "EXTENDED WARNING:",
                    "CONTINUED ALERT:",
                ]
                .iter()
                .any(|prefix| line.starts_with(prefix))
            });
            sorted.push((
                time,
                Alert {
                    product_id: id.to_owned(),
                    issue_time: time.0,
                    headline: headline.map(str::to_owned),
                },
            ));
        }
        sorted.sort_by_key(|(time, _)| std::cmp::Reverse(*time));
        Ok(sorted.into_iter().map(|(_, alert)| alert).collect())
    })
}
