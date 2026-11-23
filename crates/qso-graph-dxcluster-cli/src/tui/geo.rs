// SPDX-License-Identifier: GPL-3.0-or-later
//! Map geometry for the spot map (QGDXC-TUI-0082, QGDXC-TUI-0084): grid centers and
//! great-circle paths on an equirectangular map.
use qso_graph_dxcluster_cli::prefixes::Place;
use qso_graph_grid::{Locator, Precision};

/// The center of a four-character Maidenhead locator, such as `FN42`.
pub fn grid_center(grid: &str) -> Option<Place> {
    let locator = Locator::parse(grid).ok()?;
    if locator.precision() != Precision::Square {
        return None;
    }
    let centre = locator.centre();
    Some(Place {
        lon: centre.longitude(),
        lat: centre.latitude(),
    })
}

/// Largest step between path points, in degrees of arc.
pub const STEP_DEGREES: f64 = 2.0;

/// The short great-circle path from `a` to `b` as polylines of (longitude, latitude),
/// split where it crosses the 180th meridian so no segment spans the map. Returns no
/// path for antipodal points, whose great circle is undefined.
pub fn great_circle(a: Place, b: Place) -> Vec<Vec<(f64, f64)>> {
    let v = |p: Place| {
        let (lat, lon) = (p.lat.to_radians(), p.lon.to_radians());
        [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
    };
    let (va, vb) = (v(a), v(b));
    let dot = (va[0] * vb[0] + va[1] * vb[1] + va[2] * vb[2]).clamp(-1.0, 1.0);
    let angle = dot.acos();
    if (std::f64::consts::PI - angle) < 1e-6 {
        return vec![];
    }
    let steps = ((angle.to_degrees() / STEP_DEGREES).ceil() as usize).max(1);
    let sin = angle.sin();
    let mut lines = vec![vec![]];
    let mut last: Option<f64> = None;
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let (wa, wb) = if sin.abs() < 1e-12 {
            (1.0 - t, t)
        } else {
            (((1.0 - t) * angle).sin() / sin, (t * angle).sin() / sin)
        };
        let p = [
            wa * va[0] + wb * vb[0],
            wa * va[1] + wb * vb[1],
            wa * va[2] + wb * vb[2],
        ];
        let lat = p[2].clamp(-1.0, 1.0).asin().to_degrees();
        let lon = p[1].atan2(p[0]).to_degrees();
        if last.is_some_and(|l| (lon - l).abs() > 180.0) {
            lines.push(vec![]);
        }
        last = Some(lon);
        lines.last_mut().expect("current line").push((lon, lat));
    }
    lines.retain(|l| !l.is_empty());
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_centers_follow_maidenhead() {
        // QGDXC-TUI-0082 QGDXC-TUI-0145
        assert_eq!(
            grid_center("FN42"),
            Some(Place {
                lat: 42.5,
                lon: -71.0
            })
        );
        assert_eq!(
            grid_center("jo65"),
            Some(Place {
                lat: 55.5,
                lon: 13.0
            })
        );
        assert_eq!(
            grid_center("AA00"),
            Some(Place {
                lat: -89.5,
                lon: -179.0
            })
        );
        assert_eq!(
            grid_center("RR99"),
            Some(Place {
                lat: 89.5,
                lon: 179.0
            })
        );
        for bad in [
            "", "FN", "FN4%", "FN42..", "QN42aa", "QN42aa12", "SN42", "F#42", "FNA2", "ＦN42",
        ] {
            assert_eq!(grid_center(bad), None, "{bad}");
        }
    }

    #[test]
    fn great_circle_paths_curve_and_split_at_the_date_line() {
        // QGDXC-TUI-0084
        let boston = Place {
            lat: 42.5,
            lon: -71.0,
        };
        let tokyo = Place {
            lat: 35.5,
            lon: 139.0,
        };
        let path = great_circle(boston, tokyo);
        // Over the pole region, not along the straight map line.
        let max_lat = path.iter().flatten().map(|p| p.1).fold(f64::MIN, f64::max);
        assert!(max_lat > 60.0, "{max_lat}");
        // Crosses 180 degrees: two pieces, neither spanning the map.
        assert_eq!(path.len(), 2);
        for line in &path {
            for pair in line.windows(2) {
                assert!((pair[1].0 - pair[0].0).abs() <= 180.0);
            }
        }
        let first = path[0][0];
        let last = *path[1].last().unwrap();
        assert!((first.0 - boston.lon).abs() < 1e-9 && (first.1 - boston.lat).abs() < 1e-9);
        assert!((last.0 - tokyo.lon).abs() < 1e-9 && (last.1 - tokyo.lat).abs() < 1e-9);
        // A short path is one piece; the same point is one point; antipodes draw nothing.
        assert_eq!(
            great_circle(
                boston,
                Place {
                    lat: 40.0,
                    lon: -75.0
                }
            )
            .len(),
            1
        );
        let same = great_circle(boston, boston);
        assert_eq!((same.len(), same[0].len()), (1, 2));
        assert!(
            same[0]
                .iter()
                .all(|p| (p.0 - boston.lon).abs() < 1e-9 && (p.1 - boston.lat).abs() < 1e-9)
        );
        assert!(
            great_circle(
                Place {
                    lat: 10.0,
                    lon: 20.0
                },
                Place {
                    lat: -10.0,
                    lon: -160.0
                }
            )
            .is_empty()
        );
    }
}
