// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
#![forbid(unsafe_code)]
use libfuzzer_sys::fuzz_target;
use qso_graph_spaceweather::{self as swx, Product};

// QGDXC-SWX-0007 and QGDXC-SWX-0008.
fuzz_target!(|data: &[u8]| {
    if let Ok(values) = swx::solar_flux_history(data) {
        assert!(data.len() <= Product::SolarFluxHistory.limit());
        assert!((1..=400).contains(&values.len()));
        assert!(
            values
                .windows(2)
                .all(|w| w[0].time / 86_400 < w[1].time / 86_400)
        );
        for value in values {
            assert!((0.0..=999.0).contains(&value.flux));
            assert!(
                value
                    .ninety_day_mean
                    .is_none_or(|mean| (0.0..=999.0).contains(&mean))
            );
        }
    }
});
