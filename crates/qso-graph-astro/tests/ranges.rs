// SPDX-License-Identifier: GPL-3.0-or-later
use qso_graph_astro::*;

const FIRST: i64 = -631_152_000_000;
const LIMIT: i64 = 4_102_444_800_000;

#[test]
fn moments_spans_and_terminator_refuse_invalid_inputs() {
    for ms in [i64::MIN, FIRST - 1, LIMIT, LIMIT + 1, i64::MAX] {
        assert_eq!(Moment::from_unix_millis(ms), Err(Error::Moment));
    }
    let first = Moment::from_unix_millis(FIRST).unwrap();
    for minutes in [i64::MIN, -1, 0, 14_401, i64::MAX] {
        assert_eq!(Span::new(first, minutes), Err(Error::Span));
    }
    assert!(Span::new(first, 1).is_ok());
    assert!(Span::new(first, 14_400).is_ok());
    assert!(Span::new(Moment::from_unix_millis(LIMIT - 60_001).unwrap(), 1).is_ok());
    assert_eq!(
        Span::new(Moment::from_unix_millis(LIMIT - 60_000).unwrap(), 1),
        Err(Error::Span)
    );
    assert_eq!(
        Span::new(Moment::from_unix_millis(LIMIT - 1).unwrap(), 1),
        Err(Error::Span)
    );
    for bad in [
        f64::NAN,
        f64::NEG_INFINITY,
        f64::INFINITY,
        -90.1,
        -90.0,
        90.0,
        90.1,
    ] {
        assert_eq!(terminator(first, bad, 2.0), Err(Error::Altitude));
    }
    for bad in [
        f64::NAN,
        f64::NEG_INFINITY,
        f64::INFINITY,
        -1.0,
        0.0,
        0.49,
        0.7,
        10.01,
    ] {
        assert_eq!(terminator(first, HORIZON, bad), Err(Error::Step));
    }
    let midnight = station_day_start(first, Position::new(0.0, 180.0).unwrap());
    assert_eq!(Span::new(midnight, 1), Err(Error::Span));
}

#[test]
fn terminator_closes_splits_and_stays_at_its_solar_altitude() {
    let mut split_count = 0;
    for ms in [FIRST, -1, 0, 1_795_376_460_000, LIMIT - 1] {
        let at = Moment::from_unix_millis(ms).unwrap();
        for altitude in [
            -89.9999,
            ASTRONOMICAL,
            F2_LAYER,
            NAUTICAL,
            D_LAYER,
            CIVIL,
            HORIZON,
            0.0,
            45.0,
            89.9999,
        ] {
            for step in [0.5, 1.2, 2.0, 10.0] {
                let lines = terminator(at, altitude, step).unwrap();
                assert_eq!(lines[0].first(), lines.last().unwrap().last());
                split_count += lines.len() - 1;
                let points: usize = lines.iter().map(Vec::len).sum();
                assert_eq!(points, (360.0 / step) as usize + 1 + 2 * (lines.len() - 1));
                for pair in lines.windows(2) {
                    let &(left, a) = pair[0].last().unwrap();
                    let (right, b) = pair[1][0];
                    assert_eq!(left.abs(), 180.0);
                    assert_eq!((left, a), (-right, b));
                }
                for line in lines {
                    for pair in line.windows(2) {
                        assert!((pair[1].0 - pair[0].0).abs() <= 180.0, "{pair:?}");
                    }
                    for (lon, lat) in line {
                        let place = Position::new(lat, lon).unwrap();
                        let elevation = sun_position(at, place).elevation;
                        assert!(
                            (elevation - altitude).abs() <= 0.01,
                            "{at:?} {place:?}: {elevation} vs {altitude}"
                        );
                    }
                }
            }
        }
    }
    assert!(split_count > 0);
}

#[test]
fn supported_moments_places_spans_and_altitudes_are_finite() {
    for ms in [
        FIRST,
        FIRST + 1,
        -1,
        0,
        1,
        1_795_376_460_000,
        LIMIT - 60_001,
        LIMIT - 1,
    ] {
        let at = Moment::from_unix_millis(ms).unwrap();
        for latitude in [
            -90.00001, -90.0, -89.99999, -45.0, 0.0, 45.0, 89.99999, 90.0, 90.00001,
        ] {
            for longitude in [
                -180.00001, -180.0, -179.99999, -90.0, 0.0, 90.0, 179.99999, 180.0, 180.00001,
            ] {
                let Ok(place) = Position::new(latitude, longitude) else {
                    continue;
                };
                for result in [sun_position(at, place), moon_position(at, place)] {
                    assert!((0.0..360.0).contains(&result.azimuth));
                    assert!((-90.0..=90.0).contains(&result.elevation));
                }
                for point in [subsolar_point(at), sublunar_point(at)] {
                    assert!((-90.0..=90.0).contains(&point.latitude()));
                    assert!(point.longitude() > -180.0 && point.longitude() <= 180.0);
                }
                let phase = moon_phase(at);
                let moon = moon_at(at);
                let sun = sun_at(at);
                assert!((0.0..=1.0).contains(&phase.fraction));
                assert!((0.0..360.0).contains(&phase.elongation));
                assert!(
                    [
                        moon.longitude,
                        moon.latitude,
                        moon.distance,
                        moon.right_ascension,
                        moon.declination,
                        sun.declination,
                        sun.equation_of_time,
                        sun.apparent_longitude
                    ]
                    .into_iter()
                    .all(f64::is_finite)
                );
                assert!(station_day_start(at, place) <= at);
                if let Ok(span) = Span::new(at, 1) {
                    assert_eq!(sun_highest(span, place), at);
                    let _ = moon_crossings(span, place);
                    for altitude in [
                        -90.1,
                        -90.0,
                        ASTRONOMICAL,
                        F2_LAYER,
                        NAUTICAL,
                        D_LAYER,
                        CIVIL,
                        HORIZON,
                        90.0,
                        90.1,
                    ] {
                        let _ = sun_crossings(span, place, altitude);
                    }
                }
            }
        }
    }
    let at = Moment::from_unix_millis(-1).unwrap();
    let place = Position::new(70.0, -179.99999).unwrap();
    for minutes in [1, 2, 719, 1440, 14_399, 14_400] {
        let span = Span::new(at, minutes).unwrap();
        let highest = sun_highest(span, place).unix_millis() - at.unix_millis();
        assert!((0..minutes * 60_000).contains(&highest));
        assert_eq!(highest % 60_000, 0);
        for crossings in [
            sun_crossings(span, place, HORIZON),
            moon_crossings(span, place),
        ] {
            for event in [crossings.rising, crossings.setting].into_iter().flatten() {
                assert_eq!(event.unix_millis() % 60_000, 0);
            }
        }
    }
    for ms in [FIRST, LIMIT - 1] {
        let at = Moment::from_unix_millis(ms).unwrap();
        let place = Position::new(0.0, 0.0).unwrap();
        for depth in grey_line_windows(at, place, place) {
            assert!(depth.depth.is_finite());
            assert!(
                depth
                    .windows
                    .iter()
                    .all(|window| window.start <= window.end)
            );
        }
    }
}

#[test]
fn named_altitudes_match_the_annex() {
    assert_eq!(
        [HORIZON, CIVIL, NAUTICAL, ASTRONOMICAL, D_LAYER, F2_LAYER],
        [-0.833, -6.0, -12.0, -18.0, -10.1, -17.2]
    );
}
