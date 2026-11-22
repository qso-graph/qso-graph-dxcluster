// SPDX-License-Identifier: GPL-3.0-or-later
use qso_graph_spaceweather::{self as swx, Error, Product, Reason};
use serde_json::{Value, json};

const FLUX: &str = "products_summary_10cm-flux.json";
const KP: &str = "products_noaa-planetary-k-index.json";
const SCALES: &str = "products_noaa-scales.json";
const XRAYS: &str = "json_goes_primary_xrays-6-hour.json";
const SPEED: &str = "products_summary_solar-wind-speed.json";
const FIELD: &str = "products_summary_solar-wind-mag-field.json";
const ALERTS: &str = "products_alerts.json";

fn fixture(name: &str) -> Vec<u8> {
    let directory = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
    std::fs::read(std::path::Path::new(directory).join(name)).unwrap()
}

#[test]
fn parses_all_noaa_fixtures() {
    // QGDXC-SWX-0001 and QGDXC-SWX-0007.
    let flux = swx::solar_flux(&fixture(FLUX)).unwrap();
    assert_eq!((flux.flux, flux.time), (99.0, 1_795_291_200));
    let kp = swx::planetary_k(&fixture(KP)).unwrap();
    assert_eq!(kp.len(), 62);
    assert_eq!(kp.first().unwrap().kp, 2.0);
    assert_eq!(kp.first().unwrap().a_running, 7.0);
    assert!(kp.windows(2).all(|w| w[0].time < w[1].time));
    let scales = swx::scales(&fixture(SCALES)).unwrap();
    assert_eq!((scales.r, scales.s, scales.g), (0, 0, 0));
    assert_eq!(scales.time, 1_795_376_460);
    let rays = swx::xrays(&fixture(XRAYS)).unwrap();
    assert_eq!(rays.len(), 358);
    assert_eq!(swx::xray_class(rays.last().unwrap().flux), "B2.3");
    assert!(rays.windows(2).all(|w| w[0].time < w[1].time));
    assert_eq!(
        swx::wind_speed(&fixture(SPEED)).unwrap().proton_speed,
        343.0
    );
    let field = swx::wind_field(&fixture(FIELD)).unwrap();
    assert_eq!((field.bt, field.bz_gsm), (2.0, -2.0));
    let alerts = swx::alerts(&fixture(ALERTS)).unwrap();
    assert_eq!(alerts.len(), 63);
    assert!(
        alerts
            .windows(2)
            .all(|w| w[0].issue_time >= w[1].issue_time)
    );
    assert!(alerts.iter().any(|alert| alert.headline.is_some()));
}

fn refusal(product: Product, reason: Reason, result: Result<impl std::fmt::Debug, Error>) {
    let error = result.unwrap_err();
    assert!(error.to_string().contains(product.name()));
    assert_eq!(error, Error { product, reason });
}

fn body(name: &str) -> Value {
    serde_json::from_slice(&fixture(name)).unwrap()
}

#[test]
fn rejects_bad_json_and_kept_values_without_partial_results() {
    // QGDXC-SWX-0002 and QGDXC-SWX-0007.
    let product = Product::SolarFlux;
    for (data, reason) in [
        (vec![b' '; product.limit() + 1], Reason::ByteLimit),
        (vec![255], Reason::Utf8),
        (b"{} {}".to_vec(), Reason::Json),
        (
            br#"[{"time_tag":"2000-01-01T00:00:00Z","flux":99,"extra":{"x":0,"\u0078":1}}]"#
                .to_vec(),
            Reason::DuplicateMember,
        ),
        (
            br#"[{"time_tag":"2000-01-01T00:00:00Z"}]"#.to_vec(),
            Reason::Missing("flux"),
        ),
        (
            br#"[{"time_tag":"2000-01-01T00:00:00Z","flux":"99"}]"#.to_vec(),
            Reason::Type("flux"),
        ),
        (
            br#"[{"time_tag":"2000-01-01T00:00:00Z","flux":1000}]"#.to_vec(),
            Reason::Range("flux"),
        ),
    ] {
        refusal(product, reason, swx::solar_flux(&data));
    }
    for depth in [14, 15] {
        let data = format!(
            r#"[{{"time_tag":"2000-01-01T00:00:00Z","flux":99,"extra":{}null{}}}]"#,
            "[".repeat(depth),
            "]".repeat(depth)
        );
        if depth == 14 {
            assert!(swx::solar_flux(data.as_bytes()).is_ok());
        } else {
            refusal(product, Reason::Depth, swx::solar_flux(data.as_bytes()));
        }
    }
    refusal(Product::SolarFlux, Reason::Empty, swx::solar_flux(b"[]"));
    let records = json!(vec![body(FLUX)[0].clone(); 2]);
    refusal(
        Product::SolarFlux,
        Reason::EntryLimit,
        swx::solar_flux(records.to_string().as_bytes()),
    );
    refusal(Product::PlanetaryK, Reason::Empty, swx::planetary_k(b"[]"));
    refusal(
        Product::Xrays,
        Reason::Empty,
        swx::xrays(br#"[{"energy":"other"}]"#),
    );
    assert!(swx::alerts(b"[]").unwrap().is_empty());

    let large_kp = json!(vec![body(KP)[0].clone(); 201]).to_string();
    refusal(
        Product::PlanetaryK,
        Reason::EntryLimit,
        swx::planetary_k(large_kp.as_bytes()),
    );
    let ray = json!({"energy":"0.1-0.8nm", "time_tag":"2000-01-01T00:00:00Z","flux":1e-7});
    let large_rays = json!(vec![ray.clone(); 1001]).to_string();
    refusal(
        Product::Xrays,
        Reason::EntryLimit,
        swx::xrays(large_rays.as_bytes()),
    );
    let small_alert =
        json!({"product_id":"QX01","issue_datetime":"2000-01-01 00:00:00","message":""});
    let large_alerts = json!(vec![small_alert; 101]).to_string();
    refusal(
        Product::Alerts,
        Reason::EntryLimit,
        swx::alerts(large_alerts.as_bytes()),
    );

    let mut kp = body(KP);
    kp[1]["Kp"] = json!(9.1);
    refusal(
        Product::PlanetaryK,
        Reason::Range("Kp"),
        swx::planetary_k(kp.to_string().as_bytes()),
    );
    let mut scale = body(SCALES);
    scale["0"]["R"]["Scale"] = json!("6");
    refusal(
        Product::Scales,
        Reason::Range("Scale"),
        swx::scales(scale.to_string().as_bytes()),
    );
    let mut ray = ray;
    ray["flux"] = json!(1e-11);
    refusal(
        Product::Xrays,
        Reason::Range("flux"),
        swx::xrays(json!([ray]).to_string().as_bytes()),
    );
    let mut speed = body(SPEED);
    speed[0]["proton_speed"] = json!(-1);
    refusal(
        Product::WindSpeed,
        Reason::Range("proton_speed"),
        swx::wind_speed(speed.to_string().as_bytes()),
    );
    let mut field = body(FIELD);
    field[0]["bz_gsm"] = json!(-201);
    refusal(
        Product::WindField,
        Reason::Range("bz_gsm"),
        swx::wind_field(field.to_string().as_bytes()),
    );

    let mut at_limit = fixture(FLUX);
    at_limit.resize(product.limit(), b' ');
    assert_eq!(swx::solar_flux(&at_limit), swx::solar_flux(&fixture(FLUX)));
}

#[test]
fn validates_calendar_and_timestamp_shapes() {
    // QGDXC-SWX-0003.
    let mut flux = body(FLUX);
    for (stamp, expected) in [
        ("2000-01-01T00:00:00Z", 946_684_800),
        ("2000-02-29T23:59:59", 951_868_799),
        ("2099-12-31T23:59:59Z", 4_102_444_799),
    ] {
        flux[0]["time_tag"] = json!(stamp);
        assert_eq!(
            swx::solar_flux(flux.to_string().as_bytes()).unwrap().time,
            expected
        );
    }
    for text in [
        "1999-12-31T23:59:59",
        "2100-01-01T00:00:00",
        "2001-02-29T00:00:00",
        "2000-00-01T00:00:00",
        "2000-04-31T00:00:00",
        "2000-01-00T00:00:00",
        "2000-01-01T24:00:00",
        "2000-01-01T00:60:00",
        "2000-01-01T00:00:60",
        "2000-01-01T00:00:00+00:00",
        "2000-01-01T00:00:00.1",
        "2000-01-01 00:00:00",
        "2000-01-01T00:00:00z",
        "2000-1-01T00:00:00",
    ] {
        flux[0]["time_tag"] = json!(text);
        refusal(
            Product::SolarFlux,
            Reason::Time,
            swx::solar_flux(flux.to_string().as_bytes()),
        );
    }
    let mut scale = body(SCALES);
    scale["0"]["TimeStamp"] = json!("00:00:00Z");
    refusal(
        Product::Scales,
        Reason::Time,
        swx::scales(scale.to_string().as_bytes()),
    );
    for value in [
        "2000-01-01 00:00:00.1234567",
        "2000-01-01 00:00:00.",
        "2000-01-01T00:00:00Z",
    ] {
        let item = json!([{"product_id":"QX01","issue_datetime":value,"message":""}]);
        refusal(
            Product::Alerts,
            Reason::Time,
            swx::alerts(item.to_string().as_bytes()),
        );
    }
}

#[test]
fn time_order_and_last_duplicate_win() {
    // QGDXC-SWX-0004.
    let first = "2026-11-22T00:00:00";
    let last = "2026-11-22T03:00:00Z";
    let kp = json!([
        {"time_tag":last,"Kp":2,"a_running":6},
        {"time_tag":first,"Kp":3,"a_running":7},
        {"time_tag":last,"Kp":4,"a_running":8},
    ]);
    let points = swx::planetary_k(kp.to_string().as_bytes()).unwrap();
    assert_eq!(
        points.iter().map(|pt| pt.kp).collect::<Vec<_>>(),
        [3.0, 4.0]
    );
    let rays = json!([
        {"time_tag":last,"flux":1e-7,"energy":"0.1-0.8nm"},
        {"time_tag":first,"flux":2e-7,"energy":"0.1-0.8nm"},
        {"energy":"0.05-0.4nm","flux":"ignored"},
        {"time_tag":last,"flux":3e-7,"energy":"0.1-0.8nm"},
    ]);
    let points = swx::xrays(rays.to_string().as_bytes()).unwrap();
    assert_eq!(
        points.iter().map(|pt| pt.flux).collect::<Vec<_>>(),
        [2e-7, 3e-7]
    );
}

#[test]
fn classes_use_thresholds_and_truncate() {
    // QGDXC-SWX-0005.
    for (value, class) in [
        (1e-10, "A0.0"),
        (1e-8, "A1.0"),
        (9.999e-8, "A9.9"),
        (1e-7, "B1.0"),
        (9.999e-7, "B9.9"),
        (1e-6, "C1.0"),
        (2.57e-6, "C2.5"),
        (9.999e-6, "C9.9"),
        (1e-5, "M1.0"),
        (9.999e-5, "M9.9"),
        (1e-4, "X1.0"),
        (1.23e-3, "X12.3"),
    ] {
        assert_eq!(swx::xray_class(value), class);
    }
}

#[test]
fn alerts_preserve_headlines_validate_lengths_and_sort_fractional_seconds() {
    // QGDXC-SWX-0006.
    let real_format = ["Q", "20", "A"].concat();
    let alerts = json!([
        {"product_id":real_format,"issue_datetime":"2026-11-22 01:00:00.1","message":"Header\r\n  WARNING: first\r\nALERT: second"},
        {"product_id":"QX02","issue_datetime":"2026-11-22 01:00:00.000001","message":"\tWATCH: ignored"},
        {"product_id":"QX03","issue_datetime":"2026-11-22 01:00:00.100001","message":"WATCH: newest"},
    ]);
    let parsed = swx::alerts(alerts.to_string().as_bytes()).unwrap();
    assert_eq!(
        parsed.iter().map(|a| a.issue_time).collect::<Vec<_>>(),
        [1_795_309_200; 3]
    );
    assert_eq!(parsed[0].headline.as_deref(), Some("WATCH: newest"));
    assert_eq!(
        (&parsed[1].product_id, parsed[1].headline.as_deref()),
        (&real_format, Some("  WARNING: first"))
    );
    assert_eq!(parsed[2].headline, None);
    let mut alerts = alerts;
    for prefix in [
        "ALERT:",
        "WARNING:",
        "WATCH:",
        "SUMMARY:",
        "CANCEL WARNING:",
        "CANCEL WATCH:",
        "EXTENDED WARNING:",
        "CONTINUED ALERT:",
    ] {
        let message = format!("{prefix} test");
        alerts[0]["message"] = json!(message);
        assert_eq!(
            swx::alerts(alerts.to_string().as_bytes()).unwrap()[1]
                .headline
                .as_deref(),
            Some(message.as_str())
        );
    }
    alerts[0]["message"] = json!("a".repeat(16 * 1024));
    assert!(swx::alerts(alerts.to_string().as_bytes()).is_ok());
    alerts[0]["message"] = json!("a".repeat(16 * 1024 + 1));
    refusal(
        Product::Alerts,
        Reason::MessageLimit,
        swx::alerts(alerts.to_string().as_bytes()),
    );
    alerts[0]["message"] = json!("");
    for id in ["", "ABCDEFGHIJKLMNOPQ", "QX01\n", "QX★"] {
        alerts[0]["product_id"] = json!(id);
        refusal(
            Product::Alerts,
            Reason::Range("product_id"),
            swx::alerts(alerts.to_string().as_bytes()),
        );
    }
}
