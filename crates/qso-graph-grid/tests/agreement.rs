// SPDX-License-Identifier: GPL-3.0-or-later
use qso_graph_grid::{Locator, Position, Precision};
use serde_json::Value;

#[test]
fn locator_syntax_precision_and_canonical_case() {
    for (input, display, precision) in [
        ("fN", "FN", Precision::Field),
        ("fn31", "FN31", Precision::Square),
        ("qj00PQ", "QJ00pq", Precision::Subsquare),
        ("qN13Sm55", "QN13sm55", Precision::Extended),
    ] {
        let locator: Locator = input.parse().unwrap();
        assert_eq!(locator.precision(), precision);
        assert_eq!(locator.to_string(), display);
        assert_eq!(Locator::parse(display), Ok(locator));
    }
    for invalid in [
        "",
        "A",
        "AAA",
        "QN31p",
        "QN31pr1",
        "QN31pr555",
        " FN31",
        "FN31 ",
        "AB1 ",
        "AB\t1",
        "QB12yz",
        "AB12@!",
        "SZ",
        "QB12cd0!",
        "Ａ",
        "AéA",
        "AB\n1",
    ] {
        assert!(Locator::parse(invalid).is_err(), "{invalid:?}");
    }
}

#[test]
fn position_checks_edges_and_nonfinite_values() {
    for latitude in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -90.00001,
        90.00001,
    ] {
        assert!(Position::new(latitude, 0.0).is_err());
    }
    for longitude in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -180.00001,
        180.00001,
    ] {
        assert!(Position::new(0.0, longitude).is_err());
    }
    for (lat, lon, expected) in [
        (-90.0, -180.0, ["AA00", "aa00"]),
        (90.0, 180.0, ["RR99", "xx99"]),
        (-90.0, 180.0, ["RA90", "xa90"]),
        (90.0, -180.0, ["AR09", "ax09"]),
        (0.0, 0.0, ["JJ00", "aa00"]),
    ] {
        let position = Position::new(lat, lon).unwrap();
        assert_eq!(
            Locator::holding(position, Precision::Extended).to_string(),
            expected.concat()
        );
    }
}

#[test]
fn desktop_grid_agreement() {
    let file = std::fs::File::open("tests/fixtures/desktop-agreement.json").unwrap();
    let reference: Value = serde_json::from_reader(std::io::BufReader::new(file)).unwrap();
    for case in reference["bounds"].as_array().unwrap() {
        let label = case["locator"].as_str().unwrap();
        let locator = Locator::parse(label).unwrap();
        let bounds = locator.bounds();
        for (edge, value) in [
            ("south", bounds.south),
            ("west", bounds.west),
            ("north", bounds.north),
            ("east", bounds.east),
        ] {
            let expected = case["bounds"][edge].as_f64().unwrap();
            assert!(
                (value - expected).abs() <= 1e-9,
                "{label} {edge}: {value} vs {expected}"
            );
        }
        let centre = locator.centre();
        assert_eq!(centre.latitude(), (bounds.south + bounds.north) / 2.0);
        assert_eq!(centre.longitude(), (bounds.west + bounds.east) / 2.0);
        assert_eq!(Locator::holding(centre, locator.precision()), locator);
    }
    for case in reference["grid_of"].as_array().unwrap() {
        let position = Position::new(
            case["latitude"].as_f64().unwrap(),
            case["longitude"].as_f64().unwrap(),
        )
        .unwrap();
        for (precision, expected) in [
            Precision::Field,
            Precision::Square,
            Precision::Subsquare,
            Precision::Extended,
        ]
        .into_iter()
        .zip(case["grids"].as_array().unwrap())
        {
            assert_eq!(
                Locator::holding(position, precision).to_string(),
                expected.as_str().unwrap(),
                "{position:?}"
            );
        }
    }
}
