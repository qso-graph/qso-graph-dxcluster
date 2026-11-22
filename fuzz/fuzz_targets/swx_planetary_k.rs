// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
#![forbid(unsafe_code)]
use libfuzzer_sys::fuzz_target;
use qso_graph_spaceweather::{self as swx, Product};

// QGDXC-SWX-0007.
fuzz_target!(|data: &[u8]| {
    if let Ok(value) = swx::planetary_k(data) {
        assert!(data.len() <= Product::PlanetaryK.limit());
        assert!((1..=200).contains(&value.len()));
        assert!(value.windows(2).all(|pair| pair[0].time < pair[1].time));
    }
});
