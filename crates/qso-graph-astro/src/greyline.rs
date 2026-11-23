// SPDX-License-Identifier: GPL-3.0-or-later
//! Terminators and grey-line windows (QGDXC-AST-0030–0031).

use crate::{
    D_LAYER, Error, F2_LAYER, HORIZON, MINUTE, Moment, Position, deg, rad, signed_longitude,
    subsolar_point, sun::sun_altitude,
};

/// Polylines of (longitude, latitude) points in degrees.
pub type MapLines = Vec<Vec<(f64, f64)>>;

pub fn terminator(moment: Moment, altitude: f64, step: f64) -> Result<MapLines, Error> {
    if !(-90.0 < altitude && altitude < 90.0) {
        return Err(Error::Altitude);
    }
    let count = 360.0 / step;
    if !(0.5..=10.0).contains(&step) || count.fract() != 0.0 {
        return Err(Error::Step);
    }

    let solar = subsolar_point(moment);
    let (solar_lat, solar_lon) = (rad(solar.latitude()), solar.longitude());
    let (sin_distance, cos_distance) = rad(90.0 - altitude).sin_cos();
    let (sin_solar, cos_solar) = solar_lat.sin_cos();
    let at = |bearing| {
        let bearing = rad(bearing);
        let latitude = (sin_solar * cos_distance + cos_solar * sin_distance * bearing.cos())
            .clamp(-1.0, 1.0)
            .asin();
        let dlon = (bearing.sin() * sin_distance * cos_solar)
            .atan2(cos_distance - sin_solar * latitude.sin());
        (signed_longitude(solar_lon + deg(dlon)), deg(latitude))
    };

    let first = at(0.0);
    let mut lines = Vec::new();
    let mut line = vec![first];
    let mut previous = first;
    let count = count as usize;
    for index in 1..=count {
        let bearing = index as f64 * step;
        let point = if index == count { first } else { at(bearing) };
        if (point.0 - previous.0).abs() > 180.0 {
            // Find the crossing on the circle so the two edges meet at the same latitude.
            let (mut lo, mut hi) = (bearing - step, bearing);
            let westward_edge = previous.0.is_sign_negative();
            for _ in 0..48 {
                let middle = (lo + hi) / 2.0;
                if at(middle).0.is_sign_negative() == westward_edge {
                    lo = middle;
                } else {
                    hi = middle;
                }
            }
            let latitude = at((lo + hi) / 2.0).1;
            let edge = 180.0_f64.copysign(previous.0);
            line.push((edge, latitude));
            lines.push(line);
            line = vec![(-edge, latitude)];
        }
        line.push(point);
        previous = point;
    }
    lines.push(line);
    Ok(lines)
}

/// The first and last qualifying minutes of a contiguous window, inclusive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Window {
    pub start: Moment,
    pub end: Moment,
}

/// Windows for one solar altitude in degrees.
#[derive(Clone, Debug, PartialEq)]
pub struct DepthWindows {
    pub depth: f64,
    pub windows: Vec<Window>,
}

/// Returns the windows for the D-layer and F2-layer depths, in that order.
pub fn grey_line_windows(moment: Moment, here: Position, there: Position) -> [DepthWindows; 2] {
    let start = moment.0.div_euclid(MINUTE) * MINUTE;
    let mut results = [D_LAYER, F2_LAYER].map(|depth| DepthWindows {
        depth,
        windows: Vec::new(),
    });
    let mut open = [None; 2];
    for minute in 0..=1440 {
        let at = Moment(start + minute * MINUTE);
        let first = sun_altitude(at, here);
        let second = sun_altitude(at, there);
        for (result, open) in results.iter_mut().zip(&mut open) {
            let inside = |altitude| altitude < HORIZON && altitude >= result.depth;
            let both = inside(first) && inside(second);
            if both {
                open.get_or_insert(at);
            }
            if (!both || minute == 1440)
                && let Some(start) = open.take()
            {
                let end = if both { at } else { Moment(at.0 - MINUTE) };
                result.windows.push(Window { start, end });
            }
        }
    }
    results
}
