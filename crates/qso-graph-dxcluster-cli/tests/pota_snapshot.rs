// SPDX-License-Identifier: GPL-3.0-or-later
use qso_graph_pota::parse_activator_spots;

#[test]
fn activator_fixture_maps_to_typed_spots() {
    let fixture = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../qso-graph-pota/tests/fixtures/spot-activator.json",
    ))
    .unwrap();
    let result = parse_activator_spots(&fixture).unwrap();
    assert_eq!((result.spots.len(), result.skipped), (58, 0));
    assert_eq!(result.spots.iter().filter(|s| s.is_skimmer()).count(), 20);
    for spot in &result.spots {
        assert_eq!(spot.is_skimmer(), spot.source.as_deref() == Some("RBN"));
    }
    let ft8 = &result.spots[0];
    assert_eq!(ft8.mode.as_deref(), Some("FT8"));
    assert_eq!(ft8.frequency_hz, 14_074_000);
    assert_eq!(ft8.reference, "US-9990");
    assert_eq!(ft8.spot_time, 1_795_395_243);
}
