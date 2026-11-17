// SPDX-License-Identifier: GPL-3.0-or-later
//! Declarative session oracles: expected transitions are written from the state
//! table, rather than obtained by running the implementation.
use crate::oracle;
use serde_json::{Value, json};
struct Case {
    v: Value,
    step: usize,
    at: u64,
    profile: String,
    attempt: u64,
}
impl Case {
    fn new(id: &str, selected: &str) -> Self {
        Self {
            v: json!({"SPDX-License-Identifier":"GPL-3.0-or-later","id":id,"spec":"0.1.0","class":"Session engine","profile":selected,"provenance":{"capture":"synthetic","records":[],"derived_from":"QGDXC-VEC-0003","sanitization":["Invented Q-prefix identities"]},"config":{"host":"example.invalid","port":7300,"identity":"Q1ABC","profile":selected,"line_limit":4096,"retain_third_party_data":false,"connect_ms":30000,"login_ms":30000,"identity_ms":30000,"setup_ms":5000},"tokens":if ["auto","dxspider"].contains(&selected){(0..10).map(|n|format!("QGDXC{n:016X}")).collect::<Vec<_>>()}else{vec![]},"jitter_ms":vec![0;10],"steps":[],"expected_events":[],"expected_actions":[],"expected_state":"disconnected"}),
            step: 0,
            at: 0,
            profile: selected.into(),
            attempt: 1,
        }
    }
    fn step(&mut self, at: u64, mut s: Value) {
        self.at = at;
        s["elapsed_ms"] = json!(at);
        self.step = self.v["steps"].as_array().unwrap().len();
        self.v["steps"].as_array_mut().unwrap().push(s);
    }
    fn input(&mut self, at: u64, s: &str) {
        self.step(
            at,
            json!({"type":"input","attempt":self.attempt,"b64":oracle::b64(s.as_bytes())}),
        );
    }
    fn event(&mut self, e: Value) {
        self.v["expected_events"]
            .as_array_mut()
            .unwrap()
            .push(json!({"step":self.step,"elapsed_ms":self.at,"event":e}));
    }
    fn action(&mut self, a: Value) {
        self.v["expected_actions"]
            .as_array_mut()
            .unwrap()
            .push(json!({"step":self.step,"elapsed_ms":self.at,"action":a}));
    }
    fn state(&mut self, state: &str, reason: &str, e: Option<&Value>) {
        let mut event = e
            .cloned()
            .unwrap_or_else(|| oracle::local(&self.profile, state, reason, None));
        event["server_type"] = json!(if self.profile == "auto" {
            "unknown"
        } else {
            &self.profile
        });
        event["kind"] = json!("connection_state");
        event["fields"] = json!({"state":state,"reason":reason});
        self.event(event);
        self.v["expected_state"] = json!(state);
    }
    fn arm(&mut self, t: &str, deadline: u64) {
        self.action(json!({"type":"arm_timer","timer":t,"deadline_ms":deadline}));
    }
    fn cancel(&mut self, t: &str) {
        self.action(json!({"type":"cancel_timer","timer":t}));
    }
    fn output(&mut self, line: &str) {
        self.action(json!({"type":"output","attempt":self.attempt,"b64":oracle::b64(format!("{line}\r\n").as_bytes())}));
    }
    fn content(&mut self, text: &str, status: &str, ending: &str) -> Value {
        let e = oracle::event(
            &self.profile,
            "prompt_status",
            json!({"status":status,"text":text}),
            text.as_bytes(),
            ending,
            "exact",
        );
        self.event(e.clone());
        e
    }
    fn tick(&mut self) {
        self.step(self.at, json!({"type":"tick"}));
    }
    fn begin(&mut self) {
        self.step(0, json!({"type":"start"}));
        self.state("connecting", "started", None);
        self.action(json!({"type":"connect","attempt":1}));
        self.arm("connect", 30000);
        self.step(
            0,
            json!({"type":"transport","attempt":1,"outcome":"connected"}),
        );
        self.cancel("connect");
        self.state("await_login", "connected", None);
        self.arm("login", 30000);
    }
    fn login(&mut self, text: &str) {
        self.input(0, text);
        self.content(text, "login_prompt", "none");
        self.cancel("login");
        self.output("Q1ABC");
        self.state("await_identity", "login_sent", None);
        self.arm("identity", 30000);
        self.tick();
    }
    fn ready(&mut self, p: &str, prompt: &str) {
        self.input(100, prompt);
        let e = self.content(prompt, "ready_prompt", "none");
        self.profile = p.into();
        self.cancel("identity");
        self.state("setup", "identified", Some(&e));
        if ["rbn", "arcluster"].contains(&p) {
            self.state("ready", "setup_complete", Some(&e));
            self.arm("stable_ready", 600100);
        } else {
            self.arm("pacing", 1000);
        }
        self.tick();
    }
    fn finish(self) -> Value {
        self.v
    }
}
pub fn vectors() -> Vec<Value> {
    let mut cases = extra();
    cases.push(overlong_banner());
    cases.extend(inert_status_text());
    cases.push(application_retry_exhaustion());
    cases.extend(stricter_pacing());
    let prompts = [
        ("arcluster", "Q1ABC de Q2ABC 1-Nov 1832Z arc6>"),
        ("rbn", "Q1ABC de RELAY 1-Nov-2026 18:32Z >"),
        ("cccluster", "Q1ABC de Q2ABC 1-Nov-2026 1832Z CCC >"),
        ("dxspider", "Q1ABC de Q2ABC 1-Nov-2026 1832Z dxspider >"),
    ];
    for (p, prompt) in prompts {
        for selected in [p, "auto"] {
            let mut c = Case::new(&format!("startup-{selected}-{p}"), selected);
            c.begin();
            c.login("login:");
            c.ready(p, prompt);
            if ["cccluster", "dxspider"].contains(&p) {
                let commands = if p == "cccluster" {
                    vec![("set/ve7cc", "Enhanced Spots Enabled")]
                } else {
                    vec![
                        ("set/page 0", "Page Length is now 0 lines"),
                        ("set/nobeep", "Beeps are now off"),
                        ("unset/echo", "Echoing disabled"),
                        ("set/ve7cc", "Operation successful"),
                        ("echo QGDXC0000000000000000", "QGDXC0000000000000000"),
                    ]
                };
                let total = commands.len();
                for (i, (cmd, ack)) in commands.into_iter().enumerate() {
                    let at = (i as u64 + 1) * 1000;
                    c.step(at, json!({"type":"tick"}));
                    c.output(cmd);
                    c.arm("setup", at + 5000);
                    let acktime = if p == "cccluster" { 5500 } else { at + 100 };
                    c.input(acktime, &format!("{ack}\n{prompt}"));
                    c.content(ack, "setup_reply", "lf");
                    let e = c.content(prompt, "ready_prompt", "none");
                    c.cancel("setup");
                    if i + 1 == total {
                        c.state("ready", "setup_complete", Some(&e));
                        c.arm("stable_ready", acktime + 600000);
                    } else {
                        c.arm("pacing", at + 1000);
                    }
                    c.tick();
                }
            }
            cases.push(c.finish());
        }
    }
    for selected in ["auto", "dxspider", "arcluster", "cccluster", "rbn"] {
        for login in ["call:", "Please enter your call:", "LOGIN:"] {
            let mut c = Case::new(&format!("login-{selected}-{}", login.len()), selected);
            c.begin();
            c.login(login);
            cases.push(c.finish());
        }
    }
    for (label, text, status, reason) in [
        (
            "password",
            "password:",
            "password_prompt",
            "password_unsupported",
        ),
        (
            "refused",
            "Sorry Q1ABC is an invalid callsign\r\n",
            "notice",
            "refused",
        ),
    ] {
        let mut c = Case::new(label, "auto");
        c.begin();
        c.input(0, text);
        let e = c.content(
            text.trim_end_matches(['\r', '\n']),
            status,
            if text.ends_with('\n') { "crlf" } else { "none" },
        );
        c.cancel("login");
        c.action(json!({"type":"close","attempt":1}));
        c.state("blocked", reason, Some(&e));
        c.tick();
        c.step(1, json!({"type":"transport","attempt":1,"outcome":"eof"}));
        c.tick();
        cases.push(c.finish());
    }
    for (timer, state, reason) in [
        ("connect", "connecting", "timeout"),
        ("login", "await_login", "timeout"),
        ("identity", "await_identity", "profile_unknown"),
    ] {
        let mut c = Case::new(&format!("deadline-{timer}"), "auto");
        if timer == "connect" {
            c.step(0, json!({"type":"start"}));
            c.state(state, "started", None);
            c.action(json!({"type":"connect","attempt":1}));
            c.arm(timer, 30000);
            c.tick();
        } else {
            c.begin();
            if timer == "identity" {
                c.login("login:");
            } else {
                c.tick();
            }
        }
        c.step(30000, json!({"type":"tick"}));
        c.action(json!({"type":"close","attempt":1}));
        if reason == "timeout" {
            c.state("disconnected", reason, None);
            let mut e = oracle::local("auto", "backoff", "retry_scheduled", Some(5000));
            e["server_type"] = json!("unknown");
            c.event(e);
            c.v["expected_state"] = json!("backoff");
            c.arm("retry", 35000);
        } else {
            c.state("blocked", reason, None);
        }
        cases.push(c.finish());
    }
    for outcome in ["eof", "error", "unsupported_transport", "resource_limit"] {
        for suffix in ["partial", "pending\r"] {
            let mut c = Case::new(&format!("termination-{outcome}-{}", suffix.len()), "rbn");
            c.begin();
            c.login("login:");
            c.input(1, suffix);
            c.tick();
            c.step(2, json!({"type":"transport","attempt":1,"outcome":outcome}));
            c.event(oracle::line(
                "rbn",
                suffix.trim_end_matches('\r').as_bytes(),
                if suffix.ends_with('\r') { "cr" } else { "none" },
                (!suffix.ends_with('\r')).then_some("partial"),
            ));
            c.cancel("identity");
            let reason = if outcome == "error" {
                "transport_error"
            } else {
                outcome
            };
            if ["eof", "error"].contains(&outcome) {
                c.state("disconnected", reason, None);
                c.event(oracle::local(
                    "rbn",
                    "backoff",
                    "retry_scheduled",
                    Some(5000),
                ));
                c.v["expected_state"] = json!("backoff");
                c.arm("retry", 5002);
            } else {
                c.state("blocked", reason, None);
            }
            c.tick();
            cases.push(c.finish());
        }
    }
    for suffix in ["partial", "pending\r"] {
        let mut c = Case::new(&format!("cancel-flush-{}", suffix.len()), "rbn");
        c.begin();
        c.login("login:");
        c.input(1, suffix);
        c.tick();
        c.step(2, json!({"type":"cancel"}));
        c.event(oracle::line(
            "rbn",
            suffix.trim_end_matches('\r').as_bytes(),
            if suffix.ends_with('\r') { "cr" } else { "none" },
            (!suffix.ends_with('\r')).then_some("partial"),
        ));
        c.cancel("identity");
        c.action(json!({"type":"close","attempt":1}));
        c.state("disconnected", "cancelled", None);
        c.tick();
        c.step(100000, json!({"type":"tick"}));
        cases.push(c.finish());
    }
    cases.extend(mutations(&cases));
    cases.extend(application_and_validation());
    cases
}

fn stricter_pacing() -> Vec<Value> {
    let mut cases = vec![];
    // QGDXC-SES-0011 QGDXC-VEC-0006. These deadlines are specified here,
    // independently of the session's pacing implementation.
    for (id, settings, send_at) in [
        ("stricter-spacing", json!({"spacing_ms":2000}), 2000),
        ("stricter-window-lines", json!({"window_lines":1}), 20000),
        (
            "stricter-window-duration",
            json!({"window_ms":30000,"window_lines":1}),
            30000,
        ),
    ] {
        let mut c = Case::new(id, "cccluster");
        for (key, value) in settings.as_object().unwrap() {
            c.v["config"][key] = value.clone();
        }
        c.v["provenance"]["derived_from"] = json!("QGDXC-SES-0011 QGDXC-VEC-0006");
        c.begin();
        c.login("login:");
        let prompt = "Q1ABC de Q2ABC 1-Nov-2026 1832Z CCC >";
        c.input(100, prompt);
        let e = c.content(prompt, "ready_prompt", "none");
        c.cancel("identity");
        c.state("setup", "identified", Some(&e));
        c.arm("pacing", send_at);
        c.tick();
        for at in [1000, send_at - 1] {
            c.step(at, json!({"type":"tick"}));
        }
        c.step(send_at, json!({"type":"tick"}));
        c.output("set/ve7cc");
        c.arm("setup", send_at + 5000);
        c.input(send_at + 100, &format!("Enhanced Spots Enabled\n{prompt}"));
        c.content("Enhanced Spots Enabled", "setup_reply", "lf");
        let e = c.content(prompt, "ready_prompt", "none");
        c.cancel("setup");
        c.state("ready", "setup_complete", Some(&e));
        c.arm("stable_ready", send_at + 600100);
        c.tick();
        cases.push(c.finish());
    }
    cases
}

fn overlong_banner() -> Value {
    let mut c = Case::new("overlong-ar-banner-no-profile-evidence", "auto");
    c.v["provenance"]["derived_from"] = json!("QGDXC-EVT-0010 QGDXC-SES-0004 QGDXC-ARC-0001");
    c.begin();
    c.login("login:");
    let banner = format!("Running AR-Cluster Version 7 {}", "x".repeat(4100));
    c.input(1, &format!("{banner}\n"));
    c.event(oracle::line(
        "auto",
        &banner.as_bytes()[..4096],
        "none",
        Some("line_too_long"),
    ));
    c.tick();
    let prompt = "Q1ABC de Q2ABC 1-Nov 1832Z arc7>";
    c.input(2, &format!("{prompt}\n"));
    c.event(oracle::event(
        "auto",
        "unrecognized_line",
        json!({"reason":"unknown","text":prompt}),
        prompt.as_bytes(),
        "lf",
        "exact",
    ));
    c.tick();
    c.step(30000, json!({"type":"tick"}));
    c.action(json!({"type":"close","attempt":1}));
    c.state("blocked", "profile_unknown", None);
    c.finish()
}

fn extra() -> Vec<Value> {
    let mut cases = vec![];
    for selected in ["auto", "dxspider", "arcluster", "cccluster", "rbn"] {
        let mut c = Case::new(&format!("before-identification-{selected}"), selected);
        c.begin();
        c.login("login:");
        for line in ["DX de Q9XYZ-#: 1 Q2ABC CW -1 dB 20 WPM CQ 1832Z", "CC11^1"] {
            c.input(1, &format!("{line}\n"));
            let mut e = oracle::line(selected, line.as_bytes(), "lf", None);
            crate::generate::default_privacy(&mut e, selected, false);
            c.event(e);
        }
        c.tick();
        cases.push(c.finish());
    }
    for label in [
        "duplicate-login",
        "password-after-login",
        "profile-mismatch",
        "setup-timeout",
        "telnet-unterminated",
        "telnet-excess",
        "flush-refusal",
    ] {
        let selected = if label == "profile-mismatch" {
            "arcluster"
        } else if label == "setup-timeout" {
            "cccluster"
        } else {
            "auto"
        };
        let mut c = Case::new(label, selected);
        c.begin();
        c.login("login:");
        match label {
            "duplicate-login" => {
                c.input(1, "login:\n");
                c.event(oracle::line("auto", b"login:", "lf", None));
                c.tick();
            }
            "password-after-login" => {
                c.input(1, "PASSWORD:");
                let e = c.content("PASSWORD:", "password_prompt", "none");
                c.cancel("identity");
                c.action(json!({"type":"close","attempt":1}));
                c.state("blocked", "password_unsupported", Some(&e));
                c.tick();
            }
            "profile-mismatch" => {
                let text = "Q1ABC de RELAY 1-Nov-2026 18:32Z >";
                c.input(1, text);
                let e = c.content(text, "ready_prompt", "none");
                c.cancel("identity");
                c.action(json!({"type":"close","attempt":1}));
                c.state("blocked", "profile_mismatch", Some(&e));
                c.tick();
            }
            "setup-timeout" => {
                c.ready("cccluster", "Q1ABC de Q2ABC 1-Nov-2026 1832Z CCC >");
                c.step(1000, json!({"type":"tick"}));
                c.output("set/ve7cc");
                c.arm("setup", 6000);
                c.step(6000, json!({"type":"tick"}));
                c.action(json!({"type":"close","attempt":1}));
                c.state("blocked", "setup_failed", None);
            }
            "telnet-unterminated" | "telnet-excess" => {
                let mut b = vec![255, 250];
                if label == "telnet-excess" {
                    b.resize(4097, 0);
                }
                c.step(1, json!({"type":"input","attempt":1,"b64":oracle::b64(&b)}));
                if label == "telnet-unterminated" {
                    c.tick();
                    c.step(2, json!({"type":"transport","attempt":1,"outcome":"eof"}));
                }
                c.cancel("identity");
                if label == "telnet-excess" {
                    c.action(json!({"type":"close","attempt":1}));
                }
                c.state("blocked", "resource_limit", None);
                c.tick();
            }
            "flush-refusal" => {
                let text = "Sorry Q1ABC is an invalid callsign";
                c.input(1, &format!("{text}\r"));
                c.tick();
                c.step(2, json!({"type":"transport","attempt":1,"outcome":"eof"}));
                let e = c.content(text, "notice", "cr");
                c.cancel("identity");
                c.state("blocked", "refused", Some(&e));
                c.tick();
            }
            _ => unreachable!(),
        }
        cases.push(c.finish());
    }
    let mut c = Case::new("eight-retries-exhausted", "rbn");
    c.step(0, json!({"type":"start"}));
    c.state("connecting", "started", None);
    c.action(json!({"type":"connect","attempt":1}));
    c.arm("connect", 30000);
    c.tick();
    let mut at = 1;
    for (i, delay) in [5000, 10000, 20000, 40000, 80000, 160000, 300000, 300000, 0]
        .into_iter()
        .enumerate()
    {
        c.step(
            at,
            json!({"type":"transport","attempt":i+1,"outcome":"error"}),
        );
        if i > 0 {
            c.at = at - 1;
            c.state("connecting", "started", None);
            c.action(json!({"type":"connect","attempt":i+1}));
            c.arm("connect", c.at + 30000);
            c.at = at;
        }
        c.cancel("connect");
        c.state("disconnected", "transport_error", None);
        if i == 8 {
            c.state("blocked", "retry_exhausted", None);
        } else {
            c.event(oracle::local(
                "rbn",
                "backoff",
                "retry_scheduled",
                Some(delay),
            ));
            c.v["expected_state"] = json!("backoff");
            c.arm("retry", at + delay);
        }
        c.tick();
        at += delay + 1;
    }
    cases.push(c.finish());
    for restart in [false, true] {
        let mut c = Case::new(
            if restart {
                "explicit-restart"
            } else {
                "cancel-at-retry-deadline"
            },
            "rbn",
        );
        c.begin();
        c.login("login:");
        c.step(1, json!({"type":"transport","attempt":1,"outcome":"eof"}));
        c.cancel("identity");
        c.state("disconnected", "eof", None);
        c.event(oracle::local(
            "rbn",
            "backoff",
            "retry_scheduled",
            Some(5000),
        ));
        c.arm("retry", 5001);
        c.tick();
        c.step(5001, json!({"type":if restart{"restart"}else{"cancel"}}));
        c.cancel("retry");
        c.state("disconnected", "cancelled", None);
        if restart {
            c.attempt = 2;
            c.state("connecting", "started", None);
            c.action(json!({"type":"connect","attempt":2}));
            c.arm("connect", 35001);
        }
        c.tick();
        if !restart {
            c.step(100000, json!({"type":"tick"}));
        }
        cases.push(c.finish());
    }
    for duration in [599999, 600000] {
        let mut c = Case::new(&format!("stable-ready-reset-{duration}"), "rbn");
        c.begin();
        c.login("login:");
        c.step(1, json!({"type":"transport","attempt":1,"outcome":"eof"}));
        c.cancel("identity");
        c.state("disconnected", "eof", None);
        c.event(oracle::local(
            "rbn",
            "backoff",
            "retry_scheduled",
            Some(5000),
        ));
        c.arm("retry", 5001);
        c.tick();
        c.step(
            5002,
            json!({"type":"transport","attempt":2,"outcome":"connected"}),
        );
        c.at = 5001;
        c.state("connecting", "started", None);
        c.action(json!({"type":"connect","attempt":2}));
        c.arm("connect", 35001);
        c.at = 5002;
        c.cancel("connect");
        c.state("await_login", "connected", None);
        c.arm("login", 35002);
        c.attempt = 2;
        c.input(5002, "call:");
        c.content("call:", "login_prompt", "none");
        c.cancel("login");
        c.output("Q1ABC");
        c.state("await_identity", "login_sent", None);
        c.arm("identity", 35002);
        c.tick();
        let prompt = "Q1ABC de RELAY 1-Nov-2026 18:32Z >";
        c.input(5102, prompt);
        let e = c.content(prompt, "ready_prompt", "none");
        c.cancel("identity");
        c.state("setup", "identified", Some(&e));
        c.state("ready", "setup_complete", Some(&e));
        c.arm("stable_ready", 605102);
        c.tick();
        let at = 5102 + duration;
        c.step(at, json!({"type":"transport","attempt":2,"outcome":"eof"}));
        if duration < 600000 {
            c.cancel("stable_ready");
        }
        c.state("disconnected", "eof", None);
        let delay = if duration < 600000 { 10000 } else { 5000 };
        c.event(oracle::local(
            "rbn",
            "backoff",
            "retry_scheduled",
            Some(delay),
        ));
        c.arm("retry", at + delay);
        c.v["expected_state"] = json!("backoff");
        c.tick();
        cases.push(c.finish());
    }
    let mut c = Case::new("silence-timeout-no-keepalive", "rbn");
    c.v["config"]["silence_ms"] = json!(900000);
    c.begin();
    c.login("login:");
    c.ready("rbn", "Q1ABC de RELAY 1-Nov-2026 18:32Z >");
    let index = c.v["steps"].as_array().unwrap().len() - 2;
    c.step = index;
    c.at = 100;
    c.arm("silence", 900100);
    c.step(900100, json!({"type":"tick"}));
    c.action(json!({"type":"close","attempt":1}));
    c.state("disconnected", "silence_timeout", None);
    c.event(oracle::local(
        "rbn",
        "backoff",
        "retry_scheduled",
        Some(5000),
    ));
    c.arm("retry", 905100);
    c.v["expected_state"] = json!("backoff");
    cases.push(c.finish());
    cases
}

fn mutations(cases: &[Value]) -> Vec<Value> {
    let mut out = vec![];
    let base = cases
        .iter()
        .find(|v| v["id"] == "startup-dxspider-dxspider")
        .unwrap();
    for split in [false, true] {
        let mut v = base.clone();
        v["id"] = json!(format!("ready-then-displaced-{split}"));
        let index = v["steps"]
            .as_array()
            .unwrap()
            .iter()
            .rposition(|s| s["type"] == "input")
            .unwrap();
        let at = v["steps"][index]["elapsed_ms"].as_u64().unwrap();
        let notice = "Reconnected as Q1ABC at <ip>, this instance is disconnected";
        let notice_step = if split { index + 1 } else { index };
        if split {
            v["steps"].as_array_mut().unwrap().insert(index+1,json!({"type":"input","attempt":1,"elapsed_ms":at,"b64":oracle::b64(format!("{notice}\n").as_bytes())}));
        } else {
            let mut b = oracle::unb64(v["steps"][index]["b64"].as_str().unwrap()).unwrap();
            b.extend_from_slice(format!("{notice}\n").as_bytes());
            v["steps"][index]["b64"] = json!(oracle::b64(&b));
        }
        let event = oracle::event(
            "dxspider",
            "prompt_status",
            json!({"status":"notice","text":notice}),
            notice.as_bytes(),
            "lf",
            "exact",
        );
        let mut state = event.clone();
        state["kind"] = json!("connection_state");
        state["fields"] = json!({"state":"blocked","reason":"displaced"});
        for e in [event, state] {
            v["expected_events"]
                .as_array_mut()
                .unwrap()
                .push(json!({"step":notice_step,"elapsed_ms":at,"event":e}));
        }
        for a in [
            json!({"type":"cancel_timer","timer":"stable_ready"}),
            json!({"type":"close","attempt":1}),
        ] {
            v["expected_actions"]
                .as_array_mut()
                .unwrap()
                .push(json!({"step":notice_step,"elapsed_ms":at,"action":a}));
        }
        v["expected_state"] = json!("blocked");
        out.push(v);
    }
    let mut v = base.clone();
    v["id"] = json!("literal-custom-prompt");
    v["config"]["custom_prompt"] = json!("Q custom:>");
    let prompt = "Q1ABC de Q2ABC 1-Nov-2026 1832Z dxspider >";
    for step in v["steps"].as_array_mut().unwrap() {
        if step["type"] == "input" {
            let b = oracle::unb64(step["b64"].as_str().unwrap()).unwrap();
            let s = String::from_utf8(b).unwrap().replace(prompt, "Q custom:>");
            step["b64"] = json!(oracle::b64(s.as_bytes()));
        }
    }
    for e in v["expected_events"].as_array_mut().unwrap() {
        let event = &mut e["event"];
        if oracle::unb64(event["raw_line"].as_str().unwrap()).unwrap() == prompt.as_bytes() {
            event["raw_line"] = json!(oracle::b64(b"Q custom:>"));
            if event["fields"].get("text").is_some() {
                event["fields"]["text"] = json!("Q custom:>");
            }
        }
    }
    out.push(v);
    let mut v = cases
        .iter()
        .find(|v| v["id"] == "startup-rbn-rbn")
        .unwrap()
        .clone();
    v["id"] = json!("identity-ascii-case");
    v["config"]["identity"] = json!("q1abc");
    for a in v["expected_actions"].as_array_mut().unwrap() {
        if a["action"]["type"] == "output" {
            a["action"]["b64"] = json!(oracle::b64(b"q1abc\r\n"));
        }
    }
    out.push(v);
    let mut c = Case::new("configured-setup-error", "cccluster");
    c.v["config"]["supported_setup_errors"] = json!(["Q setup rejected"]);
    c.begin();
    c.login("login:");
    c.ready("cccluster", "Q1ABC de Q2ABC 1-Nov-2026 1832Z CCC >");
    c.step(1000, json!({"type":"tick"}));
    c.output("set/ve7cc");
    c.arm("setup", 6000);
    c.input(1100, "Q setup rejected\n");
    let e = c.content("Q setup rejected", "setup_error", "lf");
    c.cancel("setup");
    c.action(json!({"type":"close","attempt":1}));
    c.state("blocked", "setup_failed", Some(&e));
    c.tick();
    out.push(c.finish());
    out
}

fn application_and_validation() -> Vec<Value> {
    let mut out = vec![];
    for identity in ["Q1ABC-0", "Q1ABC-00", "Q1ABC-01"] {
        let mut c = Case::new(&format!("reject-{identity}"), "auto");
        c.v["config"]["identity"] = json!(identity);
        c.v["expected_config_error"] = json!("invalid login token or SSID");
        c.v.as_object_mut().unwrap().remove("expected_state");
        c.v["provenance"]["derived_from"] = json!("QGDXC-IO-0003");
        out.push(c.finish());
    }
    for prompt in ["missing terminator".into(), format!("{}>", "Q".repeat(256))] {
        let mut c = Case::new(
            &format!("reject-custom-prompt-{}", prompt.len()),
            "dxspider",
        );
        c.v["config"]["line_limit"] = json!(256);
        c.v["config"]["custom_prompt"] = json!(prompt);
        c.v["expected_config_error"] =
            json!("custom prompt must fit the line bound and end in > or :");
        c.v.as_object_mut().unwrap().remove("expected_state");
        c.v["provenance"]["derived_from"] = json!("QGDXC-DSP-0002");
        out.push(c.finish());
    }
    for phase in ["login", "identity", "setup"] {
        for suffix in ["partial", "pending\r"] {
            let mut c = Case::new(
                &format!("deadline-{phase}-flush-{}", suffix.len()),
                if phase == "setup" {
                    "cccluster"
                } else {
                    "auto"
                },
            );
            c.begin();
            if phase != "login" {
                c.login("login:");
            } else {
                c.tick();
            }
            let deadline = if phase == "setup" {
                c.ready("cccluster", "Q1ABC de Q2ABC 1-Nov-2026 1832Z CCC >");
                c.step(1000, json!({"type":"tick"}));
                c.output("set/ve7cc");
                c.arm("setup", 6000);
                6000
            } else {
                30000
            };
            c.input(1100, suffix);
            c.tick();
            c.step(deadline, json!({"type":"tick"}));
            c.event(oracle::line(
                &c.profile,
                suffix.trim_end_matches('\r').as_bytes(),
                if suffix.ends_with('\r') { "cr" } else { "none" },
                (!suffix.ends_with('\r')).then_some("partial"),
            ));
            c.action(json!({"type":"close","attempt":1}));
            if phase == "login" {
                c.state("disconnected", "timeout", None);
                c.event(oracle::local(
                    &c.profile,
                    "backoff",
                    "retry_scheduled",
                    Some(5000),
                ));
                c.arm("retry", deadline + 5000);
                c.v["expected_state"] = json!("backoff");
            } else {
                c.state(
                    "blocked",
                    if phase == "setup" {
                        "setup_failed"
                    } else {
                        "profile_unknown"
                    },
                    None,
                );
            }
            out.push(c.finish());
        }
    }
    for suffix in ["partial", "pending\r"] {
        let mut c = Case::new(&format!("restart-flush-{}", suffix.len()), "rbn");
        c.begin();
        c.login("login:");
        c.input(1, suffix);
        c.tick();
        c.step(2, json!({"type":"restart"}));
        c.event(oracle::line(
            "rbn",
            suffix.trim_end_matches('\r').as_bytes(),
            if suffix.ends_with('\r') { "cr" } else { "none" },
            (!suffix.ends_with('\r')).then_some("partial"),
        ));
        c.cancel("identity");
        c.action(json!({"type":"close","attempt":1}));
        c.state("disconnected", "cancelled", None);
        c.state("connecting", "started", None);
        c.action(json!({"type":"connect","attempt":2}));
        c.arm("connect", 30002);
        c.tick();
        out.push(c.finish());
    }
    let mut c = Case::new("application-recording-replay-multiple-connections", "rbn");
    c.v["class"] = json!("Application");
    c.v["provenance"]["derived_from"] = json!("QGDXC-REC-0004 QGDXC-REC-0005 QGDXC-SES-0011");
    c.begin();
    c.login("login:");
    let prompt = "Q1ABC de RELAY 1-Nov-2026 18:32Z >";
    c.ready("rbn", prompt);
    c.step(200, json!({"type":"cancel"}));
    c.cancel("stable_ready");
    c.action(json!({"type":"close","attempt":1}));
    c.state("disconnected", "cancelled", None);
    c.tick();
    c.attempt = 2;
    c.step(500, json!({"type":"start"}));
    c.state("connecting", "started", None);
    c.action(json!({"type":"connect","attempt":2}));
    c.arm("connect", 30500);
    c.step(
        500,
        json!({"type":"transport","attempt":2,"outcome":"connected"}),
    );
    c.cancel("connect");
    c.state("await_login", "connected", None);
    c.arm("login", 30500);
    c.input(500, "login:");
    c.content("login:", "login_prompt", "none");
    c.cancel("login");
    c.arm("pacing", 1000);
    c.tick();
    c.step(1000, json!({"type":"tick"}));
    c.output("Q1ABC");
    c.state("await_identity", "login_sent", None);
    c.arm("identity", 31000);
    c.input(1100, prompt);
    let e = c.content(prompt, "ready_prompt", "none");
    c.cancel("identity");
    c.state("setup", "identified", Some(&e));
    c.state("ready", "setup_complete", Some(&e));
    c.arm("stable_ready", 601100);
    c.tick();
    c.step(1200, json!({"type":"cancel"}));
    c.cancel("stable_ready");
    c.action(json!({"type":"close","attempt":2}));
    c.state("disconnected", "cancelled", None);
    c.tick();
    out.push(c.finish());
    out
}

fn inert_status_text() -> Vec<Value> {
    let mut out = vec![];
    for (label, body) in [
        (
            "displacement",
            "Reconnected as Q1ABC at <ip>, this instance is disconnected",
        ),
        ("refusal", "Sorry Q1ABC is an invalid callsign"),
    ] {
        for (kind, prefix) in [
            ("announcement", "To ALL de Q2ABC: "),
            ("talk", "Q1ABC de Q2ABC: "),
        ] {
            let mut c = Case::new(&format!("relayed-{label}-{kind}"), "dxspider");
            c.v["provenance"]["derived_from"] = json!("QGDXC-SES-0012 QGDXC-DSP-0003");
            c.begin();
            c.login("login:");
            let line = format!("{prefix}{body}");
            c.input(1, &format!("{line}\n"));
            c.event(oracle::line("dxspider", line.as_bytes(), "lf", None));
            c.tick();
            out.push(c.finish());
        }
    }
    let mut c = Case::new("truncated-displacement-prefix", "dxspider");
    c.v["config"]["line_limit"] = json!(256);
    c.v["provenance"]["derived_from"] = json!("QGDXC-SES-0012 QGDXC-DSP-0003 QGDXC-FRM-0003");
    c.begin();
    c.login("login:");
    let notice = "Reconnected as Q1ABC at <ip>, this instance is disconnected";
    let prefix = format!("{notice}{}", " ".repeat(256 - notice.len()));
    c.input(1, &format!("{prefix}extra\n"));
    c.event(oracle::line(
        "dxspider",
        prefix.as_bytes(),
        "none",
        Some("line_too_long"),
    ));
    c.tick();
    out.push(c.finish());
    out
}

fn application_retry_exhaustion() -> Value {
    let mut c = Case::new("application-replay-retry-exhaustion", "rbn");
    c.v["class"] = json!("Application");
    c.v["provenance"]["derived_from"] = json!("QGDXC-REC-0004 QGDXC-SES-0010");
    c.begin();
    c.tick();
    let mut close_at = 1000;
    for delay in [5000, 10000, 20000, 40000, 80000, 160000, 300000, 300000, 0] {
        c.step(
            close_at,
            json!({"type":"transport","attempt":c.attempt,"outcome":"eof"}),
        );
        c.cancel("login");
        c.state("disconnected", "eof", None);
        if delay == 0 {
            c.state("blocked", "retry_exhausted", None);
            c.tick();
            break;
        }
        c.event(oracle::local(
            "rbn",
            "backoff",
            "retry_scheduled",
            Some(delay),
        ));
        c.v["expected_state"] = json!("backoff");
        c.arm("retry", close_at + delay);
        c.tick();
        let retry_at = close_at + delay;
        c.attempt += 1;
        c.step(retry_at, json!({"type":"tick"}));
        c.state("connecting", "started", None);
        c.action(json!({"type":"connect","attempt":c.attempt}));
        c.arm("connect", retry_at + 30000);
        c.step(
            retry_at + 1,
            json!({"type":"transport","attempt":c.attempt,"outcome":"connected"}),
        );
        c.cancel("connect");
        c.state("await_login", "connected", None);
        c.arm("login", retry_at + 30001);
        c.tick();
        close_at = retry_at + 1001;
    }
    c.finish()
}
