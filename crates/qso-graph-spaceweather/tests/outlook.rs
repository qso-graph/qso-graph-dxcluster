// SPDX-License-Identifier: GPL-3.0-or-later
use qso_graph_spaceweather::{
    BandOutlook, BandStatus,
    BandStatus::{Excellent, Fair, Good, Poor},
    OutlookError, band_outlook, planetary_k, solar_flux,
};
use serde_json::Value;

const BANDS: [&str; 10] = [
    "160m", "80m", "40m", "30m", "20m", "17m", "15m", "12m", "10m", "6m",
];

fn fixture(name: &str) -> Vec<u8> {
    let directory = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/outlook");
    std::fs::read(std::path::Path::new(directory).join(name)).unwrap()
}

fn check_bands(actual: [BandOutlook; 10], expected: &Value) {
    assert_eq!(actual.map(|entry| entry.band), BANDS);
    assert_eq!(expected.as_object().unwrap().len(), actual.len());
    for BandOutlook {
        band,
        status,
        comment,
    } in actual
    {
        let text = format!("{} — {comment}", status.label());
        let expected = expected[band].as_str().unwrap();
        assert!(
            text.eq_ignore_ascii_case(expected),
            "{band}: expected {expected}, got {text}"
        );
    }
}

#[test]
fn band_outlook_matches_desktop_sweep() {
    let sweep: Value = serde_json::from_slice(&fixture("desktop-agreement.json")).unwrap();
    let cases = sweep["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 368);
    for case in cases {
        let sfi = case["sfi"].as_f64().unwrap();
        let kp = case["kp"].as_f64().unwrap();
        check_bands(band_outlook(sfi, kp).unwrap(), &case["bands"]);
    }
}

#[test]
fn band_outlook_matches_recorded_quiet_and_storm_products() {
    for set in ["quiet", "storm"] {
        let expected: Value =
            serde_json::from_slice(&fixture(&format!("{set}/expected-bands.json"))).unwrap();
        let sfi = solar_flux(&fixture(&format!("{set}/10cm-flux.json")))
            .unwrap()
            .flux;
        let k = planetary_k(&fixture(&format!("{set}/noaa-planetary-k-index.json"))).unwrap();
        let kp = k.last().unwrap().kp;
        assert!(
            sfi == expected["sfi"].as_f64().unwrap(),
            "recorded SFI differs"
        );
        assert!(
            kp == expected["kp"].as_f64().unwrap(),
            "newest recorded Kp differs"
        );
        check_bands(band_outlook(sfi, kp).unwrap(), &expected["bands"]);
    }
}

fn expect_status(outlook: &[BandOutlook; 10], band: &str, status: BandStatus) {
    let entry = outlook.iter().find(|entry| entry.band == band).unwrap();
    assert_eq!(entry.status, status, "wrong grade for {band}");
}

fn check_sfi_edge(sfi: f64, changes: &[(&str, BandStatus, BandStatus)]) {
    let below = band_outlook(sfi.next_down(), 0.0).unwrap();
    let at = band_outlook(sfi, 0.0).unwrap();
    let above = band_outlook(sfi.next_up(), 0.0).unwrap();
    for &(band, lower, higher) in changes {
        expect_status(&below, band, lower);
        expect_status(&at, band, higher);
        expect_status(&above, band, higher);
    }
}

#[test]
fn band_outlook_sfi_80() {
    check_sfi_edge(80.0, &[("15m", Poor, Fair)]);
}

#[test]
fn band_outlook_sfi_90() {
    check_sfi_edge(90.0, &[("17m", Poor, Fair)]);
}

#[test]
fn band_outlook_sfi_100() {
    check_sfi_edge(
        100.0,
        &[
            ("20m", Fair, Good),
            ("15m", Fair, Good),
            ("10m", Poor, Fair),
        ],
    );
}

#[test]
fn band_outlook_sfi_120() {
    check_sfi_edge(120.0, &[("17m", Fair, Good), ("12m", Poor, Fair)]);
}

#[test]
fn band_outlook_sfi_130() {
    check_sfi_edge(130.0, &[("10m", Fair, Good)]);
}

#[test]
fn band_outlook_sfi_140() {
    check_sfi_edge(140.0, &[("15m", Good, Excellent)]);
}

#[test]
fn band_outlook_sfi_150() {
    check_sfi_edge(150.0, &[("20m", Good, Excellent), ("12m", Fair, Good)]);
}

#[test]
fn band_outlook_sfi_160() {
    check_sfi_edge(160.0, &[("10m", Good, Excellent)]);
}

#[test]
fn band_outlook_kp_3() {
    expect_status(
        &band_outlook(0.0, 3.0_f64.next_down()).unwrap(),
        "160m",
        Good,
    );
    for kp in [3.0, 3.0_f64.next_up()] {
        expect_status(&band_outlook(0.0, kp).unwrap(), "160m", Fair);
    }
}

#[test]
fn band_outlook_kp_4() {
    expect_status(
        &band_outlook(0.0, 4.0_f64.next_down()).unwrap(),
        "80m",
        Good,
    );
    for kp in [4.0, 4.0_f64.next_up()] {
        expect_status(&band_outlook(0.0, kp).unwrap(), "80m", Fair);
    }
}

#[test]
fn band_outlook_kp_5_preserves_the_sfi_grades() {
    let below = band_outlook(100.0, 5.0_f64.next_down()).unwrap();
    for (band, status) in [("160m", Fair), ("80m", Fair), ("40m", Good)] {
        expect_status(&below, band, status);
    }
    for kp in [5.0, 5.0_f64.next_up()] {
        let storm = band_outlook(100.0, kp).unwrap();
        for (band, status) in [("160m", Poor), ("80m", Poor), ("40m", Fair)] {
            expect_status(&storm, band, status);
        }
        for (quiet, storm) in below[4..9].iter().zip(&storm[4..9]) {
            assert_eq!(storm.status, quiet.status, "storm grade for {}", storm.band);
            assert_ne!(quiet.comment, "Degraded by geomagnetic storm");
            assert_eq!(storm.comment, "Degraded by geomagnetic storm");
        }
        assert_eq!(storm[3], below[3]);
        assert_eq!(storm[9], below[9]);
    }
}

#[test]
fn band_outlook_refuses_nonfinite_and_out_of_range_inputs() {
    for value in [
        f64::NAN,
        f64::NEG_INFINITY,
        f64::INFINITY,
        0.0_f64.next_down(),
    ] {
        assert_eq!(band_outlook(value, 0.0), Err(OutlookError::InvalidSfi));
        assert_eq!(band_outlook(0.0, value), Err(OutlookError::InvalidKp));
    }
    assert_eq!(
        band_outlook(999.0_f64.next_up(), 0.0),
        Err(OutlookError::InvalidSfi)
    );
    assert_eq!(
        band_outlook(0.0, 9.0_f64.next_up()),
        Err(OutlookError::InvalidKp)
    );
    for (sfi, kp) in [(0.0, 0.0), (999.0, 9.0)] {
        assert!(band_outlook(sfi, kp).is_ok());
    }
}
