// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
#![forbid(unsafe_code)]
use libfuzzer_sys::fuzz_target;
use qso_graph_spaceweather::{self as swx, Product};

// QGDXC-SWX-0007.
fuzz_target!(|data: &[u8]| {
    if let Ok(value) = swx::scales(data) {
        assert!(data.len() <= Product::Scales.limit());
        assert!([value.r, value.s, value.g].iter().all(|scale| *scale <= 5));
    }
});
