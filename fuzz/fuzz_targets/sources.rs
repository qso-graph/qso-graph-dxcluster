// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
#![forbid(unsafe_code)]
use libfuzzer_sys::fuzz_target;
use qso_graph_dxcluster_cli::saved::{self, MAX_BYTES, MAX_NAME, MAX_SOURCES};
// QGDXC-TUI-0009: arbitrary bytes give a validated list or an error, never a panic.
fuzz_target!(|data: &[u8]| {
    let data = &data[..data.len().min(MAX_BYTES + 1)];
    if let Ok(saved) = saved::parse(data) {
        assert!(data.len() <= MAX_BYTES);
        assert!((1..=MAX_SOURCES).contains(&saved.entries.len()));
        for entry in &saved.entries {
            assert!((1..=MAX_NAME).contains(&entry.name.len()));
            assert!(entry.name.bytes().all(|b| (0x20..=0x7e).contains(&b)));
            assert!(!entry.config.retain_third_party_data);
            assert!(entry.config.validate().is_ok());
        }
    }
});
