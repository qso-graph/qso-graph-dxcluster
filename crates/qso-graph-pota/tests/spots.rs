// SPDX-License-Identifier: GPL-3.0-or-later
use qso_graph_pota::{Error, MAX_BYTES, Spots, parse_activator_spots};
use serde_json::{Value, json};

fn entry() -> Value {
    let activator = ["Q", "1", "ABC"].concat();
    let spotter = ["Q", "9", "XYZ-#"].concat();
    json!({
        "spotId": 1,
        "activator": activator,
        "spotter": spotter,
        "frequency": "14074.123",
        "reference": "US-9990",
        "spotTime": "2000-02-29T23:59:59.123456Z"
    })
}

fn parse(values: &[Value]) -> Spots {
    parse_activator_spots(&serde_json::to_vec(values).unwrap()).unwrap()
}

#[test]
fn limits_refuse_only_the_whole_snapshot() {
    let too_deep = format!("{}{}", "[".repeat(17), "]".repeat(17));
    let deep = format!("{}{}", "[".repeat(16), "]".repeat(16));
    let too_many = format!("[{}null]", "null,".repeat(1_000));
    let many = format!("[{}null]", "null,".repeat(999));
    assert_eq!(parse_activator_spots(deep.as_bytes()).unwrap().skipped, 1);
    assert_eq!(
        parse_activator_spots(many.as_bytes()).unwrap().skipped,
        1_000
    );
    assert_eq!((parse_activator_spots(b"[]").unwrap().spots.len()), 0);
    let mut largest = b"[]".to_vec();
    largest.resize(MAX_BYTES, b' ');
    assert!(parse_activator_spots(&largest).is_ok());
    largest.push(b' ');
    let escaped_key = [r#"[{"ignored":{"same":1,"s\u"#, "0061", r#"me":2}}]"#].concat();
    let refusals: [(&[u8], Error); 10] = [
        (&largest, Error::TooLarge),
        (b"[\xff]", Error::Utf8),
        (b"[0,]", Error::Json),
        (b"[] []", Error::Json),
        (br#"[{"spotId":1, "spotId":1}]"#, Error::DuplicateMember),
        (escaped_key.as_bytes(), Error::DuplicateMember),
        (
            br#"[{"ignored":[{"same":1,"same":2}]}]"#,
            Error::DuplicateMember,
        ),
        (too_deep.as_bytes(), Error::Depth),
        (b"{}", Error::NotArray),
        (too_many.as_bytes(), Error::TooManyEntries),
    ];
    for (bytes, error) in refusals {
        assert_eq!(parse_activator_spots(bytes), Err(error), "{error:?}");
    }
}

#[test]
fn skips_entries_missing_any_required_member() {
    for key in entry().as_object().unwrap().keys() {
        let mut bad = entry();
        bad.as_object_mut().unwrap().remove(key);
        let result = parse(&[bad, entry()]);
        assert_eq!((result.spots.len(), result.skipped), (1, 1), "{key}");
    }
}

#[test]
fn skips_invalid_required_members() {
    let invalid = [
        ("spotId", json!(0)),
        ("spotId", json!(-1)),
        ("spotId", json!(0.5)),
        ("spotId", json!("1")),
        ("spotId", json!(9_007_199_254_740_993_u64)),
        ("activator", json!("")),
        ("activator", json!(["Q", "1", "!"].concat())),
        ("activator", json!(["Q", "1", "é"].concat())),
        ("activator", json!(["Q1", &"A".repeat(31)].concat())),
        ("spotter", json!(null)),
        ("spotter", json!("")),
        ("spotter", json!(["Q", "9", " X"].concat())),
        ("spotter", json!(["Q9", &"Z".repeat(31)].concat())),
        ("frequency", json!(14_074)),
        ("frequency", json!("")),
        ("frequency", json!("0")),
        ("frequency", json!("300000000.001")),
        ("frequency", json!("99999999999999999999999")),
        ("frequency", json!("1.2345")),
        ("frequency", json!("1.2.3")),
        ("frequency", json!("1.")),
        ("frequency", json!("-1")),
        ("frequency", json!("2e3")),
        ("reference", json!(4)),
        ("reference", json!("us-1000")),
        ("reference", json!("-1000")),
        ("reference", json!("ABCDE-1000")),
        ("reference", json!("US-100")),
        ("reference", json!("US-100000")),
        ("reference", json!("US-12_0")),
        ("spotTime", json!(0)),
        ("spotTime", json!("1999-12-31T23:59:59Z")),
        ("spotTime", json!("2100-01-01T00:00:00Z")),
        ("spotTime", json!("2001-02-29T00:00:00Z")),
        ("spotTime", json!("2000-04-31T00:00:00Z")),
        ("spotTime", json!("2000-00-01T00:00:00Z")),
        ("spotTime", json!("2000-13-01T00:00:00Z")),
        ("spotTime", json!("2000-01-00T00:00:00Z")),
        ("spotTime", json!("2000-01-01T24:00:00Z")),
        ("spotTime", json!("2000-01-01T00:60:00Z")),
        ("spotTime", json!("2000-01-01T00:00:60Z")),
        ("spotTime", json!("2000-01-01T00:00:00.Z")),
        ("spotTime", json!("2000-01-01T00:00:00.1234567Z")),
        ("spotTime", json!("2000-01-01T00:00:00.xZ")),
        ("spotTime", json!("2000-01-01T00:00:00+00:00")),
        ("spotTime", json!("2000-01-01 00:00:00Z")),
    ];
    for (key, value) in invalid {
        let mut bad = entry();
        bad[key] = value;
        let result = parse(&[bad.clone(), entry()]);
        assert_eq!(
            (result.spots.len(), result.skipped),
            (1, 1),
            "{key}: {}",
            bad[key]
        );
    }
    for bad in [Value::Null, json!([]), json!(1), json!("text")] {
        assert_eq!(parse(&[bad, entry()]).skipped, 1);
    }
}

#[test]
fn keeps_valid_bounds_and_omits_invalid_optional_members() {
    let mut valid = entry();
    valid["spotId"] = json!(9_007_199_254_740_992_u64);
    valid["activator"] = json!(["Q1/", &"A".repeat(29)].concat());
    valid["frequency"] = json!("300000000");
    valid["reference"] = json!("A123-12345");
    valid["mode"] = json!("FT8");
    valid["name"] = json!("é".repeat(100));
    valid["comments"] = json!("x".repeat(512));
    valid["source"] = json!("rBn");
    valid["locationDesc"] = json!("x".repeat(64));
    valid["grid4"] = json!("ar09");
    valid["grid6"] = json!(["ar09", "aa"].concat());
    valid["latitude"] = json!(-90);
    valid["longitude"] = json!(180);
    valid["count"] = json!(100_000);
    let result = parse(&[valid]);
    let spot = &result.spots[0];
    assert_eq!(spot.spot_id, 1 << 53);
    assert_eq!(spot.activator.len(), 32);
    assert_eq!(spot.frequency_hz, 300_000_000_000);
    assert_eq!(spot.reference, "A123-12345");
    assert_eq!(spot.spot_time, 951_868_799);
    assert_eq!(spot.mode.as_deref(), Some("FT8"));
    assert_eq!(spot.name.as_ref().map(String::len), Some(200));
    assert_eq!(spot.comments.as_ref().map(String::len), Some(512));
    assert!(spot.is_skimmer());
    assert_eq!(spot.location_desc.as_ref().map(String::len), Some(64));
    assert_eq!(spot.grid4.as_deref(), Some("ar09"));
    // QGDXC-POTA-0003: coordinates are never kept, valid or not.
    assert_eq!(spot.count, Some(100_000));

    let mut invalid = entry();
    invalid["frequency"] = json!("0.001");
    invalid["mode"] = json!("bad\tmode");
    invalid["name"] = json!("é".repeat(101));
    invalid["comments"] = json!("x".repeat(513));
    invalid["source"] = json!("x".repeat(65));
    invalid["locationDesc"] = json!(17);
    invalid["grid4"] = json!("az09");
    invalid["latitude"] = json!(90.1);
    invalid["longitude"] = json!(-181);
    invalid["count"] = json!(100_001);
    invalid["grid6"] = json!(["ar09", "aa"].concat());
    let result = parse(&[invalid]);
    let spot = &result.spots[0];
    assert_eq!(result.skipped, 0);
    assert_eq!(spot.frequency_hz, 1);
    assert_eq!(spot.mode, None);
    assert_eq!(spot.name, None);
    assert_eq!(spot.comments, None);
    assert_eq!(spot.source, None);
    assert_eq!(spot.location_desc, None);
    assert_eq!(spot.grid4, None);
    assert_eq!(spot.count, None);
    assert!(!spot.is_skimmer());
    let mut non_rbn = entry();
    non_rbn["source"] = json!("RBN relay");
    assert!(!parse(&[non_rbn]).spots[0].is_skimmer());
}

#[test]
fn handles_json_numbers_without_rounding_into_valid_spots() {
    let text = serde_json::to_string(&entry()).unwrap();
    let add = |member: &str| format!(",{member}}}");
    let text = text.replace(
        '}',
        &add(r#""ignored":1e9000, "latitude":90.00000000000000001"#),
    );
    let bytes = format!("[{text}]");
    let result = parse_activator_spots(bytes.as_bytes()).unwrap();
    assert_eq!(result.skipped, 0);
    for (number, valid) in [
        ("9007199254740992.01", false),
        ("1.00000000000000001", false),
        ("1e9000", false),
        ("1e-9000", false),
        ("1.0", true),
        ("90071992547409920e-1", true),
    ] {
        let bytes = bytes.replace(r#""spotId":1"#, &format!(r#""spotId":{number}"#));
        assert_eq!(
            parse_activator_spots(bytes.as_bytes()).unwrap().skipped,
            usize::from(!valid),
            "{number}"
        );
    }
}

#[test]
fn utc_seconds_use_reported_time_and_discard_fraction() {
    for (text, expected) in [
        ("2000-01-01T00:00:00", 946_684_800),
        ("2000-01-01T00:00:00.9", 946_684_800),
        ("2001-01-01T00:00:00Z", 978_307_200),
        ("2099-12-31T23:59:59Z", 4_102_444_799),
    ] {
        let mut row = entry();
        row["spotTime"] = json!(text);
        assert_eq!(parse(&[row]).spots[0].spot_time, expected);
    }
}
