// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
#![forbid(unsafe_code)]
use libfuzzer_sys::fuzz_target;
use qso_graph_dxcluster_cli::prefixes::{MAX_BYTES, Prefixes};
// QGDXC-TUI-0083: arbitrary bytes give a bounded table or an error, never a panic, and
// every lookup returns a place on the globe.
fuzz_target!(|data: &[u8]| {
    let data = &data[..data.len().min(MAX_BYTES + 1)];
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let split = text.len() / 2;
    let (table, calls) = text.split_at(text.floor_char_boundary(split));
    if let Ok(t) = Prefixes::parse(table) {
        for call in calls.split(['\n', ' ']).take(64) {
            if let Some(p) = t.lookup(call) {
                assert!(p.lat.abs() <= 90.0 && p.lon.abs() <= 180.0);
            }
        }
    }
    let _ = Prefixes::parse(text);
});
