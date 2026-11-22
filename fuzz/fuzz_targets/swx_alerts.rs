// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
#![forbid(unsafe_code)]
use libfuzzer_sys::fuzz_target;
use qso_graph_spaceweather::{self as swx, Product};

// QGDXC-SWX-0007.
fuzz_target!(|data: &[u8]| {
    if let Ok(value) = swx::alerts(data) {
        assert!(data.len() <= Product::Alerts.limit());
        assert!(value.len() <= 100);
        assert!(
            value
                .windows(2)
                .all(|pair| pair[0].issue_time >= pair[1].issue_time)
        );
    }
});
