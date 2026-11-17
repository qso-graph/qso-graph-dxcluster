// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
#![forbid(unsafe_code)]
use libfuzzer_sys::fuzz_target;
use qso_graph_dxcluster::*;
fuzz_target!(|data: &[u8]| {
    let profile = [
        Profile::Auto,
        Profile::DxSpider,
        Profile::ArCluster,
        Profile::CcCluster,
        Profile::Rbn,
    ][data.first().copied().unwrap_or(0) as usize % 5];
    let config = Config::new("example.invalid".into(), 7300, "Q1ABC".into(), profile);
    let mut session = Session::new(
        config,
        (0..256).map(|i| format!("QGDXC{i:016X}")).collect(),
        vec![0; 256],
    )
    .unwrap();
    let mut now = 0;
    for b in data.iter().take(100000) {
        match b {
            0 => {
                let _ = session.start(now);
            }
            1 if session.state == State::Connecting => {
                let _ = session.connected(now, session.attempt);
            }
            2 => {
                let _ = session.cancel(now);
            }
            3 => {
                let _ = session.transport_end(now, session.attempt, "eof");
            }
            4 => {
                if let Some((at, t)) = session.next_timer()
                    && at >= now
                {
                    now = at;
                    let _ = session.fire(t, now);
                }
            }
            5 => {
                let _ = session.restart(now);
            }
            _ => {
                let _ = session.input(now, session.attempt, *b, None);
            }
        }
    }
});
