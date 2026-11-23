// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
#![forbid(unsafe_code)]
use libfuzzer_sys::fuzz_target;
use qso_graph_pota::parse_activator_spots;

fuzz_target!(|bytes: &[u8]| {
    if let Ok(result) = parse_activator_spots(bytes) {
        assert!(result.spots.len() + result.skipped <= 1_000);
    }
});
