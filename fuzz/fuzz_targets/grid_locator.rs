// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use qso_graph_grid::Locator;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data)
        && let Ok(locator) = Locator::parse(text)
    {
        assert_eq!(Locator::parse(&locator.to_string()), Ok(locator));
    }
});
