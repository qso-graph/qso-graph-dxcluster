// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
#![forbid(unsafe_code)]
use libfuzzer_sys::fuzz_target;
use qso_graph_spaceweather::{self as swx, Product};

// QGDXC-SWX-0007.
fuzz_target!(|data: &[u8]| {
    if let Ok(value) = swx::xrays(data) {
        assert!(data.len() <= Product::Xrays.limit());
        assert!((1..=1000).contains(&value.len()));
        assert!(value.windows(2).all(|pair| pair[0].time < pair[1].time));
        for sample in value {
            assert!(!swx::xray_class(sample.flux).is_empty());
        }
    }
});
