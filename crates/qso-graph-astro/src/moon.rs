// SPDX-License-Identifier: GPL-3.0-or-later
//! Lunar position and phase from Meeus, following Desktop (QGDXC-AST-0020–0024).

use crate::{
    Crossings, Horizontal, Moment, Position, Span, azimuth, crossings, deg, elevation, norm, rad,
    signed_longitude, sun_at,
};
use core::fmt;

const EARTH_RADIUS: f64 = 6378.14;

// Meeus Table 47.A.
const LR: [[i32; 6]; 60] = [
    [0, 0, 1, 0, 6288774, -20905355],
    [2, 0, -1, 0, 1274027, -3699111],
    [2, 0, 0, 0, 658314, -2955968],
    [0, 0, 2, 0, 213618, -569925],
    [0, 1, 0, 0, -185116, 48888],
    [0, 0, 0, 2, -114332, -3149],
    [2, 0, -2, 0, 58793, 246158],
    [2, -1, -1, 0, 57066, -152138],
    [2, 0, 1, 0, 53322, -170733],
    [2, -1, 0, 0, 45758, -204586],
    [0, 1, -1, 0, -40923, -129620],
    [1, 0, 0, 0, -34720, 108743],
    [0, 1, 1, 0, -30383, 104755],
    [2, 0, 0, -2, 15327, 10321],
    [0, 0, 1, 2, -12528, 0],
    [0, 0, 1, -2, 10980, 79661],
    [4, 0, -1, 0, 10675, -34782],
    [0, 0, 3, 0, 10034, -23210],
    [4, 0, -2, 0, 8548, -21636],
    [2, 1, -1, 0, -7888, 24208],
    [2, 1, 0, 0, -6766, 30824],
    [1, 0, -1, 0, -5163, -8379],
    [1, 1, 0, 0, 4987, -16675],
    [2, -1, 1, 0, 4036, -12831],
    [2, 0, 2, 0, 3994, -10445],
    [4, 0, 0, 0, 3861, -11650],
    [2, 0, -3, 0, 3665, 14403],
    [0, 1, -2, 0, -2689, -7003],
    [2, 0, -1, 2, -2602, 0],
    [2, -1, -2, 0, 2390, 10056],
    [1, 0, 1, 0, -2348, 6322],
    [2, -2, 0, 0, 2236, -9884],
    [0, 1, 2, 0, -2120, 5751],
    [0, 2, 0, 0, -2069, 0],
    [2, -2, -1, 0, 2048, -4950],
    [2, 0, 1, -2, -1773, 4130],
    [2, 0, 0, 2, -1595, 0],
    [4, -1, -1, 0, 1215, -3958],
    [0, 0, 2, 2, -1110, 0],
    [3, 0, -1, 0, -892, 3258],
    [2, 1, 1, 0, -810, 2616],
    [4, -1, -2, 0, 759, -1897],
    [0, 2, -1, 0, -713, -2117],
    [2, 2, -1, 0, -700, 2354],
    [2, 1, -2, 0, 691, 0],
    [2, -1, 0, -2, 596, 0],
    [4, 0, 1, 0, 549, -1423],
    [0, 0, 4, 0, 537, -1117],
    [4, -1, 0, 0, 520, -1571],
    [1, 0, -2, 0, -487, -1739],
    [2, 1, 0, -2, -399, 0],
    [0, 0, 2, -2, -381, -4421],
    [1, 1, 1, 0, 351, 0],
    [3, 0, -2, 0, -340, 0],
    [4, 0, -3, 0, 330, 0],
    [2, -1, 2, 0, 327, 0],
    [0, 2, 1, 0, -323, 1165],
    [1, 1, -1, 0, 299, 0],
    [2, 0, 3, 0, 294, 0],
    [2, 0, -1, -2, 0, 8752],
];

// Meeus Table 47.B, as used by Desktop.
const B: [[i32; 5]; 30] = [
    [0, 0, 0, 1, 5128122],
    [0, 0, 1, 1, 280602],
    [0, 0, 1, -1, 277693],
    [2, 0, 0, -1, 173237],
    [2, 0, -1, 1, 55413],
    [2, 0, -1, -1, 46271],
    [2, 0, 0, 1, 32573],
    [0, 0, 2, 1, 17198],
    [2, 0, 1, -1, 9266],
    [0, 0, 2, -1, 8822],
    [2, -1, 0, -1, 8216],
    [2, 0, -2, -1, 4324],
    [2, 0, 1, 1, 4200],
    [2, 1, 0, -1, -3359],
    [2, -1, -1, 1, 2463],
    [2, -1, 0, 1, 2211],
    [2, -1, -1, -1, 2065],
    [0, 1, -1, -1, -1870],
    [4, 0, -1, -1, 1828],
    [0, 1, 0, 1, -1794],
    [0, 0, 0, 3, -1749],
    [0, 1, -1, 1, -1565],
    [1, 0, 0, 1, -1491],
    [0, 1, 1, 1, -1475],
    [0, 1, 1, -1, -1410],
    [0, 1, 0, -1, -1344],
    [1, 0, 0, -1, -1335],
    [0, 0, 3, 1, 1107],
    [4, 0, 0, -1, 1021],
    [4, 0, -1, 1, 833],
];

/// Geocentric lunar angles in degrees and distance in kilometers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Moon {
    pub longitude: f64,
    pub latitude: f64,
    pub distance: f64,
    pub right_ascension: f64,
    pub declination: f64,
}

fn eccentricity_scale(anomaly: i32, eccentricity: f64) -> f64 {
    match anomaly.abs() {
        1 => eccentricity,
        2 => eccentricity * eccentricity,
        _ => 1.0,
    }
}

pub fn moon_at(moment: Moment) -> Moon {
    let t = (moment.julian() - 2_451_545.0) / 36_525.0;
    let mean_longitude = norm(
        218.3164477 + 481267.88123421 * t - 0.0015786 * t * t + t.powi(3) / 538841.0
            - t.powi(4) / 65194000.0,
    );
    let elongation = norm(
        297.8501921 + 445267.1114034 * t - 0.0018819 * t * t + t.powi(3) / 545868.0
            - t.powi(4) / 113065000.0,
    );
    let sun_anomaly =
        norm(357.5291092 + 35999.0502909 * t - 0.0001536 * t * t + t.powi(3) / 24490000.0);
    let moon_anomaly = norm(
        134.9633964 + 477198.8675055 * t + 0.0087414 * t * t + t.powi(3) / 69699.0
            - t.powi(4) / 14712000.0,
    );
    let argument = norm(
        93.272095 + 483202.0175233 * t - 0.0036539 * t * t - t.powi(3) / 3526000.0
            + t.powi(4) / 863310000.0,
    );
    let a1 = norm(119.75 + 131.849 * t);
    let a2 = norm(53.09 + 479264.29 * t);
    let a3 = norm(313.45 + 481266.484 * t);
    let e = 1.0 - 0.002516 * t - 0.0000074 * t * t;
    let angle = |d, m, mp, f| {
        rad(f64::from(d) * elongation
            + f64::from(m) * sun_anomaly
            + f64::from(mp) * moon_anomaly
            + f64::from(f) * argument)
    };
    let mut l = 0.0;
    let mut r = 0.0;
    for [d, m, mp, f, cl, cr] in LR {
        let angle = angle(d, m, mp, f);
        let scale = eccentricity_scale(m, e);
        l += f64::from(cl) * scale * angle.sin();
        r += f64::from(cr) * scale * angle.cos();
    }
    let mut b = 0.0;
    for [d, m, mp, f, cb] in B {
        let scale = eccentricity_scale(m, e);
        b += f64::from(cb) * scale * angle(d, m, mp, f).sin();
    }
    l += 3958.0 * rad(a1).sin()
        + 1962.0 * rad(mean_longitude - argument).sin()
        + 318.0 * rad(a2).sin();
    b += -2235.0 * rad(mean_longitude).sin()
        + 382.0 * rad(a3).sin()
        + 175.0 * rad(a1 - argument).sin()
        + 175.0 * rad(a1 + argument).sin()
        + 127.0 * rad(mean_longitude - moon_anomaly).sin()
        - 115.0 * rad(mean_longitude + moon_anomaly).sin();
    let omega = rad(125.04452 - 1934.136261 * t);
    let nutation =
        (-17.2 * omega.sin() - 1.32 * (2.0 * rad(280.4665 + 36000.7698 * t)).sin()) / 3600.0;
    let longitude = norm(mean_longitude + l / 1e6 + nutation);
    let latitude = b / 1e6;
    let distance = 385000.56 + r / 1000.0;
    let obliquity = rad(23.4392911 - 0.0130042 * t + 0.00256 * omega.cos());
    let lambda = rad(longitude);
    let beta = rad(latitude);
    let right_ascension = norm(deg((lambda.sin() * obliquity.cos()
        - beta.tan() * obliquity.sin())
    .atan2(lambda.cos())));
    let declination =
        deg((beta.sin() * obliquity.cos() + beta.cos() * obliquity.sin() * lambda.sin()).asin());
    Moon {
        longitude,
        latitude,
        distance,
        right_ascension,
        declination,
    }
}

fn sidereal(moment: Moment) -> f64 {
    let days = moment.julian() - 2_451_545.0;
    let t = days / 36_525.0;
    norm(280.46061837 + 360.98564736629 * days + 0.000387933 * t * t - t.powi(3) / 38710000.0)
}

fn hour_angle(moment: Moment, place: Position, moon: Moon) -> f64 {
    rad(sidereal(moment) + place.longitude() - moon.right_ascension)
}

pub fn moon_position(moment: Moment, place: Position) -> Horizontal {
    let moon = moon_at(moment);
    let (latitude, declination, hour) = (
        rad(place.latitude()),
        rad(moon.declination),
        hour_angle(moment, place, moon),
    );
    let geocentric = elevation(latitude, declination, hour);
    let parallax = deg(((EARTH_RADIUS / moon.distance) * rad(geocentric).cos()).asin());
    Horizontal {
        azimuth: azimuth(latitude, declination, hour),
        elevation: geocentric - parallax,
    }
}

pub fn moon_crossings(span: Span, place: Position) -> Crossings {
    crossings(span, |moment| {
        let moon = moon_at(moment);
        let parallax = deg((EARTH_RADIUS / moon.distance).asin());
        let h0 = 0.7275 * parallax - 34.0 / 60.0;
        elevation(
            rad(place.latitude()),
            rad(moon.declination),
            hour_angle(moment, place, moon),
        ) - h0
    })
}

pub fn sublunar_point(moment: Moment) -> Position {
    let moon = moon_at(moment);
    let longitude = signed_longitude(moon.right_ascension - sidereal(moment));
    Position::new(moon.declination, longitude).expect("lunar declination and normalized longitude")
}

/// Illuminated fraction, elongation east of the sun in degrees, and named phase.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Phase {
    pub fraction: f64,
    pub elongation: f64,
    pub name: PhaseName,
}

/// The named lunar phases (QGDXC-AST-0023).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhaseName {
    NewMoon,
    WaxingCrescent,
    FirstQuarter,
    WaxingGibbous,
    FullMoon,
    WaningGibbous,
    LastQuarter,
    WaningCrescent,
}

impl PhaseName {
    pub fn label(self) -> &'static str {
        match self {
            Self::NewMoon => "New moon",
            Self::WaxingCrescent => "Waxing crescent",
            Self::FirstQuarter => "First quarter",
            Self::WaxingGibbous => "Waxing gibbous",
            Self::FullMoon => "Full moon",
            Self::WaningGibbous => "Waning gibbous",
            Self::LastQuarter => "Last quarter",
            Self::WaningCrescent => "Waning crescent",
        }
    }
}

impl fmt::Display for PhaseName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

pub fn moon_phase(moment: Moment) -> Phase {
    use PhaseName::*;
    let moon = moon_at(moment);
    let sun = sun_at(moment);
    let difference = moon.longitude - sun.apparent_longitude;
    let elongation = norm(difference);
    let psi = (rad(moon.latitude).cos() * rad(difference).cos()).acos();
    let sun_distance = 149597870.7;
    let phase_angle = (sun_distance * psi.sin()).atan2(moon.distance - sun_distance * psi.cos());
    let fraction = (1.0 + phase_angle.cos()) / 2.0;
    let name = match elongation {
        ..=6.0 | 354.0.. => NewMoon,
        ..84.0 => WaxingCrescent,
        ..=96.0 => FirstQuarter,
        ..174.0 => WaxingGibbous,
        ..=186.0 => FullMoon,
        ..264.0 => WaningGibbous,
        ..=276.0 => LastQuarter,
        _ => WaningCrescent,
    };
    Phase {
        fraction,
        elongation,
        name,
    }
}
