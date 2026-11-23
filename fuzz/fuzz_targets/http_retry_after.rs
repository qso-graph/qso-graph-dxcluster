// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use qso_graph_http::parse_retry_after;
use std::time::Duration;

fuzz_target!(|data: &[u8]| {
    for now in [Duration::from_secs(1_795_376_460), Duration::MAX] {
        let _ = parse_retry_after(data, now);
    }
});
