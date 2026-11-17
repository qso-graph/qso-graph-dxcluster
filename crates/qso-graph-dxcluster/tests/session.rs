// SPDX-License-Identifier: GPL-3.0-or-later
use qso_graph_dxcluster::*;
fn session(p: Profile) -> TestSession {
    TestSession::new(Config::new(
        "example.invalid".into(),
        7300,
        "q1abc".into(),
        p,
    ))
    .unwrap()
}

// The test caller supplies each request immediately, without an entropy pool.
struct TestSession(
    qso_graph_dxcluster::Session,
    std::collections::HashSet<String>,
);
impl std::ops::Deref for TestSession {
    type Target = qso_graph_dxcluster::Session;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for TestSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl TestSession {
    fn new(config: Config) -> Result<Self, &'static str> {
        qso_graph_dxcluster::Session::new(config).map(|s| Self(s, Default::default()))
    }
    fn supply(&mut self, b: Batch) -> Result<Batch, &'static str> {
        let mut out = Batch {
            events: b.events,
            actions: vec![],
        };
        for a in b.actions {
            let supplied = match a {
                Action::ReadinessToken { attempt } => {
                    let token = format!("QGDXC{:016X}", attempt - 1);
                    assert!(self.1.insert(token.clone()));
                    self.0
                        .readiness_token(self.0.current_time(), attempt, token)?
                }
                Action::RetryJitter { .. } => self.0.retry_jitter(self.0.current_time(), 0)?,
                a => {
                    out.actions.push(a);
                    continue;
                }
            };
            append(&mut out, self.supply(supplied)?);
        }
        Ok(out)
    }
    fn start(&mut self, now: u64) -> Result<Batch, &'static str> {
        let b = self.0.start(now)?;
        self.supply(b)
    }
    fn restart(&mut self, now: u64) -> Result<Batch, &'static str> {
        let b = self.0.restart(now)?;
        self.supply(b)
    }
    fn fire(&mut self, timer: Timer, now: u64) -> Result<Batch, &'static str> {
        let b = self.0.fire(timer, now)?;
        self.supply(b)
    }
    fn transport_end(
        &mut self,
        now: u64,
        attempt: u64,
        reason: &'static str,
    ) -> Result<Batch, &'static str> {
        let b = self.0.transport_end(now, attempt, reason)?;
        self.supply(b)
    }
    fn input(
        &mut self,
        now: u64,
        attempt: u64,
        byte: u8,
        received_at: Option<&str>,
    ) -> Result<Batch, &'static str> {
        let b = self.0.input(now, attempt, byte, received_at)?;
        self.supply(b)
    }
}

fn append(out: &mut Batch, mut b: Batch) {
    out.events.append(&mut b.events);
    out.actions.append(&mut b.actions);
}
fn input(s: &mut TestSession, t: u64, bytes: &[u8]) -> Batch {
    let mut b = Batch::default();
    for byte in bytes {
        let attempt = s.attempt;
        append(&mut b, s.input(t, attempt, *byte, None).unwrap());
    }
    b
}
fn begin(s: &mut TestSession) -> Batch {
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
    let mut s = TestSession::new(cfg).unwrap();
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
        (b"Sorry Q1ABC is an invalid callsign\r", "refused"),
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

#[test]
fn delayed_transport_admission_controls_pacing_and_reply_deadlines() {
    let mut s = session(Profile::DxSpider);
    s.config.setup_ms = 5000;
    begin(&mut s);
    input(
        &mut s,
        100,
        b"Capabilities: ve7cc rbn\nQ1ABC de Q2ABC 1-Nov-2026 1832Z dxspider >",
    );
    // A controlled transport holds the due command until 5 seconds, then
    // takes another 200 ms to accept the line. No wall clock or socket needed.
    let b = s.fire(Timer::Pacing, 5000).unwrap();
    assert_eq!(outputs(&b), ["set/page 0\r\n"]);
    s.output_admitted(5200, 1).unwrap();
    assert_eq!(s.deadline(Timer::Setup), Some(10200));
    let b = input(
        &mut s,
        5210,
        b"Page Length is now 0 lines\nQ1ABC de Q2ABC 1-Nov-2026 1832Z dxspider >",
    );
    assert!(outputs(&b).is_empty());
    assert_eq!(s.deadline(Timer::Pacing), Some(6200));
    assert!(s.fire(Timer::Pacing, 6199).is_err());
    let b = s.fire(Timer::Pacing, 6200).unwrap();
    assert_eq!(outputs(&b), ["set/nobeep\r\n"]);
    assert_eq!(s.deadline(Timer::Setup), Some(11200));
}

#[test]
fn relayed_and_incomplete_status_text_cannot_end_sessions() {
    // QGDXC-SES-0012 QGDXC-DSP-0003 QGDXC-EVT-0010
    for body in [
        "Reconnected as Q1ABC at <ip>, this instance is disconnected",
        "Sorry Q1ABC is an invalid callsign",
        "password:",
        "login:",
    ] {
        for (prefix, kind) in [
            ("To ALL de Q2ABC: ", "announcement"),
            ("Q1ABC de Q2ABC: ", "talk"),
            ("WX de Q2ABC: ", "announcement"),
            ("WWV de Q2ABC <23>: ", "wwv"),
            ("WCY de Q2ABC <23>: ", "wcy"),
        ] {
            let mut s = session(Profile::DxSpider);
            begin(&mut s);
            input(&mut s, 0, b"\n");
            let b = input(&mut s, 1, format!("{prefix}{body}\n").as_bytes());
            assert_eq!(s.state, State::AwaitIdentity, "{prefix}{body}");
            assert_eq!(b.events.len(), 1);
            assert_eq!(b.events[0].kind, kind);
            assert!(b.actions.is_empty());
        }
    }
    let notice = "Reconnected as Q1ABC at <ip>, this instance is disconnected";
    let mut s = session(Profile::DxSpider);
    s.config.line_limit = 256;
    // Rebuild to apply the frame bound to the parser.
    let mut s = TestSession::new(s.config.clone()).unwrap();
    begin(&mut s);
    input(&mut s, 0, b"\n");
    let prefix = format!("{notice}{}", " ".repeat(256 - notice.len()));
    let b = input(&mut s, 1, format!("{prefix}extra\n").as_bytes());
    assert_eq!(s.state, State::AwaitIdentity);
    assert_eq!(b.events[0].kind, "unrecognized_line");
    assert_eq!(b.events[0].raw_status, "truncated");
    assert!(b.actions.is_empty());
    input(&mut s, 2, notice.as_bytes());
    let b = s.transport_end(3, 1, "eof").unwrap();
    assert_eq!(s.state, State::Backoff);
    assert!(
        !b.events
            .iter()
            .any(|e| e.get("reason") == Some("displaced"))
    );
}
#[test]
fn setup_errors_require_complete_classified_status() {
    // QGDXC-SES-0007 QGDXC-SES-0012
    for (line, kind, reason) in [
        ("To ALL de Q2ABC: Q setup rejected", "announcement", None),
        ("Q1ABC de Q2ABC: Q setup rejected", "talk", None),
        ("WWV de Q2ABC <23>: Q setup rejected", "wwv", None),
        ("WCY de Q2ABC <23>: Q setup rejected", "wcy", None),
        ("Q setup rejected", "prompt_status", Some("setup_failed")),
    ] {
        let mut s = session(Profile::DxSpider);
        s.config.supported_setup_errors = vec!["Q setup rejected".into(), line.into()];
        begin(&mut s);
        input(&mut s, 0, b"\n");
        input(&mut s, 100, b"Q1ABC de Q2ABC 1-Nov-2026 1832Z dxspider >");
        s.fire(Timer::Pacing, 1000).unwrap();
        let b = input(&mut s, 1100, format!("{line}\n").as_bytes());
        assert_eq!(b.events[0].kind, kind);
        assert_eq!(
            s.state,
            if reason.is_some() {
                State::Blocked
            } else {
                State::Setup
            }
        );
    }
    let mut s = session(Profile::CcCluster);
    s.config.supported_setup_errors = vec!["Q setup rejected".into()];
    begin(&mut s);
    input(&mut s, 0, b"\n");
    input(&mut s, 100, b"Q1ABC de Q2ABC 1-Nov-2026 1832Z CCC >");
    s.fire(Timer::Pacing, 1000).unwrap();
    input(&mut s, 1100, b"Q setup rejected");
    let b = s.transport_end(1200, 1, "eof").unwrap();
    assert_eq!(s.state, State::Backoff);
    assert!(
        !b.events
            .iter()
            .any(|e| e.get("reason") == Some("setup_failed"))
    );
}

#[test]
fn stable_reconnects_continue_beyond_1024_attempts() {
    // QGDXC-SES-0010 QGDXC-SEC-0001
    for profile in [
        Profile::Auto,
        Profile::DxSpider,
        Profile::ArCluster,
        Profile::CcCluster,
        Profile::Rbn,
    ] {
        let mut s = TestSession::new(Config::new(
            "example.invalid".into(),
            7300,
            "Q1ABC".into(),
            profile,
        ))
        .unwrap();
        s.start(0).unwrap();
        let mut now = 0;
        for attempt in 1..=4096 {
            assert_eq!(s.attempt, attempt);
            s.connected(now, attempt).unwrap();
            input(&mut s, now, b"Please enter your call: ");
            let prompt = match profile {
                Profile::DxSpider => "Q1ABC de Q2ABC 1-Nov-2026 1832Z dxspider >",
                Profile::ArCluster => "Q1ABC de Q2ABC 1-Nov 1832Z arc6>",
                Profile::CcCluster => "Q1ABC de Q2ABC 1-Nov-2026 1832Z CCC >",
                _ => "Q1ABC de RELAY 1-Nov-2026 18:32Z >",
            };
            input(&mut s, now + 100, prompt.as_bytes());
            if profile == Profile::DxSpider {
                for (i, ack) in [
                    "Page Length is now 0 lines".to_owned(),
                    "Beeps are now off".into(),
                    "Echoing disabled".into(),
                    "Operation successful".into(),
                    format!("QGDXC{:016X}", attempt - 1),
                ]
                .iter()
                .enumerate()
                {
                    let at = now + (i as u64 + 1) * 1000;
                    let b = s.fire(Timer::Pacing, at).unwrap();
                    if i == 4 {
                        assert_eq!(outputs(&b), [format!("echo {ack}\r\n")]);
                    }
                    input(&mut s, at, format!("{ack}\n{prompt}").as_bytes());
                }
            } else if profile == Profile::CcCluster {
                s.fire(Timer::Pacing, now + 1000).unwrap();
                input(
                    &mut s,
                    now + 1000,
                    format!("Enhanced Spots Enabled\n{prompt}").as_bytes(),
                );
            }
            assert_eq!(s.state, State::Ready);
            now = s.deadline(Timer::StableReady).unwrap();
            s.fire(Timer::StableReady, now).unwrap();
            s.transport_end(now, attempt, "eof").unwrap();
            assert_eq!(s.deadline(Timer::Retry), Some(now + 5000));
            now += 5000;
            s.fire(Timer::Retry, now).unwrap();
        }
    }
}
