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
        if self.profile == Profile::Pota {
            return Err("unsupported session profile");
        }
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
    /// Supply a fresh caller token with `Session::readiness_token` before continuing.
    ReadinessToken {
        attempt: u64,
    },
    /// Supply caller jitter with `Session::retry_jitter` before continuing.
    RetryJitter {
        maximum_ms: u64,
    },
    Connect {
        attempt: u64,
    },
    Close {
        attempt: u64,
    },
    Output {
        attempt: u64,
        bytes: Vec<u8>,
    },
    ArmTimer {
        timer: Timer,
        deadline_ms: u64,
    },
    CancelTimer {
        timer: Timer,
    },
}
impl Action {
    pub fn json(&self) -> String {
        match self {
            Self::ReadinessToken { attempt } => {
                format!("{{\"type\":\"readiness_token\",\"attempt\":{attempt}}}")
            }
            Self::RetryJitter { maximum_ms } => {
                format!("{{\"type\":\"retry_jitter\",\"maximum_ms\":{maximum_ms}}}")
            }
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
/// QGDXC-IO-0005: maximum events returned by input or a drain call.
pub const INPUT_EVENTS: usize = 8;
/// QGDXC-IO-0005: maximum admitted chunk, including its pending remainder.
pub const INPUT_BYTES: usize = 1024 * 1024;
#[derive(Debug)]
struct PendingInput {
    bytes: Vec<u8>,
    cursor: usize,
    attempt: u64,
    received_at: Option<String>,
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
    needs_token: bool,
    retry_base: Option<u64>,
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
    recorded_transport: bool,
    input_pending: Option<PendingInput>,
}
impl Session {
    pub fn new(config: Config) -> Result<Self, &'static str> {
        config.validate()?;
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
            needs_token: false,
            retry_base: None,
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
            recorded_transport: false,
            input_pending: None,
        })
    }
    /// Use QGDXC-REC-0004 transport observations. Select before starting;
    /// the adapter supplies complete outbound lines and recorded boundaries.
    pub fn with_recorded_transport(mut self) -> Self {
        self.recorded_transport = true;
        self
    }
    /// Observe a complete outbound line without transmitting received bytes.
    /// Only the locally pending login or setup literal can admit output.
    pub fn recorded_output(
        &mut self,
        now: u64,
        attempt: u64,
        bytes: &[u8],
    ) -> Result<Batch, &'static str> {
        self.at(now)?;
        let mut b = Batch::default();
        if !self.recorded_transport || attempt != self.attempt {
            return Err("unexpected recorded output admission");
        }
        if self.terminal.is_none()
            && self
                .pending
                .as_ref()
                .is_some_and(|line| bytes.strip_suffix(b"\r\n") == Some(line.as_bytes()))
        {
            self.emit_output(&mut b)?;
        }
        Ok(b)
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
        if expected.is_none() {
            c.setup_errors.clear();
        } else if c.expected_reply != expected {
            c.setup_errors
                .clone_from(&self.config.supported_setup_errors);
        }
        c.expected_reply = expected;
    }
    fn begin(&mut self, b: &mut Batch) -> Result<(), &'static str> {
        if self.attempt >= values::MAX_INTEGER {
            return Err("attempt limit");
        }
        self.begin_supplied(b)
    }
    /// Complete the QGDXC-DSP-0004 request, made only when DXSpider setup
    /// reaches its echo step. The caller owns token uniqueness across
    /// attempts; the engine retains only the current token.
    pub fn readiness_token(
        &mut self,
        now: u64,
        attempt: u64,
        token: String,
    ) -> Result<Batch, &'static str> {
        self.at(now)?;
        if !self.needs_token || attempt != self.attempt || self.state != State::Setup {
            return Err("unexpected readiness token");
        }
        if !values::token(&token) || token == self.token {
            return Err("invalid or repeated readiness token");
        }
        self.token = token;
        self.needs_token = false;
        let mut b = Batch::default();
        self.pending = Some(self.command());
        self.admit(&mut b)?;
        Ok(b)
    }
    fn begin_supplied(&mut self, b: &mut Batch) -> Result<(), &'static str> {
        self.attempt = self
            .attempt
            .checked_add(1)
            .filter(|n| *n <= values::MAX_INTEGER)
            .ok_or("attempt limit")?;
        self.terminal = None;
        self.terminal_evidence = None;
        self.needs_token = false;
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
    /// Resume an automatic attempt at an authoritative recorded boundary
    /// (QGDXC-REC-0004), preserving QGDXC-SES-0010 retry history.
    pub fn recorded_retry(&mut self, now: u64) -> Result<Batch, &'static str> {
        self.at(now)?;
        if !self.recorded_transport || self.state != State::Backoff {
            return Err("unexpected recorded retry");
        }
        let mut b = Batch::default();
        self.cancel_timer(Timer::Retry, &mut b);
        self.begin(&mut b)?;
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
        self.needs_token = false;
        self.retry_base = None;
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
    /// Admit a chunk under QGDXC-IO-0005. Consume the returned batch, then
    /// call `drain_input` while `has_pending_input` before offering more bytes.
    /// Each call returns at most INPUT_EVENTS events, stopping at the first
    /// byte that produces events or actions. Admission errors leave it unchanged.
    pub fn input_chunk(
        &mut self,
        now: u64,
        attempt: u64,
        bytes: &[u8],
        received_at: Option<&str>,
    ) -> Result<Batch, &'static str> {
        if self.has_pending_input() {
            return Err("drain pending input before admitting more bytes");
        }
        if bytes.len() > INPUT_BYTES {
            return Err("input chunk exceeds admission limit");
        }
        if received_at.is_some_and(|s| !values::receipt(s)) {
            return Err("invalid UTC receipt timestamp");
        }
        self.at(now)?;
        self.input_pending = Some(PendingInput {
            bytes: bytes.to_vec(),
            cursor: 0,
            attempt,
            received_at: received_at.map(str::to_owned),
        });
        self.drain_input()
    }
    pub fn has_pending_input(&self) -> bool {
        self.input_pending.is_some()
    }
    /// Continue the admitted chunk at the current session time. Actions from
    /// the previous batch must complete first, just as for byte input.
    pub fn drain_input(&mut self) -> Result<Batch, &'static str> {
        let Some(mut pending) = self.input_pending.take() else {
            return Ok(Batch::default());
        };
        let mut batch = Batch::default();
        while pending.cursor < pending.bytes.len() {
            let byte = pending.bytes[pending.cursor];
            pending.cursor += 1;
            batch = self.input_byte(
                self.now,
                pending.attempt,
                byte,
                pending.received_at.as_deref(),
            )?;
            if !batch.events.is_empty() || !batch.actions.is_empty() {
                break;
            }
        }
        if pending.cursor < pending.bytes.len() {
            self.input_pending = Some(pending);
        }
        Ok(batch)
    }
    pub fn input(
        &mut self,
        now: u64,
        attempt: u64,
        byte: u8,
        received_at: Option<&str>,
    ) -> Result<Batch, &'static str> {
        if self.has_pending_input() {
            return Err("drain pending input before admitting more bytes");
        }
        self.input_byte(now, attempt, byte, received_at)
    }
    fn input_byte(
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
        if self.recorded_transport && self.terminal.is_some() {
            return Ok(());
        }
        // Only complete parser-classified statuses can affect the session.
        if e.kind != "prompt_status" || e.raw_status == "truncated" {
            return Ok(());
        }
        let raw = values::decode(&e.raw_line).0;
        if self.parser.context.password && status.as_deref() == Some("password_prompt") {
            self.end("password_unsupported", Some(&e), false, b)?;
            return Ok(());
        }
        if status.as_deref() == Some("notice")
            && let Some(reason) = parser::notice(&text, &self.parser.context)
        {
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
                if status.as_deref() == Some("setup_error") {
                    self.end("setup_failed", Some(&e), false, b)?;
                } else if status.as_deref() == Some("ready_prompt") && self.ack {
                    self.cancel_timer(Timer::Setup, b);
                    self.outstanding = false;
                    self.ack = false;
                    self.setup_step += 1;
                    if self.profile == Profile::CcCluster || self.setup_step == 5 {
                        self.ready(Some(&e), b);
                    } else if self.setup_step == 4 {
                        // QGDXC-DSP-0004: only a DXSpider echo step needs a
                        // token, so other servers, including ones found by
                        // auto detection, never request one.
                        self.needs_token = true;
                        b.actions.push(Action::ReadinessToken {
                            attempt: self.attempt,
                        });
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
        if self.pending.is_none() || self.recorded_transport {
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
        self.emit_output(b)
    }
    fn emit_output(&mut self, b: &mut Batch) -> Result<(), &'static str> {
        let mut bytes = self
            .pending
            .take()
            .ok_or("missing pending output")?
            .into_bytes();
        bytes.extend_from_slice(b"\r\n");
        if !self.recorded_transport {
            self.rate.push_back(self.now);
        }
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
        if self.recorded_transport && self.transport && !ended {
            if self.terminal.is_none() {
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
                self.retry_base = Some(base);
                b.actions.push(Action::RetryJitter {
                    maximum_ms: base / 5,
                });
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
    /// Complete the QGDXC-SES-0010 request with one caller-supplied value.
    pub fn retry_jitter(&mut self, now: u64, jitter_ms: u64) -> Result<Batch, &'static str> {
        self.at(now)?;
        let base = self.retry_base.ok_or("unexpected retry jitter")?;
        if jitter_ms > base / 5 {
            return Err("retry jitter exceeds 20%");
        }
        self.retry_base = None;
        self.retries += 1;
        let delay = base + jitter_ms;
        let mut b = Batch::default();
        self.change(State::Backoff, "retry_scheduled", None, Some(delay), &mut b);
        self.arm(Timer::Retry, self.now + delay, &mut b);
        self.context();
        Ok(b)
    }
    /// Report actual transport admission after consuming an Output action.
    /// Call before admitting more input. The abstract vector harness admits
    /// immediately; adapters that delay output update pacing and reply time here.
    pub fn output_admitted(&mut self, now: u64, attempt: u64) -> Result<Batch, &'static str> {
        self.at(now)?;
        let mut b = Batch::default();
        if attempt != self.attempt {
            return Err("stale output admission");
        }
        let last = self.rate.back_mut().ok_or("no output to admit")?;
        *last = now;
        match self.state {
            State::AwaitIdentity => {
                self.arm(Timer::Identity, now + self.config.identity_ms, &mut b)
            }
            State::Setup if self.outstanding => {
                self.arm(Timer::Setup, now + self.config.setup_ms, &mut b)
            }
            _ => return Err("no startup output awaiting admission"),
        }
        Ok(b)
    }
    /// Deliver a due timer at caller time. Abstract harnesses use its deadline;
    /// live adapters use the current monotonic time after scheduling delays.
    pub fn fire(&mut self, t: Timer, now: u64) -> Result<Batch, &'static str> {
        self.at(now)?;
        let mut b = Batch::default();
        if !self.deadline(t).is_some_and(|deadline| deadline <= now) {
            return Err("timer must be due");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_resource_storage_stays_flat() {
        // QGDXC-SES-0010 QGDXC-SEC-0001 QGDXC-DSP-0004
        let config = Config::new(
            "example.invalid".into(),
            7300,
            "Q1ABC".into(),
            Profile::Auto,
        );
        let mut s = Session::new(config).unwrap();
        let mut now = 0;
        let mut request = s.start(now).unwrap();
        let mut capacities = None;
        let mut parser_reset = None;
        for attempt in 1..=8192 {
            // Auto detection of a non-DXSpider server requests no token.
            assert!(
                !request
                    .actions
                    .iter()
                    .any(|a| matches!(a, Action::ReadinessToken { .. }))
            );
            s.connected(now, attempt).unwrap();
            for byte in b"Please enter your call: \nQ1ABC de RELAY 1-Nov-2026 18:32Z >" {
                s.input(now, attempt, *byte, None).unwrap();
            }
            assert_eq!(s.state, State::Ready);
            now = s.deadline(Timer::StableReady).unwrap();
            s.fire(Timer::StableReady, now).unwrap();
            let ended = s.transport_end(now, attempt, "eof").unwrap();
            assert!(
                ended
                    .actions
                    .contains(&Action::RetryJitter { maximum_ms: 1000 })
            );
            s.retry_jitter(now, 0).unwrap();
            assert_eq!(s.deadline(Timer::Retry), Some(now + 5000));
            now += 5000;
            request = s.fire(Timer::Retry, now).unwrap();
            // Check the engine's retained attempt-related allocations, rather
            // than RSS or caller-owned traces/uniqueness sets. Framing and
            // context are reconstructed at close, with identical reset state.
            let retained = (s.token.capacity(), s.rate.capacity());
            assert_eq!(*capacities.get_or_insert(retained), retained);
            let reset = format!("{:?}", s.parser);
            assert_eq!(parser_reset.get_or_insert(reset.clone()), &reset);
            assert!(s.pending.is_none());
            assert!(s.terminal_evidence.is_none());
            assert!(s.retry_base.is_none());
            assert_eq!(s.rate.len(), 1);
        }
    }

    /// Drive a real DXSpider login and setup until the echo step, firing
    /// pacing as it comes due. Returns the batches seen and the final time.
    fn dxspider_to_echo_step(s: &mut Session, attempt: u64) -> (Vec<Batch>, u64) {
        let mut now = 0;
        let mut seen = vec![s.connected(now, attempt).unwrap()];
        let prompt = "\nQ1ABC de Q9XYZ 1-Nov-2026 1832Z dxspider >";
        let replies = [
            "login: ".to_owned(),
            format!("\nCapabilities: ve7cc rbn{prompt}"),
            format!("\nPage Length is now 0 lines{prompt}"),
            format!("\nBeeps are now off{prompt}"),
            format!("\nEchoing disabled{prompt}"),
            format!("\nOperation successful{prompt}"),
        ];
        for reply in replies {
            while let Some(d) = s.deadline(Timer::Pacing) {
                now = d;
                seen.push(s.fire(Timer::Pacing, now).unwrap());
            }
            for byte in reply.bytes() {
                seen.push(s.input(now, attempt, byte, None).unwrap());
            }
        }
        (seen, now)
    }

    #[test]
    fn readiness_token_requested_only_at_dxspider_echo_step() {
        // QGDXC-DSP-0004 QGDXC-REC-0004
        for profile in [Profile::Auto, Profile::DxSpider] {
            let mut s = Session::new(Config::new(
                "example.invalid".into(),
                7300,
                "Q1ABC".into(),
                profile,
            ))
            .unwrap();
            let started = s.start(0).unwrap();
            assert!(
                !started
                    .actions
                    .iter()
                    .any(|a| matches!(a, Action::ReadinessToken { .. }))
            );
            assert!(
                s.readiness_token(0, 1, "QGDXC0000000000000001".into())
                    .is_err()
            );
            let (seen, mut now) = dxspider_to_echo_step(&mut s, 1);
            assert_eq!(s.state, State::Setup);
            let requests: Vec<_> = seen
                .iter()
                .flat_map(|b| &b.actions)
                .filter(|a| matches!(a, Action::ReadinessToken { .. }))
                .collect();
            assert_eq!(requests, [&Action::ReadinessToken { attempt: 1 }]);
            let sent: Vec<_> = seen
                .iter()
                .flat_map(|b| &b.actions)
                .filter_map(|a| match a {
                    Action::Output { bytes, .. } => {
                        Some(String::from_utf8_lossy(bytes).into_owned())
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(
                sent,
                [
                    "Q1ABC\r\n",
                    "set/page 0\r\n",
                    "set/nobeep\r\n",
                    "unset/echo\r\n",
                    "set/ve7cc\r\n"
                ]
            );
            assert!(
                s.readiness_token(now, 2, "QGDXC0000000000000001".into())
                    .is_err()
            );
            assert!(s.readiness_token(now, 1, "bad".into()).is_err());
            let mut b = s
                .readiness_token(now, 1, "QGDXC0000000000000001".into())
                .unwrap();
            if let Some(d) = s.deadline(Timer::Pacing) {
                now = d;
                b = s.fire(Timer::Pacing, now).unwrap();
            }
            assert!(b.actions.contains(&Action::Output {
                attempt: 1,
                bytes: b"echo QGDXC0000000000000001\r\n".to_vec(),
            }));
            assert!(
                s.readiness_token(now, 1, "QGDXC0000000000000002".into())
                    .is_err()
            );
            s.cancel(now).unwrap();
            assert!(
                s.readiness_token(now, 1, "QGDXC0000000000000002".into())
                    .is_err()
            );
        }
    }
}
