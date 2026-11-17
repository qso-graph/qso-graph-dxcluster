// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
#![forbid(unsafe_code)]
use libfuzzer_sys::fuzz_target;
use qso_graph_dxcluster::{Context, Profile, framing::Frame, parser::parse_frame};
fuzz_target!(|data: &[u8]| {
    let data = &data[..data.len().min(65536)];
    for retain in [false, true] {
        let mut context = Context::ready(Profile::Rbn);
        context.retain_third_party_data = retain;
        let _ = parse_frame(
            Frame {
                bytes: data.to_vec(),
                ending: "none",
                reason: None,
            },
            &context,
            None,
        );
    }
});
