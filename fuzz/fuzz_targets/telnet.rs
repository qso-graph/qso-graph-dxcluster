// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
#![forbid(unsafe_code)]
use libfuzzer_sys::fuzz_target;
use qso_graph_dxcluster::framing::Telnet;
fuzz_target!(|data: &[u8]| {
    let mut t = Telnet::default();
    for chunk in data.chunks(3) {
        for b in chunk {
            if t.push(*b).is_err() {
                let _ = t.eof();
            }
        }
    }
    let _ = t.eof();
});
