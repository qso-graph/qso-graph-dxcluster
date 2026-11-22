// SPDX-License-Identifier: GPL-3.0-or-later
use qso_graph_spaceweather::{self as swx, Alert, AlertFamily, AlertKind, Error, Product, Reason};
use serde_json::{Value, json};

const FLUX: &str = "products_summary_10cm-flux.json";
const FLUX_HISTORY: &str = "json_f107_cm_flux.json";
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
    let daily = swx::solar_flux_history(&fixture(FLUX_HISTORY)).unwrap();
    assert_eq!(daily.len(), 41);
    assert!(
        daily
            .windows(2)
            .all(|w| w[0].time / 86_400 < w[1].time / 86_400)
    );
    let last = daily.last().unwrap();
    assert_eq!(
        (last.time, last.flux, last.ninety_day_mean),
        (flux.time, 99.0, Some(111.0))
    );
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

fn alert_messages(messages: &[(&str, &str)]) -> Vec<Alert> {
    let entries: Vec<_> = messages
        .iter()
        .map(|(id, message)| {
            json!({"product_id": id, "issue_datetime": "2026-11-19 00:00:00", "message": message})
        })
        .collect();
    swx::alerts(&serde_json::to_vec(&entries).unwrap()).unwrap()
}

fn active_serials(alerts: &[Alert], time: i64) -> Vec<u64> {
    swx::in_force(alerts, time)
        .iter()
        .map(|alert| alert.serial.unwrap())
        .collect()
}

#[test]
fn noaa_warnings_and_alerts_are_in_force_in_severity_and_issue_order() {
    // QGDXC-SWX-0009 and QGDXC-SWX-0011.
    let alerts = swx::alerts(&fixture(ALERTS)).unwrap();
    let serial = |n| alerts.iter().find(|a| a.serial == Some(n)).unwrap();
    let original = serial(5456);
    assert_eq!(original.kind, Some(AlertKind::Warning));
    assert_eq!(original.message_issue_time, Some(1_795_048_860));
    assert_eq!(original.valid_from, Some(1_795_048_800));
    assert_eq!(original.valid_to, Some(1_795_057_200));
    assert_eq!(serial(2276).valid_from, Some(1_795_049_700));
    assert_eq!(serial(2698).message_issue_time, Some(1_795_049_160));
    assert_eq!(serial(2063).message_issue_time, Some(1_795_050_480));
    let extension = serial(5457);
    assert_eq!(extension.kind, Some(AlertKind::ExtendedWarning));
    assert_eq!(extension.extended_serial, Some(5456));
    assert_eq!(extension.valid_until, Some(1_795_078_800));

    for (time, expected) in [
        (1_795_048_799, [].as_slice()),             // 00:39:59 UTC.
        (1_795_048_800, &[5456]),                   // 00:40 UTC.
        (1_795_051_079, &[2063, 2698, 2276, 5456]), // 01:17:59 UTC.
        (1_795_051_080, &[2063, 2698, 5457, 2276]), // 01:18 UTC.
        (1_795_051_800, &[2063, 2698, 5457, 2276]), // 01:30 UTC.
        (1_795_057_200, &[2063, 2698, 5457]),       // 03:00 UTC.
        (1_795_059_000, &[2063, 2698, 5457]),       // 03:30 UTC.
        (1_795_059_960, &[2063, 5457]),             // 03:46 UTC.
        (1_795_078_740, &[5457]),                   // 08:59 UTC.
        (1_795_078_800, &[]),                       // 09:00 UTC.
        (1_795_078_860, &[]),                       // 09:01 UTC.
    ] {
        assert_eq!(active_serials(&alerts, time), expected, "{time}");
    }
}

#[test]
fn noaa_watch_includes_the_last_second_of_its_last_storm_day() {
    // QGDXC-SWX-0009 and QGDXC-SWX-0011.
    let alerts = swx::alerts(&fixture(ALERTS)).unwrap();
    let watch = alerts.iter().find(|a| a.serial == Some(1138)).unwrap();
    assert_eq!(watch.kind, Some(AlertKind::Watch));
    assert_eq!(watch.message_issue_time, Some(1_794_749_640));
    let days: Vec<_> = watch
        .watch_days
        .as_ref()
        .unwrap()
        .iter()
        .map(|day| (day.date, day.level.as_str()))
        .collect();
    assert_eq!(
        days,
        [
            (1_794_787_200, "None"),
            (1_794_873_600, "G1"),
            (1_794_960_000, "G1"),
        ]
    );
    assert!(active_serials(&alerts, 1_794_749_639).is_empty());
    assert_eq!(active_serials(&alerts, 1_794_749_640), [1138]);
    assert_eq!(active_serials(&alerts, 1_795_046_399), [1138]);
    assert_eq!(watch.end_time(), Some(1_795_046_399));
    assert!(active_serials(&alerts, 1_795_046_400).is_empty());
}

#[test]
fn alert_families_exclude_noaa_electron_flux_and_type_ii_and_iv_radio() {
    // QGDXC-SWX-0010 and QGDXC-SWX-0011.
    let alerts = swx::alerts(&fixture(ALERTS)).unwrap();
    for name in ["Electron", "Type II Radio", "Type IV Radio"] {
        let alert = alerts
            .iter()
            .find(|alert| {
                alert
                    .headline
                    .as_deref()
                    .is_some_and(|headline| headline.contains(name))
                    && matches!(
                        alert.kind,
                        Some(AlertKind::Alert | AlertKind::ContinuedAlert)
                    )
            })
            .unwrap();
        assert_eq!(alert.family(), AlertFamily::Other, "{name}");
        assert!(
            swx::in_force(
                std::slice::from_ref(alert),
                alert.message_issue_time.unwrap()
            )
            .is_empty()
        );
    }
    for (headline, expected) in [
        ("gEoMaGnEtIc storm", AlertFamily::Geomagnetic),
        ("x-RAY flux", AlertFamily::RadioBlackout),
        ("radio BLACKOUT", AlertFamily::RadioBlackout),
        ("pROTON event", AlertFamily::SolarRadiation),
        ("sOLar RADIATION storm", AlertFamily::SolarRadiation),
    ] {
        let message = format!("  ALERT: {headline}\nIssue Time: 2026 Nov 19 0130 UTC");
        let alerts = alert_messages(&[("QX20", &message)]);
        assert_eq!(alerts[0].family(), expected);
        assert_eq!(swx::in_force(&alerts, 1_795_051_800).len(), 1);
    }
}

#[test]
fn cancellation_ends_its_warning_at_the_message_issue_time() {
    // QGDXC-SWX-0009 and QGDXC-SWX-0011.
    let mut alerts = swx::alerts(&fixture(ALERTS)).unwrap();
    alerts.extend(alert_messages(&[(
        "QX20",
        concat!(
            "CANCEL WARNING: Geomagnetic warning ended\n",
            "Issue Time: 2026 Nov 19 0200 UTC\nCancel Serial Number: 2276",
        ),
    )]));
    let cancel = alerts.last().unwrap();
    assert_eq!(cancel.kind, Some(AlertKind::CancelWarning));
    assert_eq!(cancel.cancelled_serial, Some(2276));
    assert_eq!(
        active_serials(&alerts, 1_795_053_599),
        [2063, 2698, 5457, 2276]
    );
    assert_eq!(active_serials(&alerts, 1_795_053_600), [2063, 2698, 5457]);
}

#[test]
fn watch_days_roll_into_january_and_ignore_trailing_none() {
    // QGDXC-SWX-0009 and QGDXC-SWX-0011.
    let alerts = alert_messages(&[(
        "QX20",
        concat!(
            "WATCH: Geomagnetic storm\nSerial Number: 1\nIssue Time: 2026 Dec 30 1200 UTC\n",
            "Highest Storm Level Predicted by Day:\nDec 31:  G1 (Minor)   Jan 01:  G2 (Moderate)   Jan 02:  None",
        ),
    )]);
    let watch = &alerts[0];
    let dates: Vec<_> = watch
        .watch_days
        .as_ref()
        .unwrap()
        .iter()
        .map(|day| day.date)
        .collect();
    assert_eq!(dates, [1_798_675_200, 1_798_761_600, 1_798_848_000]);
    assert_eq!(active_serials(&alerts, 1_798_847_999), [1]);
    assert!(active_serials(&alerts, 1_798_848_000).is_empty());
}

#[test]
fn newest_issued_watch_per_family_supersedes_the_older_watch() {
    // QGDXC-SWX-0011.
    let alerts = alert_messages(&[
        (
            "QX20",
            concat!(
                "WATCH: Geomagnetic storm\nSerial Number: 1\nIssue Time: 2026 Nov 19 0000 UTC\n",
                "Highest Storm Level Predicted by Day:\nNov 21:  G1",
            ),
        ),
        (
            "QX21",
            concat!(
                "WATCH: Geomagnetic storm\nSerial Number: 2\nIssue Time: 2026 Nov 19 0100 UTC\n",
                "Highest Storm Level Predicted by Day:\nNov 20:  G1",
            ),
        ),
        (
            "QX22",
            concat!(
                "WATCH: Solar Radiation storm\nSerial Number: 3\nIssue Time: 2026 Nov 19 0001 UTC\n",
                "Highest Storm Level Predicted by Day:\nNov 21:  S1",
            ),
        ),
        (
            "QX23",
            concat!(
                "CANCEL WATCH: Geomagnetic storm\nIssue Time: 2026 Nov 19 0200 UTC\n",
                "Cancel Serial Number: 2",
            ),
        ),
    ]);
    assert_eq!(active_serials(&alerts, 1_795_049_999), [3, 1]);
    assert_eq!(active_serials(&alerts, 1_795_050_000), [2, 3]);
    assert_eq!(active_serials(&alerts, 1_795_053_600), [3]);
}

#[test]
fn malformed_optional_alert_metadata_does_not_refuse_the_product() {
    // QGDXC-SWX-0009 and QGDXC-SWX-0011.
    let alerts = alert_messages(&[
        (
            "QX20",
            concat!(
                "WARNING: Geomagnetic storm\nIssue Time: 2026 Nov 19 0130 UTC\n",
                "Serial Number: +42\nValid From: 2026 Nov 19 0000 UTC\nValid To: 2026 Nov 31 0300 UTC",
            ),
        ),
        (
            "QX21",
            "ALERT: Geomagnetic storm\nIssue Time: 2026 Nov 19 2460 UTC",
        ),
        (
            "QX22",
            "SUMMARY: Geomagnetic storm\nIssue Time: 2026 Nov 19 0130 UTC",
        ),
        (
            "QX23",
            concat!(
                "WATCH: Geomagnetic storm\nIssue Time: 2026 Nov 19 0000 UTC\n",
                "Highest Storm Level Predicted by Day:\nNov 31:  G1",
            ),
        ),
        ("QX24", "Issue Time: 2026 Nov 19 0000 UTC"),
    ]);
    assert_eq!(alerts.len(), 5);
    assert_eq!(alerts[0].serial, None);
    assert_eq!(alerts[0].valid_from, Some(1_795_046_400));
    assert_eq!(alerts[0].valid_to, None);
    assert_eq!(alerts[1].message_issue_time, None);
    assert_eq!(alerts[3].watch_days, None);
    assert_eq!(alerts[4].kind, None);
    assert!(swx::in_force(&alerts, 1_795_051_800).is_empty());
}

#[test]
fn flux_history_filters_and_keeps_the_last_entry_per_utc_date() {
    // QGDXC-SWX-0008.
    let records = br#"[
        {"time_tag":"2024-03-01T20:00:00","reporting_schedule":"Noon","frequency":2800,"flux":105,"ninety_day_mean":985},
        {"time_tag":"2024-02-29T20:00:00Z","reporting_schedule":"Noon","frequency":2800,"flux":105,"ninety_day_mean":0},
        {"reporting_schedule":"Morning"},
        {"reporting_schedule":"Noon","frequency":1415},
        {"time_tag":"2024-03-01T17:00:00Z","reporting_schedule":"Noon","frequency":2800,"flux":999,"ninety_day_mean":null}
    ]"#;
    let daily = swx::solar_flux_history(records).unwrap();
    assert_eq!(daily.len(), 2);
    assert_eq!(daily[1].time - daily[0].time, 21 * 3600);
    assert_eq!(
        (daily[0].flux, daily[0].ninety_day_mean),
        (105.0, Some(0.0))
    );
    assert_eq!((daily[1].flux, daily[1].ninety_day_mean), (999.0, None));
}

#[test]
fn flux_history_refuses_invalid_products() {
    // QGDXC-SWX-0002, QGDXC-SWX-0003, QGDXC-SWX-0007 and QGDXC-SWX-0008.
    let product = Product::SolarFluxHistory;
    let cases = [
        (vec![b' '; product.limit() + 1], Reason::ByteLimit),
        (vec![255], Reason::Utf8),
        (b"[".to_vec(), Reason::Json),
        (
            format!("{}0{}", "[".repeat(17), "]".repeat(17)).into_bytes(),
            Reason::Depth,
        ),
        (
            br#"[{"reporting_schedule":"Morning","extra":0,"extra":1}]"#.to_vec(),
            Reason::DuplicateMember,
        ),
        (b"{}".to_vec(), Reason::Type("array")),
        (b"[]".to_vec(), Reason::Empty),
        (
            br#"[{"reporting_schedule":"Morning"}]"#.to_vec(),
            Reason::Empty,
        ),
    ];
    for (bytes, reason) in cases {
        refusal(product, reason, swx::solar_flux_history(&bytes));
    }
    let good = body(FLUX_HISTORY)[1].clone();
    for (key, bad, reason) in [
        (
            "reporting_schedule",
            json!(0),
            Reason::Type("reporting_schedule"),
        ),
        ("frequency", json!("2800"), Reason::Type("frequency")),
        ("time_tag", json!("2025-02-29T20:00:00"), Reason::Time),
        ("time_tag", json!("2024-02-29 20:00:00"), Reason::Time),
        ("flux", json!(true), Reason::Type("flux")),
        ("flux", json!(1000), Reason::Range("flux")),
        (
            "ninety_day_mean",
            json!("111"),
            Reason::Type("ninety_day_mean"),
        ),
        (
            "ninety_day_mean",
            json!(-1),
            Reason::Range("ninety_day_mean"),
        ),
    ] {
        let mut changed = good.clone();
        changed[key] = bad;
        let bytes = json!([good, changed]).to_string();
        refusal(product, reason, swx::solar_flux_history(bytes.as_bytes()));
        changed.as_object_mut().unwrap().remove(key);
        let bytes = json!([good, changed]).to_string();
        refusal(
            product,
            Reason::Missing(key),
            swx::solar_flux_history(bytes.as_bytes()),
        );
    }
    let mut records = vec![json!({"reporting_schedule":"Morning"}); 399];
    records.push(good);
    assert_eq!(
        swx::solar_flux_history(json!(records).to_string().as_bytes())
            .unwrap()
            .len(),
        1
    );
    records.push(json!({"reporting_schedule":"Noon","frequency":1415}));
    refusal(
        product,
        Reason::EntryLimit,
        swx::solar_flux_history(json!(records).to_string().as_bytes()),
    );
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
