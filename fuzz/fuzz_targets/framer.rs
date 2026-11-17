// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
#![forbid(unsafe_code)]
use libfuzzer_sys::fuzz_target;
use qso_graph_dxcluster::framing::Framer;
fuzz_target!(|data: &[u8]| {
    let mut f = Framer::new(256).unwrap();
    for chunk in data.chunks(7) {
        for b in chunk {
            let _ = f.push(*b);
        }
    }
    let _ = f.eof();
});
