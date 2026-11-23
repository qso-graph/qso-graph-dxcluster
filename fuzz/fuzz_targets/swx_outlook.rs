// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
#![forbid(unsafe_code)]
use libfuzzer_sys::fuzz_target;
use qso_graph_spaceweather::band_outlook;

// QGDXC-SWX-0007 and QGDXC-SWX-0012.
fuzz_target!(|data: &[u8]| {
    if let Some((sfi, rest)) = data.split_first_chunk::<8>()
        && let Ok(kp) = <[u8; 8]>::try_from(rest)
        && let Ok(outlook) = band_outlook(f64::from_le_bytes(*sfi), f64::from_le_bytes(kp))
    {
        assert_eq!(
            outlook.map(|entry| entry.band),
            [
                "160m", "80m", "40m", "30m", "20m", "17m", "15m", "12m", "10m", "6m"
            ]
        );
    }
});
