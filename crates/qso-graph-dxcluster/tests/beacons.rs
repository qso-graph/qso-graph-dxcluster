// SPDX-License-Identifier: GPL-3.0-or-later
//! Annex G: the NCDXF/IARU beacon schedule and beacon reports.
use qso_graph_dxcluster::beacons::{BEACONS, FREQUENCIES, identify, on_air, report, seconds_left};
use qso_graph_dxcluster::{Event, Profile, Value};

#[test]
fn beacon_schedule_follows_the_published_rotation() {
    // QGDXC-BCN-0001
    assert_eq!(BEACONS.len(), 18);
    assert_eq!(
        FREQUENCIES,
        [
            (14_100_000, "20m"),
            (18_110_000, "17m"),
            (21_150_000, "15m"),
            (24_930_000, "12m"),
            (28_200_000, "10m"),
        ]
    );
    // The first beacon opens each cycle on 14.100 and steps up one band every 10 s.
    for band in 0..5 {
        assert_eq!(on_air(band as u64 * 10, band), 0);
        assert_eq!(on_air(band as u64 * 10 + 9, band), 0);
    }
    assert_eq!(on_air(0, 1), 17);
    assert_eq!(on_air(10, 0), 1);
    assert_eq!(on_air(179, 0), 17);
    assert_eq!(on_air(180, 0), 0);
    // Every cycle starts on a UTC minute divisible by three.
    assert_eq!(on_air(1_700_000_100, 0), on_air(1_700_000_100 % 180, 0));
    // Each band carries every beacon exactly once per three minutes, and two bands
    // never carry the same beacon at once.
    for band in 0..5 {
        let mut seen: Vec<_> = (0..18).map(|slot| on_air(slot * 10, band)).collect();
        seen.sort();
        assert_eq!(seen, (0..18).collect::<Vec<_>>());
    }
    for t in (0..180).step_by(10) {
        let mut now: Vec<_> = (0..5).map(|b| on_air(t, b)).collect();
        now.sort();
        now.dedup();
        assert_eq!(now.len(), 5);
    }
    assert_eq!(
        (seconds_left(0), seconds_left(9), seconds_left(10)),
        (10, 1, 10)
    );
    assert!(
        BEACONS
            .iter()
            .all(|b| !b.call.is_empty() && !b.place.is_empty())
    );
}

fn spot(call: &str, hz: i64, spot_type: &str) -> Event {
    let mut event = Event::new(Profile::Rbn, "spot");
    event.set("call", call);
    event.set("frequency_hz", Value::Integer(hz));
    event.set("spot_type", spot_type);
    event
}

#[test]
fn beacon_reports_are_identified_by_type_call_and_frequency() {
    // QGDXC-BCN-0002
    let call = BEACONS[3].call;
    let lower = call.to_ascii_lowercase();
    assert_eq!(report(&spot(call, 21_150_000, "NCDXF B")), Some((3, 2)));
    assert_eq!(report(&spot(&lower, 21_151_000, "NCDXF B")), Some((3, 2)));
    assert_eq!(report(&spot(call, 21_149_000, "NCDXF B")), Some((3, 2)));
    assert_eq!(identify(&lower, 28_200_000), Some((3, 4)));
    // Off frequency, another type, another call, another kind, or no frequency.
    assert_eq!(report(&spot(call, 21_151_001, "NCDXF B")), None);
    assert_eq!(report(&spot(call, 21_150_000, "BEACON")), None);
    assert_eq!(report(&spot(call, 21_150_000, "CQ")), None);
    assert_eq!(report(&spot("Q1ABC", 21_150_000, "NCDXF B")), None);
    let mut talk = spot(call, 21_150_000, "NCDXF B");
    talk.kind = "talk";
    assert_eq!(report(&talk), None);
    let mut text = spot(call, 21_150_000, "NCDXF B");
    text.set("frequency_hz", "21150000");
    assert_eq!(report(&text), None);
    let mut negative = spot(call, 21_150_000, "NCDXF B");
    negative.set("frequency_hz", Value::Integer(-21_150_000));
    assert_eq!(report(&negative), None);
}
