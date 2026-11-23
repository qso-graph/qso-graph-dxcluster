// SPDX-License-Identifier: GPL-3.0-or-later
//! NOAA's solar calculation and Desktop's minute sampling (QGDXC-AST-0010–0014).

use crate::{
    Crossings, DAY, Horizontal, MINUTE, Moment, Position, Span, azimuth, crossings, deg, elevation,
    norm, rad, signed_longitude,
};

/// Altitudes in degrees (QGDXC-AST-0011).
pub const HORIZON: f64 = -0.833;
pub const CIVIL: f64 = -6.0;
pub const NAUTICAL: f64 = -12.0;
pub const ASTRONOMICAL: f64 = -18.0;
pub const D_LAYER: f64 = -10.1;
pub const F2_LAYER: f64 = -17.2;

/// Solar angles in degrees and equation of time in minutes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sun {
    pub declination: f64,
    pub equation_of_time: f64,
    pub apparent_longitude: f64,
}

pub fn sun_at(moment: Moment) -> Sun {
    let t = (moment.julian() - 2_451_545.0) / 36_525.0;
    let mean_longitude = (280.46646 + t * (36000.76983 + t * 0.0003032)) % 360.0;
    let mean_anomaly = 357.52911 + t * (35999.05029 - 0.0001537 * t);
    let eccentricity = 0.016708634 - t * (0.000042037 + 0.0000001267 * t);
    let centre = rad(mean_anomaly).sin() * (1.914602 - t * (0.004817 + 0.000014 * t))
        + rad(2.0 * mean_anomaly).sin() * (0.019993 - 0.000101 * t)
        + rad(3.0 * mean_anomaly).sin() * 0.000289;
    let omega = 125.04 - 1934.136 * t;
    let apparent_longitude = mean_longitude + centre - 0.00569 - 0.00478 * rad(omega).sin();
    let mean_obliquity =
        23.0 + (26.0 + (21.448 - t * (46.815 + t * (0.00059 - t * 0.001813))) / 60.0) / 60.0;
    let obliquity = mean_obliquity + 0.00256 * rad(omega).cos();
    let declination = deg((rad(obliquity).sin() * rad(apparent_longitude).sin()).asin());
    let y = rad(obliquity / 2.0).tan().powi(2);
    let equation_of_time =
        4.0 * deg(y * (2.0 * rad(mean_longitude)).sin()
            - 2.0 * eccentricity * rad(mean_anomaly).sin()
            + 4.0 * eccentricity * y * rad(mean_anomaly).sin() * (2.0 * rad(mean_longitude)).cos()
            - 0.5 * y * y * (4.0 * rad(mean_longitude)).sin()
            - 1.25 * eccentricity * eccentricity * (2.0 * rad(mean_anomaly)).sin());
    Sun {
        declination,
        equation_of_time,
        apparent_longitude: norm(apparent_longitude),
    }
}

fn minutes_utc(moment: Moment) -> f64 {
    (((moment.julian() - 0.5) % 1.0 + 1.0) % 1.0) * 1440.0
}

fn hour_angle(moment: Moment, sun: Sun, longitude: f64) -> f64 {
    let true_solar_time = minutes_utc(moment) + sun.equation_of_time + 4.0 * longitude;
    rad(true_solar_time / 4.0 - 180.0)
}

pub fn sun_position(moment: Moment, place: Position) -> Horizontal {
    let sun = sun_at(moment);
    let (latitude, declination, hour) = (
        rad(place.latitude()),
        rad(sun.declination),
        hour_angle(moment, sun, place.longitude()),
    );
    Horizontal {
        azimuth: azimuth(latitude, declination, hour),
        elevation: elevation(latitude, declination, hour),
    }
}

pub(super) fn sun_altitude(moment: Moment, place: Position) -> f64 {
    let sun = sun_at(moment);
    elevation(
        rad(place.latitude()),
        rad(sun.declination),
        hour_angle(moment, sun, place.longitude()),
    )
}

pub fn subsolar_point(moment: Moment) -> Position {
    let sun = sun_at(moment);
    let longitude = signed_longitude((720.0 - minutes_utc(moment) - sun.equation_of_time) / 4.0);
    Position::new(sun.declination, longitude).expect("solar declination and normalized longitude")
}

pub fn sun_crossings(span: Span, place: Position, altitude: f64) -> Crossings {
    crossings(span, |at| sun_altitude(at, place) - altitude)
}

pub fn sun_highest(span: Span, place: Position) -> Moment {
    let mut best = span.start;
    let mut highest = f64::NEG_INFINITY;
    for minute in 0..span.minutes {
        let at = Moment(span.start.0 + minute * MINUTE);
        let altitude = sun_altitude(at, place);
        if altitude > highest {
            highest = altitude;
            best = at;
        }
    }
    best
}

/// Midnight in the station's mean solar time (QGDXC-AST-0013).
///
/// Near the start of the accepted input range, the resulting midnight can
/// precede it. Such a midnight cannot start a validated span.
pub fn station_day_start(moment: Moment, place: Position) -> Moment {
    let offset = (place.longitude() / 15.0) * 3_600_000.0;
    let local = (moment.0 as f64 + offset) as i64;
    let midnight = local.div_euclid(DAY) * DAY;
    Moment((midnight as f64 - offset) as i64)
}
