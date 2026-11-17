// SPDX-License-Identifier: GPL-3.0-or-later
use qso_graph_dxcluster::*;
fn session(p: Profile) -> Session {
    Session::new(
        Config::new("example.invalid".into(), 7300, "q1abc".into(), p),
        (0..32).map(|n| format!("QGDXC{n:016X}")).collect(),
        vec![0; 32],
    )
    .unwrap()
}
fn append(out: &mut Batch, mut b: Batch) {
    out.events.append(&mut b.events);
    out.actions.append(&mut b.actions);
}
fn input(s: &mut Session, t: u64, bytes: &[u8]) -> Batch {
    let mut b = Batch::default();
    for byte in bytes {
        let attempt = s.attempt;
        append(&mut b, s.input(t, attempt, *byte, None).unwrap());
    }
    b
}
fn begin(s: &mut Session) -> Batch {
    let mut b = s.start(0).unwrap();
    append(&mut b, s.connected(0, 1).unwrap());
    append(&mut b, input(s, 0, b"Please enter your call: "));
    b
}
fn outputs(b: &Batch) -> Vec<String> {
    b.actions
        .iter()
        .filter_map(|a| {
            if let Action::Output { bytes, .. } = a {
                Some(String::from_utf8(bytes.clone()).unwrap())
            } else {
                None
            }
        })
        .collect()
}
#[test]
fn passive_profiles_and_duplicate_login() {
    // QGDXC-SES-0001 QGDXC-SES-0002 QGDXC-SES-0004 QGDXC-SES-0005 QGDXC-SES-0006
    // QGDXC-FRM-0004 QGDXC-ARC-0001 QGDXC-ARC-0002 QGDXC-RBN-0001 QGDXC-DSP-0001 QGDXC-EVT-0002
    for (profile, prompt) in [
        (Profile::ArCluster, "Q1ABC de Q2ABC 1-nOv 1832Z arc6>"),
        (Profile::Rbn, "Q1ABC de RELAY 1-Nov-2026 18:32Z >"),
    ] {
        let mut s = session(Profile::Auto);
        let b = begin(&mut s);
        assert_eq!(outputs(&b), ["q1abc\r\n"]);
        assert_eq!(s.state, State::AwaitIdentity);
        assert!(outputs(&input(&mut s, 1, b"login: \nDX de Q2ABC: 1 Q3ABC 1832Z\n")).is_empty());
        let b = input(&mut s, 100, prompt.as_bytes());
        assert_eq!(s.state, State::Ready);
        assert_eq!(s.profile, profile);
        assert!(outputs(&b).is_empty());
        assert_eq!(
            b.events.iter().map(|e| e.kind).collect::<Vec<_>>(),
            ["prompt_status", "connection_state", "connection_state"]
        );
        assert!(input(&mut s, 100, prompt.as_bytes()).actions.is_empty());
    }
    let mut s = session(Profile::Auto);
    begin(&mut s);
    input(
        &mut s,
        100,
        b"banner running DXSpider\nCapabilities: ve7cc rbn\n",
    );
    assert_eq!(s.state, State::AwaitIdentity);
    assert_eq!(s.profile, Profile::DxSpider);
}
#[test]
fn cc_paced_deadline() {
    // QGDXC-CCC-0001 QGDXC-CCC-0002 QGDXC-SES-0007 QGDXC-SES-0011 QGDXC-VEC-0003
    let mut s = session(Profile::CcCluster);
    s.config.setup_ms = 5000;
    begin(&mut s);
    let p = b"Q1ABC de Q2ABC 1-Nov-2026 1832Z CCC >";
    let b = input(&mut s, 100, p);
    assert!(outputs(&b).is_empty());
    assert_eq!(s.deadline(Timer::Setup), None);
    assert_eq!(s.deadline(Timer::Pacing), Some(1000));
    let b = s.fire(Timer::Pacing, 1000).unwrap();
    assert_eq!(outputs(&b), ["set/ve7cc\r\n"]);
    assert_eq!(s.deadline(Timer::Setup), Some(6000));
    input(&mut s, 5500, b"\r\nEnhanced Spots Enabled\r\n");
    input(&mut s, 5500, p);
    assert_eq!(s.state, State::Ready);
    assert_eq!(s.deadline(Timer::Setup), None);
}
#[test]
fn dxspider_startup_and_custom_prompt() {
    // QGDXC-DSP-0002 QGDXC-DSP-0004 QGDXC-SES-0006 QGDXC-SEC-0002
    let mut s = session(Profile::DxSpider);
    s.config.custom_prompt = Some("Q ready:>".into());
    // Reconstruct so context uses the caller's configured literal.
    let cfg = s.config.clone();
    let mut s = Session::new(cfg, vec!["QGDXC0000000000000000".into()], vec![0]).unwrap();
    begin(&mut s);
    input(&mut s, 100, b"Capabilities: ve7cc rbn\nQ ready:>");
    for (i, (cmd, ack)) in [
        ("set/page 0", "Page Length is now 0 lines"),
        ("set/nobeep", "Beeps are now off"),
        ("unset/echo", "Echoing disabled"),
        ("set/ve7cc", "Operation successful"),
        ("echo QGDXC0000000000000000", "QGDXC0000000000000000"),
    ]
    .iter()
    .enumerate()
    {
        let now = (i as u64 + 1) * 1000;
        let b = s.fire(Timer::Pacing, now).unwrap();
        assert_eq!(outputs(&b), [format!("{cmd}\r\n")]);
        if i == 4 {
            input(&mut s, now, b"echo QGDXC0000000000000000\nQ ready:>");
            assert_eq!(s.state, State::Setup);
        }
        input(&mut s, now, format!("{ack}\nQ ready:>").as_bytes());
    }
    assert_eq!(s.state, State::Ready);
    let b = input(&mut s, 5000, b"quit\nset/hostile\n");
    assert!(outputs(&b).is_empty());
    let mut cfg = s.config.clone();
    cfg.custom_prompt = Some("bad".into());
    assert!(cfg.validate().is_err());
}
#[test]
fn failures_latch_flush_and_cancel() {
    // QGDXC-SES-0003 QGDXC-SES-0012 QGDXC-SES-0013 QGDXC-SES-0014 QGDXC-DSP-0003 QGDXC-DSP-0009
    for (bytes, reason) in [
        (b"Password:".as_slice(), "password_unsupported"),
        (b"Sorry Q1ABC is an invalid callsign", "refused"),
    ] {
        let mut s = session(Profile::Auto);
        s.start(0).unwrap();
        s.connected(0, 1).unwrap();
        input(&mut s, 0, bytes);
        let b = s.transport_end(1, 1, "eof").unwrap();
        assert_eq!(s.state, State::Blocked);
        assert!(
            b.events.iter().any(|e| e.get("reason") == Some(reason))
                || reason == "password_unsupported"
        );
        assert!(
            s.transport_end(1, 1, "transport_error")
                .unwrap()
                .events
                .is_empty()
        );
        assert!(s.next_timer().is_none());
    }
    for reason in [
        "eof",
        "transport_error",
        "unsupported_transport",
        "resource_limit",
    ] {
        for suffix in ["content\r", "partial"] {
            let mut s = session(Profile::Rbn);
            begin(&mut s);
            input(&mut s, 1, format!("\n{suffix}").as_bytes());
            let b = s.transport_end(2, 1, reason).unwrap();
            assert_eq!(
                b.events[0].raw_line,
                suffix.trim_end_matches('\r').as_bytes()
            );
        }
    }
    let mut s = session(Profile::DxSpider);
    begin(&mut s);
    input(&mut s, 1, b"Capabilities: ve7cc rbn\n");
    let b = input(
        &mut s,
        1,
        b"Reconnected as Q1ABC at <ip>, this instance is disconnected\n",
    );
    assert!(
        b.events
            .iter()
            .any(|e| e.get("reason") == Some("displaced"))
    );
    assert_eq!(s.state, State::Blocked);
    let mut s = session(Profile::Rbn);
    s.start(0).unwrap();
    s.cancel(1).unwrap();
    assert_eq!(s.state, State::Disconnected);
    assert!(s.next_timer().is_none());
    assert!(s.transport_end(2, 1, "eof").unwrap().events.is_empty());
}
#[test]
fn retries_stability_and_restart() {
    // QGDXC-SES-0008 QGDXC-SES-0009 QGDXC-SES-0010 QGDXC-IO-0004 QGDXC-IO-0005
    let mut s = session(Profile::Rbn);
    s.start(0).unwrap();
    let mut now = 0;
    for delay in [5000, 10000, 20000, 40000, 80000, 160000, 300000, 300000] {
        s.transport_end(now, s.attempt, "transport_error").unwrap();
        assert_eq!(s.deadline(Timer::Retry), Some(now + delay));
        now += delay;
        s.fire(Timer::Retry, now).unwrap();
    }
    s.transport_end(now, s.attempt, "eof").unwrap();
    assert_eq!(s.state, State::Blocked);
    assert!(s.next_timer().is_none());
    let b = s.restart(now).unwrap();
    assert!(matches!(b.actions[0], Action::Connect { .. }));
    s.transport_end(now, s.attempt, "transport_error").unwrap();
    assert_eq!(s.deadline(Timer::Retry), Some(now + 5000));
    s.cancel(now + 5000).unwrap();
    assert!(s.next_timer().is_none());
    let mut s = session(Profile::Rbn);
    begin(&mut s);
    input(&mut s, 100, b"Q1ABC de RELAY 1-Nov-2026 18:32Z >");
    assert_eq!(s.deadline(Timer::Silence), None);
    s.fire(Timer::StableReady, 600100).unwrap();
    assert!(s.next_timer().is_none());
    assert!(s.start(1).is_err());
    let b = s.restart(600101).unwrap();
    let close = b
        .actions
        .iter()
        .position(|a| matches!(a, Action::Close { .. }))
        .unwrap();
    let connect = b
        .actions
        .iter()
        .position(|a| matches!(a, Action::Connect { .. }))
        .unwrap();
    assert!(close < connect);
}
#[test]
fn identity_mismatch_timeout_and_resource_flush() {
    let mut s = session(Profile::ArCluster);
    begin(&mut s);
    input(&mut s, 100, b"Q1ABC de RELAY 1-Nov-2026 18:32Z >");
    assert_eq!(s.state, State::Blocked);
    let mut s = session(Profile::Auto);
    begin(&mut s);
    s.fire(Timer::Identity, 30000).unwrap();
    assert_eq!(s.state, State::Blocked);
    let mut s = session(Profile::Rbn);
    begin(&mut s);
    input(&mut s, 10, &[255, 250, 0]);
    let b = s.transport_end(20, 1, "eof").unwrap();
    assert!(
        b.events
            .iter()
            .any(|e| e.get("reason") == Some("resource_limit"))
    );
}

#[test]
fn silence_uses_application_bytes_and_custom_context() {
    // QGDXC-SES-0008 QGDXC-IO-0004 QGDXC-DSP-0002
    let mut s = session(Profile::Rbn);
    s.config.silence_ms = Some(900000);
    begin(&mut s);
    input(&mut s, 100, b"Q1ABC de RELAY 1-Nov-2026 18:32Z >");
    assert_eq!(s.deadline(Timer::Silence), Some(900100));
    input(&mut s, 1000, &[255, 251, 1]);
    assert_eq!(s.deadline(Timer::Silence), Some(900100));
    input(&mut s, 2000, b"x");
    assert_eq!(s.deadline(Timer::Silence), Some(902000));
    let mut c = Context::ready(Profile::DxSpider);
    c.identity = "Q1ABC".into();
    c.custom_prompt = Some("Q custom:>".into());
    let mut p = Parser::new(c, 256).unwrap();
    let mut events = vec![];
    for b in b"Q custom:>  \r\n" {
        events.extend(p.push(*b, None).unwrap());
    }
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].raw_line, b"Q custom:>");
    assert_eq!(events[1].get("reason"), Some("blank"));
    assert_eq!(events[1].raw_line, b"  ");
}
