// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{Context, Event, Parser, Profile, Value, parser, values};
use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub call: String,
    pub profile: Profile,
    pub line_limit: usize,
    pub retain_third_party_data: bool,
    pub connect_ms: u64,
    pub login_ms: u64,
    pub identity_ms: u64,
    pub setup_ms: u64,
    pub silence_ms: Option<u64>,
    pub spacing_ms: u64,
    pub window_ms: u64,
    pub window_lines: usize,
    pub custom_prompt: Option<String>,
    pub supported_setup_errors: Vec<String>,
}
impl Config {
    pub fn new(host: String, port: u16, call: String, profile: Profile) -> Self {
        Self {
            host,
            port,
            call,
            profile,
            line_limit: 4096,
            retain_third_party_data: false,
            connect_ms: 30000,
            login_ms: 30000,
            identity_ms: 30000,
            setup_ms: 30000,
            silence_ms: None,
            spacing_ms: 1000,
            window_ms: 20000,
            window_lines: 20,
            custom_prompt: None,
            supported_setup_errors: Vec::new(),
        }
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.host.is_empty()
            || self.host.len() > 253
            || self.host.chars().any(char::is_control)
            || self.port == 0
        {
            return Err("invalid endpoint");
        }
        if !values::login(&self.call) {
            return Err("invalid login token or SSID");
        }
        if !(256..=65536).contains(&self.line_limit) {
            return Err("invalid line limit");
        }
        if [
            self.connect_ms,
            self.login_ms,
            self.identity_ms,
            self.setup_ms,
        ]
        .iter()
        .any(|t| !(5000..=120000).contains(t))
        {
            return Err("startup timeout must be 5000–120000 ms");
        }
        if self
            .silence_ms
            .is_some_and(|t| !(900000..=values::MAX_INTEGER).contains(&t))
        {
            return Err("silence timeout must be at least 900000 ms");
        }
        if self.spacing_ms < 1000
            || self.spacing_ms > values::MAX_INTEGER
            || self.window_ms < 20000
            || self.window_ms > values::MAX_INTEGER
            || self.window_lines == 0
            || self.window_lines > 20
        {
            return Err("invalid output pacing");
        }
        if self.supported_setup_errors.len() > 16
            || self
                .supported_setup_errors
                .iter()
                .any(|s| s.is_empty() || s.len() > self.line_limit)
        {
            return Err("invalid setup-error recognizer configuration");
        }
        if self.custom_prompt.as_ref().is_some_and(|s| {
            s.len() > self.line_limit || !matches!(s.as_bytes().last(), Some(b'>' | b':'))
        }) {
            return Err("custom prompt must fit the line bound and end in > or :");
        }
        Ok(())
    }
    pub fn canonical_identity(&self) -> String {
        self.call.to_ascii_uppercase()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Disconnected,
    Connecting,
    AwaitLogin,
    AwaitIdentity,
    Setup,
    Ready,
    Backoff,
    Blocked,
}
impl State {
    pub fn name(self) -> &'static str {
        match self {
            Self::Disconnected => "disconnected",
            Self::Connecting => "connecting",
            Self::AwaitLogin => "await_login",
            Self::AwaitIdentity => "await_identity",
            Self::Setup => "setup",
            Self::Ready => "ready",
            Self::Backoff => "backoff",
            Self::Blocked => "blocked",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(usize)]
pub enum Timer {
    Connect,
    Login,
    Identity,
    Setup,
    StableReady,
    Silence,
    Retry,
    Pacing,
}
impl Timer {
    pub const ALL: [Self; 8] = [
        Self::Connect,
        Self::Login,
        Self::Identity,
        Self::Setup,
        Self::StableReady,
        Self::Silence,
        Self::Retry,
        Self::Pacing,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Connect => "connect",
            Self::Login => "login",
            Self::Identity => "identity",
            Self::Setup => "setup",
            Self::StableReady => "stable_ready",
            Self::Silence => "silence",
            Self::Retry => "retry",
            Self::Pacing => "pacing",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Connect { attempt: u64 },
    Close { attempt: u64 },
    Output { attempt: u64, bytes: Vec<u8> },
    ArmTimer { timer: Timer, deadline_ms: u64 },
    CancelTimer { timer: Timer },
}
impl Action {
    pub fn json(&self) -> String {
        match self {
            Self::Connect { attempt } => format!("{{\"type\":\"connect\",\"attempt\":{attempt}}}"),
            Self::Close { attempt } => format!("{{\"type\":\"close\",\"attempt\":{attempt}}}"),
            Self::Output { attempt, bytes } => format!(
                "{{\"type\":\"output\",\"attempt\":{attempt},\"b64\":{}}}",
                crate::model::quote(&crate::model::base64(bytes))
            ),
            Self::ArmTimer { timer, deadline_ms } => format!(
                "{{\"type\":\"arm_timer\",\"timer\":\"{}\",\"deadline_ms\":{deadline_ms}}}",
                timer.name()
            ),
            Self::CancelTimer { timer } => format!(
                "{{\"type\":\"cancel_timer\",\"timer\":\"{}\"}}",
                timer.name()
            ),
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct Batch {
    pub events: Vec<Event>,
    pub actions: Vec<Action>,
}
/// A single endpoint/identity owner. An action must finish before admitting the
/// next stimulus; in particular Close completes before a subsequent Connect.
#[derive(Debug)]
pub struct Session {
    pub config: Config,
    pub state: State,
    pub attempt: u64,
    pub profile: Profile,
    parser: Parser,
    timers: [Option<u64>; 8],
    now: u64,
    tokens: VecDeque<String>,
    jitter: VecDeque<u64>,
    token: String,
    retries: usize,
    transport: bool,
    pending: Option<String>,
    rate: VecDeque<u64>,
    setup_step: usize,
    ack: bool,
    outstanding: bool,
    evidence: Option<Profile>,
    terminal: Option<&'static str>,
    terminal_evidence: Option<Event>,
    flushing: bool,
}
impl Session {
    pub fn new(
        config: Config,
        tokens: Vec<String>,
        jitter: Vec<u64>,
    ) -> Result<Self, &'static str> {
        config.validate()?;
        if tokens.len() > 1024 || jitter.len() > 1024 || tokens.iter().any(|t| !values::token(t)) {
            return Err("invalid or excessive readiness tokens/jitter");
        }
        let mut unique = tokens.clone();
        unique.sort();
        unique.dedup();
        if unique.len() != tokens.len() {
            return Err("readiness tokens must differ for every attempt");
        }
        if jitter.iter().any(|j| *j > 60000) {
            return Err("jitter exceeds maximum");
        }
        let mut context = Context::ready(config.profile);
        context.identity = config.call.clone();
        context.retain_third_party_data = config.retain_third_party_data;
        context.custom_prompt = config.custom_prompt.clone();
        let parser = Parser::new(context, config.line_limit)?;
        let profile = config.profile;
        Ok(Self {
            config,
            state: State::Disconnected,
            attempt: 0,
            profile,
            parser,
            timers: [None; 8],
            now: 0,
            tokens: tokens.into(),
            jitter: jitter.into(),
            token: String::new(),
            retries: 0,
            transport: false,
            pending: None,
            rate: VecDeque::new(),
            setup_step: 0,
            ack: false,
            outstanding: false,
            evidence: None,
            terminal: None,
            terminal_evidence: None,
            flushing: false,
        })
    }
    fn at(&mut self, now: u64) -> Result<(), &'static str> {
        if now < self.now || now > values::MAX_INTEGER {
            return Err("time must be nondecreasing and exact");
        }
        self.now = now;
        Ok(())
    }
    pub fn current_time(&self) -> u64 {
        self.now
    }
    pub fn deadline(&self, t: Timer) -> Option<u64> {
        self.timers[t as usize]
    }
    pub fn next_timer(&self) -> Option<(u64, Timer)> {
        Timer::ALL
            .into_iter()
            .filter_map(|t| self.deadline(t).map(|d| (d, t)))
            .min()
    }
    fn arm(&mut self, t: Timer, d: u64, b: &mut Batch) {
        if self.deadline(t) == Some(d) {
            return;
        }
        self.cancel_timer(t, b);
        self.timers[t as usize] = Some(d);
        b.actions.push(Action::ArmTimer {
            timer: t,
            deadline_ms: d,
        });
    }
    fn cancel_timer(&mut self, t: Timer, b: &mut Batch) {
        if self.timers[t as usize].take().is_some() {
            b.actions.push(Action::CancelTimer { timer: t });
        }
    }
    fn change(
        &mut self,
        state: State,
        reason: &'static str,
        evidence: Option<&Event>,
        retry: Option<u64>,
        b: &mut Batch,
    ) {
        self.state = state;
        let mut e = evidence
            .cloned()
            .unwrap_or_else(|| Event::new(self.profile, "connection_state"));
        e.kind = "connection_state";
        e.fields.clear();
        e.server_type = self.profile;
        e.set("state", state.name());
        e.set("reason", reason);
        if let Some(r) = retry {
            e.set("retry_in_ms", Value::Integer(r as i64));
        }
        b.events.push(e);
    }
    fn context(&mut self) {
        let expected = if self.state == State::Setup && self.outstanding {
            Some(self.reply())
        } else {
            None
        };
        let c = &mut self.parser.context;
        c.profile = self.profile;
        c.login = self.state == State::AwaitLogin;
        c.password = matches!(self.state, State::AwaitLogin | State::AwaitIdentity);
        c.post_login = matches!(
            self.state,
            State::AwaitIdentity | State::Setup | State::Ready
        );
        c.identity_context = self.state == State::AwaitIdentity;
        c.expected_reply = expected;
    }
    fn begin(&mut self, b: &mut Batch) -> Result<(), &'static str> {
        if matches!(self.config.profile, Profile::Auto | Profile::DxSpider) {
            self.token = self
                .tokens
                .pop_front()
                .ok_or("missing readiness token for connection attempt")?;
        }
        self.attempt = self
            .attempt
            .checked_add(1)
            .filter(|n| *n <= values::MAX_INTEGER)
            .ok_or("attempt limit")?;
        self.terminal = None;
        self.terminal_evidence = None;
        self.profile = self.config.profile;
        self.evidence = None;
        self.setup_step = 0;
        self.ack = false;
        self.outstanding = false;
        self.change(State::Connecting, "started", None, None, b);
        b.actions.push(Action::Connect {
            attempt: self.attempt,
        });
        self.arm(Timer::Connect, self.now + self.config.connect_ms, b);
        self.context();
        Ok(())
    }
    pub fn start(&mut self, now: u64) -> Result<Batch, &'static str> {
        self.at(now)?;
        let mut b = Batch::default();
        if matches!(self.state, State::Disconnected | State::Blocked) {
            self.retries = 0;
            self.begin(&mut b)?;
        }
        Ok(b)
    }
    pub fn restart(&mut self, now: u64) -> Result<Batch, &'static str> {
        let mut b = self.cancel(now)?;
        self.retries = 0;
        self.begin(&mut b)?;
        Ok(b)
    }
    pub fn cancel(&mut self, now: u64) -> Result<Batch, &'static str> {
        self.at(now)?;
        let mut b = Batch::default();
        if !matches!(self.state, State::Disconnected | State::Blocked) {
            self.end("cancelled", None, false, &mut b)?;
        } else {
            for t in Timer::ALL {
                self.cancel_timer(t, &mut b);
            }
            self.terminal = None;
        }
        Ok(b)
    }
    pub fn connected(&mut self, now: u64, attempt: u64) -> Result<Batch, &'static str> {
        self.at(now)?;
        let mut b = Batch::default();
        if attempt != self.attempt {
            return Ok(b);
        }
        if self.state != State::Connecting {
            return Err("impossible transport success");
        }
        self.transport = true;
        self.cancel_timer(Timer::Connect, &mut b);
        self.change(State::AwaitLogin, "connected", None, None, &mut b);
        self.arm(Timer::Login, now + self.config.login_ms, &mut b);
        self.context();
        Ok(b)
    }
    pub fn input(
        &mut self,
        now: u64,
        attempt: u64,
        byte: u8,
        received_at: Option<&str>,
    ) -> Result<Batch, &'static str> {
        self.at(now)?;
        let mut b = Batch::default();
        if attempt != self.attempt
            || !self.transport
            || matches!(
                self.state,
                State::Disconnected | State::Blocked | State::Backoff
            )
        {
            return Ok(b);
        }
        self.context();
        let parsed = self.parser.push(byte, received_at);
        // Stripped commands do not count as application-data activity.
        if let Ok(events) = parsed {
            if self.state == State::Ready
                && (self.parser_application(byte) || !events.is_empty())
                && let Some(ms) = self.config.silence_ms
            {
                self.arm(Timer::Silence, now + ms, &mut b);
            }
            for e in events {
                self.content(e, &mut b)?;
            }
        } else {
            self.end("resource_limit", None, false, &mut b)?;
        }
        Ok(b)
    }
    fn parser_application(&self, _byte: u8) -> bool {
        self.parser.last_application()
    }
    fn content(&mut self, e: Event, b: &mut Batch) -> Result<(), &'static str> {
        let status = e.get("status").map(str::to_owned);
        let text = e.get("text").unwrap_or("").to_owned();
        b.events.push(e.clone());
        let raw = values::decode(&e.raw_line).0;
        if self.parser.context.password && status.as_deref() == Some("password_prompt") {
            self.end("password_unsupported", Some(&e), false, b)?;
            return Ok(());
        }
        if let Some(reason) = parser::notice(&text, &self.parser.context) {
            self.end(reason, Some(&e), false, b)?;
            return Ok(());
        }
        if status.as_deref() == Some("login_prompt") && self.state == State::AwaitLogin {
            self.cancel_timer(Timer::Login, b);
            self.pending = Some(self.config.call.clone());
            self.admit(b)?;
            return Ok(());
        }
        if matches!(
            self.state,
            State::AwaitLogin | State::AwaitIdentity | State::Setup | State::Ready
        ) {
            let detected = if status.as_deref() == Some("ready_prompt") {
                parser::ready_profile(&raw, &self.parser.context)
            } else if status.as_deref() == Some("server_identity") {
                parser::identity_profile(&raw, &self.parser.context)
            } else {
                None
            };
            if let Some(p) = detected {
                if (self.config.profile != Profile::Auto && self.config.profile != p)
                    || self.evidence.is_some_and(|previous| previous != p)
                {
                    self.end("profile_mismatch", Some(&e), false, b)?;
                    return Ok(());
                }
                self.evidence = Some(p);
                self.profile = p;
                if p == Profile::ArCluster {
                    self.parser.context.ar_banner = true;
                }
            }
            if self.state == State::AwaitIdentity
                && status.as_deref() == Some("ready_prompt")
                && self.evidence.is_some()
            {
                self.cancel_timer(Timer::Identity, b);
                self.change(State::Setup, "identified", Some(&e), None, b);
                self.context();
                if matches!(self.profile, Profile::ArCluster | Profile::Rbn) {
                    self.ready(Some(&e), b);
                } else {
                    self.pending = Some(self.command());
                    self.admit(b)?;
                }
            } else if self.state == State::Setup && self.outstanding {
                if text == self.reply() && status.as_deref() == Some("setup_reply") {
                    self.ack = true;
                }
                if self
                    .config
                    .supported_setup_errors
                    .iter()
                    .any(|supported| supported == &text)
                {
                    self.end("setup_failed", Some(&e), false, b)?;
                } else if status.as_deref() == Some("ready_prompt") && self.ack {
                    self.cancel_timer(Timer::Setup, b);
                    self.outstanding = false;
                    self.ack = false;
                    self.setup_step += 1;
                    if self.profile == Profile::CcCluster || self.setup_step == 5 {
                        self.ready(Some(&e), b);
                    } else {
                        self.pending = Some(self.command());
                        self.admit(b)?;
                    }
                }
            }
        }
        self.context();
        Ok(())
    }
    fn command(&self) -> String {
        if self.profile == Profile::CcCluster {
            return "set/ve7cc".into();
        }
        match self.setup_step {
            0 => "set/page 0".into(),
            1 => "set/nobeep".into(),
            2 => "unset/echo".into(),
            3 => "set/ve7cc".into(),
            _ => format!("echo {}", self.token),
        }
    }
    fn reply(&self) -> String {
        if self.profile == Profile::CcCluster {
            return "Enhanced Spots Enabled".into();
        }
        match self.setup_step {
            0 => "Page Length is now 0 lines".into(),
            1 => "Beeps are now off".into(),
            2 => "Echoing disabled".into(),
            3 => "Operation successful".into(),
            _ => self.token.clone(),
        }
    }
    fn admit(&mut self, b: &mut Batch) -> Result<(), &'static str> {
        if self.pending.is_none() {
            return Ok(());
        }
        while self
            .rate
            .front()
            .is_some_and(|t| self.now.saturating_sub(*t) >= self.config.window_ms)
        {
            self.rate.pop_front();
        }
        let mut at = self.rate.back().map_or(self.now, |t| {
            self.now.max(t.saturating_add(self.config.spacing_ms))
        });
        if self.rate.len() >= self.config.window_lines {
            at = at.max(self.rate[0].saturating_add(self.config.window_ms));
        }
        if at > self.now {
            self.arm(Timer::Pacing, at, b);
            return Ok(());
        }
        self.cancel_timer(Timer::Pacing, b);
        let mut bytes = self
            .pending
            .take()
            .ok_or("missing pending output")?
            .into_bytes();
        bytes.extend_from_slice(b"\r\n");
        self.rate.push_back(self.now);
        b.actions.push(Action::Output {
            attempt: self.attempt,
            bytes,
        });
        if self.state == State::AwaitLogin {
            self.change(State::AwaitIdentity, "login_sent", None, None, b);
            self.arm(Timer::Identity, self.now + self.config.identity_ms, b);
        } else {
            self.outstanding = true;
            self.arm(Timer::Setup, self.now + self.config.setup_ms, b);
        }
        self.context();
        Ok(())
    }
    fn ready(&mut self, e: Option<&Event>, b: &mut Batch) {
        self.change(State::Ready, "setup_complete", e, None, b);
        self.arm(Timer::StableReady, self.now + 600000, b);
        if let Some(ms) = self.config.silence_ms {
            self.arm(Timer::Silence, self.now + ms, b);
        }
        self.context();
    }
    pub fn transport_end(
        &mut self,
        now: u64,
        attempt: u64,
        reason: &'static str,
    ) -> Result<Batch, &'static str> {
        self.at(now)?;
        let mut b = Batch::default();
        if attempt == self.attempt
            && !matches!(
                self.state,
                State::Disconnected | State::Blocked | State::Backoff
            )
        {
            self.end(reason, None, true, &mut b)?;
        }
        Ok(b)
    }
    fn end(
        &mut self,
        reason: &'static str,
        e: Option<&Event>,
        ended: bool,
        b: &mut Batch,
    ) -> Result<(), &'static str> {
        if self.flushing {
            if reason != "cancelled" && self.terminal.is_none() {
                self.terminal = Some(reason);
                self.terminal_evidence = e.cloned();
            }
            return Ok(());
        }
        self.flushing = true;
        let (events, telnet_failed) = self.parser.eof(None);
        for event in events {
            self.content(event, b)?;
        }
        self.flushing = false;
        let reason = if reason == "cancelled" {
            reason
        } else {
            self.terminal
                .or(telnet_failed.then_some("resource_limit"))
                .unwrap_or(reason)
        };
        let evidence = if reason == "cancelled" {
            e.cloned()
        } else {
            self.terminal_evidence.take().or_else(|| e.cloned())
        };
        let e = evidence.as_ref();
        for t in Timer::ALL {
            self.cancel_timer(t, b);
        }
        self.pending = None;
        self.outstanding = false;
        if !ended && (self.transport || self.state == State::Connecting) {
            b.actions.push(Action::Close {
                attempt: self.attempt,
            });
        }
        self.transport = false;
        self.parser = Parser::new(
            {
                let mut c = Context::ready(self.config.profile);
                c.identity = self.config.call.clone();
                c.retain_third_party_data = self.config.retain_third_party_data;
                c.custom_prompt = self.config.custom_prompt.clone();
                c
            },
            self.config.line_limit,
        )?;
        if matches!(
            reason,
            "eof" | "transport_error" | "timeout" | "silence_timeout"
        ) {
            self.change(State::Disconnected, reason, e, None, b);
            if self.retries == 8 {
                self.terminal = Some("retry_exhausted");
                self.change(State::Blocked, "retry_exhausted", None, None, b);
            } else {
                let base = [5000, 10000, 20000, 40000, 80000, 160000, 300000, 300000][self.retries];
                let jitter = self.jitter.pop_front().ok_or("missing retry jitter")?;
                if jitter > base / 5 {
                    return Err("retry jitter exceeds 20%");
                }
                self.retries += 1;
                let delay = base + jitter;
                self.change(State::Backoff, "retry_scheduled", None, Some(delay), b);
                self.arm(Timer::Retry, self.now + delay, b);
            }
        } else if reason == "cancelled" {
            self.terminal = None;
            self.change(State::Disconnected, reason, e, None, b);
        } else {
            self.terminal = Some(reason);
            self.change(State::Blocked, reason, e, None, b);
        }
        self.context();
        Ok(())
    }
    /// Fire exactly the timer returned by next_timer, at its logical deadline.
    pub fn fire(&mut self, t: Timer, now: u64) -> Result<Batch, &'static str> {
        self.at(now)?;
        let mut b = Batch::default();
        if self.deadline(t) != Some(now) {
            return Err("timer must fire at its deadline");
        }
        self.timers[t as usize] = None;
        match t {
            Timer::Connect | Timer::Login => self.end("timeout", None, false, &mut b)?,
            Timer::Identity => self.end("profile_unknown", None, false, &mut b)?,
            Timer::Setup => self.end("setup_failed", None, false, &mut b)?,
            Timer::StableReady => self.retries = 0,
            Timer::Silence => self.end("silence_timeout", None, false, &mut b)?,
            Timer::Retry => self.begin(&mut b)?,
            Timer::Pacing => self.admit(&mut b)?,
        }
        Ok(b)
    }
}
