// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
#![forbid(unsafe_code)]
use libfuzzer_sys::fuzz_target;
use qso_graph_spaceweather as swx;

// QGDXC-SWX-0009–0011.
fuzz_target!(|data: &[u8]| {
    if let Some((product, time)) = data.split_last_chunk::<8>()
        && let Ok(alerts) = swx::alerts(product)
    {
        let active = swx::in_force(&alerts, i64::from_le_bytes(*time));
        assert!(active.len() <= alerts.len());
    }
});
