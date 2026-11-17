// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
#![forbid(unsafe_code)]
use libfuzzer_sys::fuzz_target;
use qso_graph_dxcluster::values;
fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(&data[..data.len().min(65536)]) {
        let _ = values::frequency(s);
        let _ = values::time(s, true);
        let _ = values::date(s);
        let _ = values::grid(s);
        let _ = values::login(s);
        let _ = values::received_call(s);
        let _ = values::receipt(s);
    }
});
