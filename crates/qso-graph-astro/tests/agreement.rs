// SPDX-License-Identifier: GPL-3.0-or-later
use qso_graph_astro::*;
use serde_json::Value;

fn fixture(name: &str) -> Value {
    let file = std::fs::File::open(format!("tests/fixtures/{name}.json")).unwrap();
    serde_json::from_reader(std::io::BufReader::new(file)).unwrap()
}

fn number(value: &Value, field: &str) -> f64 {
    value[field].as_f64().unwrap()
}

fn position(value: &Value) -> Position {
    Position::new(number(value, "latitude"), number(value, "longitude")).unwrap()
}

fn moment(ms: i64) -> Moment {
    Moment::from_unix_millis(ms).unwrap()
}

// The supplied PyEphem and Desktop dates are all in 2026.
fn utc(iso: &str) -> Moment {
    let month: usize = iso[5..7].parse().unwrap();
    let day: i64 = iso[8..10].parse().unwrap();
    let hour: i64 = iso[11..13].parse().unwrap();
    let minute: i64 = iso[14..16].parse().unwrap();
    let second: i64 = iso[17..19].parse().unwrap();
    let lengths = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let days = lengths[..month - 1].iter().sum::<i64>() + day - 1;
    assert_eq!(&iso[..4], "2026");
    moment(1_767_225_600_000 + (((days * 24 + hour) * 60 + minute) * 60 + second) * 1000)
}

fn station_day(day: &Value, place: Position) -> Moment {
    station_day_start(utc(&format!("{}T12:00:00Z", day.as_str().unwrap())), place)
}

fn close(label: &str, actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        // Positions stay out of failure messages: the label says which value failed.
        "{label}: outside tolerance {tolerance}"
    );
}

fn crossings_equal(actual: Crossings, expected: &Value, label: &str) {
    assert_eq!(
        actual.rising.map(Moment::unix_millis),
        expected["rising"].as_i64(),
        "{label} rise"
    );
    assert_eq!(
        actual.setting.map(Moment::unix_millis),
        expected["setting"].as_i64(),
        "{label} set"
    );
    let all_day = actual.all_day.map(|side| match side {
        AllDay::Above => "above",
        AllDay::Below => "below",
    });
    assert_eq!(all_day, expected["all_day"].as_str(), "{label} all day");
}

#[test]
fn desktop_crossings_noon_and_day_start() {
    let data = fixture("desktop-agreement");
    let altitudes = [
        ("horizon", HORIZON),
        ("civil", CIVIL),
        ("nautical", NAUTICAL),
        ("astronomical", ASTRONOMICAL),
        ("d_layer", D_LAYER),
        ("f2_layer", F2_LAYER),
    ];
    for case in data["days"].as_array().unwrap() {
        let label = format!("{} {}", case["place"], case["date"]);
        let place = position(case);
        let start = station_day(&case["date"], place);
        assert_eq!(
            start.unix_millis(),
            case["start"].as_i64().unwrap(),
            "{label} day start"
        );
        let span = Span::new(start, 1440).unwrap();
        for (name, altitude) in altitudes {
            crossings_equal(
                sun_crossings(span, place, altitude),
                &case["sun"][name],
                &format!("{label} {name}"),
            );
        }
        assert_eq!(
            sun_highest(span, place).unix_millis(),
            case["solar_noon"].as_i64().unwrap(),
            "{label} solar noon"
        );
        crossings_equal(
            moon_crossings(span, place),
            &case["moon"],
            &format!("{label} moon"),
        );
    }
}

#[test]
fn desktop_angles_distance_and_phase() {
    let data = fixture("desktop-agreement");
    let places = data["places"].as_array().unwrap();
    for case in data["positions"].as_array().unwrap() {
        let at = moment(case["ms"].as_i64().unwrap());
        let sun = sun_at(at);
        let moon = moon_at(at);
        let phase = moon_phase(at);
        let sublunar = sublunar_point(at);
        for (name, actual) in [
            ("sun_declination", sun.declination),
            ("equation_of_time", sun.equation_of_time),
            ("sun_apparent_longitude", sun.apparent_longitude),
            ("moon_longitude", moon.longitude),
            ("moon_latitude", moon.latitude),
            ("moon_right_ascension", moon.right_ascension),
            ("moon_declination", moon.declination),
            ("moon_elongation", phase.elongation),
            ("sublunar_latitude", sublunar.latitude()),
            ("sublunar_longitude", sublunar.longitude()),
        ] {
            close(&format!("{at:?} {name}"), actual, number(case, name), 1e-6);
        }
        for (place, expected) in places.iter().zip(case["sun_altitudes"].as_array().unwrap()) {
            close(
                &format!("{at:?} {} sun elevation", place["name"]),
                sun_position(at, position(place)).elevation,
                expected.as_f64().unwrap(),
                1e-6,
            );
        }
        close(
            "moon distance",
            moon.distance,
            number(case, "moon_distance_km"),
            1e-3,
        );
        close(
            "illuminated fraction",
            phase.fraction,
            number(case, "moon_fraction"),
            1e-9,
        );
        assert_eq!(
            phase.name.label(),
            case["moon_phase"].as_str().unwrap(),
            "{at:?}"
        );
        close(
            "subsolar elevation",
            sun_position(at, subsolar_point(at)).elevation,
            90.0,
            1e-5,
        );
    }
}

#[test]
fn desktop_grey_line_windows() {
    let data = fixture("desktop-agreement");
    let find = |name: &Value| {
        position(
            data["places"]
                .as_array()
                .unwrap()
                .iter()
                .find(|place| place["name"] == *name)
                .unwrap(),
        )
    };
    for case in data["grey_line"].as_array().unwrap() {
        let at = moment(case["from"].as_i64().unwrap());
        let answers = grey_line_windows(at, find(&case["here"]), find(&case["there"]));
        for (answer, expected) in answers.iter().zip(case["depths"].as_array().unwrap()) {
            assert_eq!(answer.depth, number(expected, "depth"));
            let actual: Vec<_> = answer
                .windows
                .iter()
                .map(|w| [w.start.unix_millis(), w.end.unix_millis()])
                .collect();
            assert_eq!(
                serde_json::json!(actual),
                expected["windows"],
                "{at:?} {} to {} at {}",
                case["here"],
                case["there"],
                answer.depth
            );
        }
    }
}

fn reference_events(name: &str, calculate: impl Fn(Span, Position) -> Crossings) {
    let reference = fixture(name);
    let mut differences = Vec::new();
    for case in reference.as_array().unwrap() {
        let place = position(case);
        let span = Span::new(station_day(&case["day"], place), 1440).unwrap();
        let found = calculate(span, place);
        for (event, actual) in [("rise", found.rising), ("set", found.setting)] {
            let actual = actual.map(Moment::unix_millis);
            let expected = case[event].as_str().map(|iso| utc(iso).unix_millis());
            match (actual, expected) {
                (Some(actual), Some(expected)) if (actual - expected).abs() <= 60_000 => {}
                (None, None) => {}
                _ => differences.push(format!(
                    "{} {} {event}: actual {actual:?}, PyEphem {expected:?}, delta ms {:?}",
                    case["place"],
                    case["day"],
                    actual.zip(expected).map(|(a, b)| a - b)
                )),
            }
        }
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

#[test]
fn pyephem_sun_events() {
    reference_events("sun-reference", |span, place| {
        sun_crossings(span, place, HORIZON)
    });
}

#[test]
fn pyephem_moon_events() {
    reference_events("moon-reference", moon_crossings);
}

#[test]
fn pyephem_moon_ephemeris() {
    for case in fixture("moon-reference").as_array().unwrap() {
        let at = utc(case["noon"].as_str().unwrap());
        let moon = moon_at(at);
        close(
            &format!("{at:?} declination"),
            moon.declination,
            number(case, "declination"),
            0.05,
        );
        close("distance", moon.distance, number(case, "distance_km"), 60.0);
        close(
            "illuminated fraction",
            moon_phase(at).fraction,
            number(case, "illuminated"),
            0.005,
        );
    }
}

#[test]
fn pyephem_horizontal_positions() {
    let mut differences = Vec::new();
    for case in fixture("position-reference").as_array().unwrap() {
        let at = utc(case["utc"].as_str().unwrap());
        let place = position(case);
        for (body, actual, tolerance) in [
            ("sun", sun_position(at, place), 0.02),
            ("moon", moon_position(at, place), 0.05),
        ] {
            let expected_azimuth = number(case, &format!("{body}_azimuth"));
            let expected_elevation = number(case, &format!("{body}_elevation"));
            let delta = (actual.azimuth - expected_azimuth).abs();
            let azimuth_error = delta.min(360.0 - delta) * expected_elevation.to_radians().cos();
            let elevation_error = (actual.elevation - expected_elevation).abs();
            if azimuth_error > tolerance || elevation_error > tolerance {
                differences.push(format!("{} {} {body}: scaled azimuth error {azimuth_error:.6}°, elevation error {elevation_error:.6}° (limit {tolerance}°)", case["place"], case["utc"]));
            }
        }
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}
