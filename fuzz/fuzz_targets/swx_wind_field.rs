// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
#![forbid(unsafe_code)]
use libfuzzer_sys::fuzz_target;
use qso_graph_spaceweather::{self as swx, Product};

// QGDXC-SWX-0007.
fuzz_target!(|data: &[u8]| {
    if let Ok(value) = swx::wind_field(data) {
        assert!(data.len() <= Product::WindField.limit());
        assert!((0.0..=200.0).contains(&value.bt));
        assert!((-200.0..=200.0).contains(&value.bz_gsm));
    }
});
