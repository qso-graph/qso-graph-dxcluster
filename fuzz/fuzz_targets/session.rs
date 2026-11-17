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
    let mut session = Session::new(config).unwrap();
    let mut now = 0;
    for b in data.iter().take(100000) {
        let batch = match b {
            0 => session.start(now),
            1 if session.state == State::Connecting => session.connected(now, session.attempt),
            2 => session.cancel(now),
            3 => session.transport_end(now, session.attempt, "eof"),
            4 => {
                if let Some((at, t)) = session.next_timer()
                    && at >= now
                {
                    now = at;
                    session.fire(t, now)
                } else {
                    Ok(Batch::default())
                }
            }
            5 => session.restart(now),
            _ => session.input(now, session.attempt, *b, None),
        };
        if let Ok(batch) = batch {
            for action in batch.actions {
                match action {
                    Action::ReadinessToken { attempt } => {
                        let _ =
                            session.readiness_token(now, attempt, format!("QGDXC{attempt:016X}"));
                    }
                    Action::RetryJitter { maximum_ms } => {
                        let _ = session.retry_jitter(now, u64::from(*b) % (maximum_ms + 1));
                    }
                    _ => {}
                }
            }
        }
    }
    // QGDXC-IO-0005: adversarial coalesced input uses the same drain path.
    let mut session = Session::new(Config::new(
        "example.invalid".into(),
        7300,
        "Q1ABC".into(),
        Profile::Rbn,
    ))
    .unwrap();
    session.start(0).unwrap();
    session.connected(0, session.attempt).unwrap();
    let mut batch = session
        .input_chunk(
            0,
            session.attempt,
            &data[..data.len().min(INPUT_BYTES)],
            None,
        )
        .unwrap();
    loop {
        assert!(batch.events.len() <= INPUT_EVENTS);
        drop(batch);
        if !session.has_pending_input() {
            break;
        }
        batch = session.drain_input().unwrap();
    }
});
