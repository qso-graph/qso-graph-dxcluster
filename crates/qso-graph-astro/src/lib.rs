// SPDX-License-Identifier: GPL-3.0-or-later
//! Sun, moon, and grey-line calculations (Annex L).
#![forbid(unsafe_code)]

mod greyline;
mod moon;
mod sun;

pub use greyline::{DepthWindows, MapLines, Window, grey_line_windows, terminator};
pub use moon::{
    Moon, Phase, PhaseName, moon_at, moon_crossings, moon_phase, moon_position, sublunar_point,
};
pub use qso_graph_grid::Position;
pub use sun::{
    ASTRONOMICAL, CIVIL, D_LAYER, F2_LAYER, HORIZON, NAUTICAL, Sun, station_day_start,
    subsolar_point, sun_at, sun_crossings, sun_highest, sun_position,
};

use core::{f64::consts::PI, fmt};

const MINUTE: i64 = 60_000;
const DAY: i64 = 1_440 * MINUTE;
const FIRST: i64 = -631_152_000_000;
const LIMIT: i64 = 4_102_444_800_000;

/// An input outside its specified range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Moment,
    Span,
    Altitude,
    Step,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Moment => "moment must be in 1950 through 2099 UTC",
            Self::Span => "span must be 1 to 14400 minutes within the supported UTC range",
            Self::Altitude => "terminator altitude must be strictly between -90 and 90 degrees",
            Self::Step => "terminator step must be 0.5 to 10 degrees and divide 360",
        })
    }
}
impl core::error::Error for Error {}

/// Whole UTC milliseconds since 1970 (QGDXC-AST-0002).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Moment(i64);

impl Moment {
    pub fn from_unix_millis(milliseconds: i64) -> Result<Self, Error> {
        if !(FIRST..LIMIT).contains(&milliseconds) {
            return Err(Error::Moment);
        }
        Ok(Self(milliseconds))
    }

    pub fn unix_millis(self) -> i64 {
        self.0
    }

    fn julian(self) -> f64 {
        self.0 as f64 / DAY as f64 + 2_440_587.5
    }
}

/// A start and a whole number of minutes (QGDXC-AST-0002).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    start: Moment,
    minutes: i64,
}

impl Span {
    pub fn new(start: Moment, minutes: i64) -> Result<Self, Error> {
        if !(1..=14_400).contains(&minutes)
            || !(FIRST..LIMIT).contains(&start.0)
            || start.0 + minutes * MINUTE >= LIMIT
        {
            return Err(Error::Span);
        }
        Ok(Self { start, minutes })
    }
}

/// Azimuth east of true north and geometric elevation, both in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Horizontal {
    pub azimuth: f64,
    pub elevation: f64,
}

/// The body's position relative to an altitude on a span without crossings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllDay {
    Above,
    Below,
}

/// The first upward and downward crossings of an altitude (QGDXC-AST-0012).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Crossings {
    pub rising: Option<Moment>,
    pub setting: Option<Moment>,
    pub all_day: Option<AllDay>,
}

fn crossings(span: Span, height: impl Fn(Moment) -> f64) -> Crossings {
    let mut found = Crossings::default();
    let mut before = height(span.start);
    for minute in 1..=span.minutes {
        let at = span.start.0 + minute * MINUTE;
        let after = height(Moment(at));
        if (before < 0.0) != (after < 0.0) {
            let interpolated = (at - MINUTE) as f64 + (before / (before - after)) * MINUTE as f64;
            let rounded = (interpolated / MINUTE as f64 + 0.5).floor() as i64 * MINUTE;
            let event = if after >= 0.0 {
                &mut found.rising
            } else {
                &mut found.setting
            };
            event.get_or_insert(Moment(rounded));
        }
        before = after;
    }
    if found.rising.is_none() && found.setting.is_none() {
        let middle = Moment(span.start.0 + span.minutes * (MINUTE / 2));
        found.all_day = Some(if height(middle) > 0.0 {
            AllDay::Above
        } else {
            AllDay::Below
        });
    }
    found
}

// Keep the operation order used in Desktop's angular conversions.
fn rad(degrees: f64) -> f64 {
    (degrees * PI) / 180.0
}

fn deg(radians: f64) -> f64 {
    (radians * 180.0) / PI
}

fn norm(degrees: f64) -> f64 {
    ((degrees % 360.0) + 360.0) % 360.0
}

fn signed_longitude(degrees: f64) -> f64 {
    let longitude = ((degrees % 360.0 + 540.0) % 360.0) - 180.0;
    if longitude == -180.0 {
        180.0
    } else {
        longitude
    }
}

fn elevation(latitude: f64, declination: f64, hour: f64) -> f64 {
    deg(
        (latitude.sin() * declination.sin() + latitude.cos() * declination.cos() * hour.cos())
            .clamp(-1.0, 1.0)
            .asin(),
    )
}

fn azimuth(latitude: f64, declination: f64, hour: f64) -> f64 {
    let east = -hour.sin() * declination.cos();
    let north =
        declination.sin() * latitude.cos() - declination.cos() * latitude.sin() * hour.cos();
    norm(deg(east.atan2(north)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossing_rounding_and_midpoint() {
        // Halfway between UTC minutes rounds toward the later minute, on both
        // sides of the epoch. A zero sample counts as above at a crossing.
        for (start, expected) in [(-60_000, 0), (0, 60_000)] {
            let span = Span::new(Moment(start), 1).unwrap();
            let found = crossings(span, |at| (at.0 - start - 30_000) as f64);
            assert_eq!(found.rising, Some(Moment(expected)));
            assert_eq!(found.setting, None);
            assert_eq!(found.all_day, None);
        }
        let span = Span::new(Moment(0), 1).unwrap();
        assert_eq!(
            crossings(span, |at| at.0 as f64 - 60_000.0).rising,
            Some(Moment(60_000))
        );
        assert_eq!(
            crossings(span, |at| 30_000.0 - (at.0 - 30_000).abs() as f64).all_day,
            Some(AllDay::Above)
        );
        assert_eq!(crossings(span, |_| 0.0).all_day, Some(AllDay::Below));
    }
}
