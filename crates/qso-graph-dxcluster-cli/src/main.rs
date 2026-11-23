// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
use qso_graph_dxcluster::{model::escape, *};
use qso_graph_dxcluster_cli::recording;
use recording::{Recorder, ReplayStream};
mod output;
mod tui;
use output::Output;
use std::{
    fs::File,
    io::{self, BufReader, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
const HELP: &str = "qg-dxc reads live DX spots for radio operators and application developers.\n\nUsage:\n  qg-dxc connect <host> <port> --call <CALL> [--profile auto|dxspider|arcluster|cccluster|rbn]\n                 [--record <file>] [--retain-third-party-data] [--json]\n  qg-dxc tui --call <CALL> --node <host>:<port>[/<profile>] [--node …]\n             [--keep-human N] [--keep-skimmer N]\n  qg-dxc tui --node pota\n  qg-dxc tui --replay <file> [--replay …] [--keep-human N] [--keep-skimmer N]\n  qg-dxc tui [--sources <file>] [--keep-human N] [--keep-skimmer N]\n  qg-dxc replay <file> [--json] [--retain-third-party-data]\n\nReceive-only. Supply your own node and operator identity. No node directory or password support.\nRecording requires --retain-third-party-data. Replay never opens a socket.\nTUI combines live feeds or paced recordings in one filterable spot list for operators.\nTUI without --node or --replay lists the nodes in your own sources file to choose from\n(default: qso-graph/dxcluster-sources.json in your config folder; see docs/user/sources.md).\nTUI --keep-human N and --keep-skimmer N set independent spot caps (default 100 each; 10..=10000).\nTUI --theme color|daylight|phosphor|amber|mono selects a theme (overrides NO_COLOR).\nTUI --space-weather opens the NOAA SWPC view and starts fetching its products.\nTUI requires terminal input and output; press l to edit limits, t to cycle themes, ? for keys.\n";
fn main() -> std::process::ExitCode {
    let mut retain = false;
    match run(&mut retain) {
        Ok(code) => std::process::ExitCode::from(code),
        Err(e) => {
            eprintln!("qg-dxc: {}", terminal_text(&e, retain));
            std::process::ExitCode::FAILURE
        }
    }
}
// QGDXC-SEC-0003 QGDXC-SEC-0005: notes and diagnostics use the same
// privacy transform as events, including sensitive or ambiguous CC11 layouts.
fn terminal_text(text: &str, retain: bool) -> String {
    let mut event = Event::new(Profile::Auto, "unrecognized_line");
    event.raw_line = text.as_bytes().to_vec();
    event.set("text", text);
    privacy::apply(&mut event, retain);
    escape(event.get("text").unwrap_or_default())
}
fn rendered(event: &Event, json: bool, retain: bool) -> String {
    let mut event = event.clone();
    privacy::apply(&mut event, retain);
    if json {
        event.json()
    } else {
        let fields = event
            .fields
            .iter()
            .map(|(k, v)| format!("{k}={}", v.json()))
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "{} {} {}",
            event.server_type.name(),
            event.kind,
            escape(&fields)
        )
    }
}
fn render(event: &Event, json: bool, retain: bool) -> io::Result<()> {
    writeln!(io::stdout().lock(), "{}", rendered(event, json, retain))
}
fn run(retention: &mut bool) -> Result<u8, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{HELP}");
        return Ok(0);
    }
    if args == ["--version"] {
        println!("qg-dxc {}", env!("CARGO_PKG_VERSION"));
        return Ok(0);
    }
    if args[0] == "tui" {
        return tui::run(&args[1..]);
    }
    let json = args.iter().any(|a| a == "--json");
    if args[0] == "replay" {
        if args.len() < 2
            || args[1].starts_with('-')
            || args[2..]
                .iter()
                .any(|a| !matches!(a.as_str(), "--json" | "--retain-third-party-data"))
        {
            return Err("usage: qg-dxc replay <file> [--json] [--retain-third-party-data]".into());
        }
        *retention = args[2..].iter().any(|a| a == "--retain-third-party-data");
        let mut replay = ReplayStream::new(
            BufReader::new(File::open(&args[1]).map_err(|e| e.to_string())?),
            *retention,
        )?;
        while let Some((line, v, events)) = replay.next_record()? {
            if let Some(note) = v["note"].as_str() {
                eprintln!("note {line}: {}", terminal_text(note, *retention));
            }
            for event in events {
                render(&event, json, *retention).map_err(|e| e.to_string())?;
            }
        }
        for event in replay.finish()? {
            render(&event, json, *retention).map_err(|e| e.to_string())?;
        }
        return Ok(0);
    }
    if args[0] != "connect" || args.len() < 3 {
        return Err("use --help for connection and replay usage".into());
    }
    let port = args[2]
        .parse::<u16>()
        .map_err(|_| "port must be 1 to 65535")?;
    let mut call = None;
    let mut profile = Profile::Auto;
    let mut retain = false;
    let mut destination = None;
    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => {}
            "--retain-third-party-data" => retain = true,
            "--call" | "--profile" | "--record" => {
                let option = &args[i];
                i += 1;
                let value = args.get(i).ok_or("missing option value")?;
                match option.as_str() {
                    "--call" => call = Some(value.clone()),
                    "--profile" => profile = Profile::parse(value).ok_or("unsupported profile")?,
                    _ => destination = Some(value.clone()),
                }
            }
            _ => return Err("unknown connection option".into()),
        }
        i += 1;
    }
    let mut config = Config::new(
        args[1].clone(),
        port,
        call.ok_or("--call is required")?,
        profile,
    );
    *retention = retain;
    config.retain_third_party_data = retain;
    config.validate().map_err(str::to_owned)?;
    if destination.is_some() && !retain {
        return Err("--record requires --retain-third-party-data".into());
    }
    let recorder = destination
        .map(|path| {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|e| e.to_string())
        })
        .transpose()?
        .map(|file| Recorder::new(file, retain).map_err(|e| e.to_string()))
        .transpose()?;
    connect(config, recorder, json)
}
// QGDXC-SES-0009: each dial returns before the next address is attempted.
fn connect_addresses<A, T>(
    addresses: &[A],
    timeout: Duration,
    mut elapsed: impl FnMut() -> Duration,
    cancelled: impl Fn() -> bool,
    mut dial: impl FnMut(&A, Duration) -> io::Result<T>,
) -> io::Result<T> {
    let mut last = io::Error::other("endpoint resolved to no addresses");
    for (index, address) in addresses.iter().enumerate() {
        if cancelled() {
            return Err(io::Error::other("cancelled"));
        }
        let remaining = timeout.saturating_sub(elapsed());
        if remaining.is_zero() {
            break;
        }
        let left = u32::try_from(addresses.len() - index).unwrap_or(u32::MAX);
        let budget = (remaining / left)
            .max(Duration::from_secs(3))
            .min(remaining);
        let result = dial(address, budget);
        if cancelled() {
            return Err(io::Error::other("cancelled"));
        }
        match result {
            Ok(stream) => return Ok(stream),
            Err(error) => last = error,
        }
    }
    Err(last)
}

enum SendError {
    Transport,
    Recording(String),
}
struct Connector {
    rx: mpsc::Receiver<io::Result<TcpStream>>,
    cancel: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Drop for Connector {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
enum Delivery {
    Closed,
    Text(Output),
    Tui(mpsc::SyncSender<tui::sources::Envelope>),
}
impl Delivery {
    fn flush(&mut self) -> Result<(), String> {
        match self {
            Self::Text(o) => o.flush(),
            Self::Tui(_) | Self::Closed => Ok(()),
        }
    }
    fn backpressured(&self) -> bool {
        match self {
            Self::Text(o) => o.backpressured(),
            Self::Tui(_) | Self::Closed => false,
        }
    }
    fn enqueue(&mut self, event: Event, json: bool, retain: bool) -> Result<(), String> {
        match self {
            Self::Closed => Ok(()),
            Self::Text(o) => o.enqueue(rendered(&event, json, retain)),
            Self::Tui(tx) => tx
                .send(tui::sources::Envelope {
                    order: 0,
                    message: tui::sources::Message::Event(event),
                })
                .map_err(|_| "interface closed".to_owned()),
        }
    }
    fn finish(self, cancelled: impl Fn() -> bool) -> Result<(), String> {
        match self {
            Self::Text(o) => o.finish(cancelled),
            Self::Tui(_) | Self::Closed => Ok(()),
        }
    }
}
struct Live {
    session: Session,
    socket: Option<TcpStream>,
    recorder: Option<Recorder<File>>,
    clock: Instant,
    json: bool,
    output: Delivery,
    receiver: Option<Connector>,
    closing: Option<Connector>,
    deferred_connect: bool,
    stop: Arc<AtomicBool>,
}
impl Drop for Live {
    fn drop(&mut self) {
        self.output = Delivery::Closed;
        if let Ok(batch) = self.session.cancel(self.now()) {
            let _ = self.batch(batch);
        }
        if let Some(socket) = self.socket.take() {
            let _ = socket.shutdown(std::net::Shutdown::Both);
        }
        let _ = self.record_close("cancelled");
    }
}
impl Live {
    fn now(&self) -> u64 {
        self.clock
            .elapsed()
            .as_millis()
            .min(u128::from(qso_graph_dxcluster::values::MAX_INTEGER)) as u64
    }
    fn input(&mut self, bytes: &[u8]) -> Result<(), String> {
        let b = self
            .session
            .input_chunk(self.now(), self.session.attempt, bytes, None)
            .map_err(str::to_owned)?;
        self.batch(b)
    }
    fn drain_input(&mut self) -> Result<(), String> {
        let b = self.session.drain_input().map_err(str::to_owned)?;
        self.batch(b)
    }
    fn record_close(&mut self, reason: &str) -> Result<(), String> {
        let t = self.clock.elapsed().as_secs_f64();
        if let Some(r) = &mut self.recorder {
            r.close(t, reason)
                .map_err(|e| format!("recording failed: {e}"))?;
        }
        Ok(())
    }
    fn batch(&mut self, b: Batch) -> Result<(), String> {
        let cause = b
            .events
            .iter()
            .find(|e| {
                e.kind == "connection_state"
                    && matches!(e.get("state"), Some("blocked" | "disconnected"))
            })
            .and_then(|e| e.get("reason"))
            .unwrap_or("cancelled")
            .to_owned();
        let rendering = b.events.into_iter().try_for_each(|e| {
            self.output
                .enqueue(e, self.json, self.session.config.retain_third_party_data)
        });
        for a in b.actions {
            match a {
                Action::ReadinessToken { attempt } => {
                    let token = readiness_token(attempt, os_random)?;
                    let b = self
                        .session
                        .readiness_token(self.now(), attempt, token)
                        .map_err(str::to_owned)?;
                    self.batch(b)?;
                }
                Action::RetryJitter { maximum_ms } => {
                    let jitter = retry_jitter(maximum_ms, os_random)?;
                    let b = self
                        .session
                        .retry_jitter(self.now(), jitter)
                        .map_err(str::to_owned)?;
                    self.batch(b)?;
                }
                Action::Connect { .. } => {
                    if self.closing.is_some() {
                        self.deferred_connect = true;
                        continue;
                    }
                    if self.socket.is_some() || self.receiver.is_some() {
                        return Err("transport already active".into());
                    }
                    let (tx, rx) = mpsc::channel();
                    let host = self.session.config.host.clone();
                    let port = self.session.config.port;
                    let timeout = Duration::from_millis(self.session.config.connect_ms);
                    let stop = self.stop.clone();
                    let cancelled = Arc::new(AtomicBool::new(false));
                    let worker_cancel = cancelled.clone();
                    let worker = std::thread::spawn(move || {
                        let result = (|| {
                            let start = Instant::now();
                            let addresses: Vec<_> =
                                (host.as_str(), port).to_socket_addrs()?.collect();
                            connect_addresses(
                                &addresses,
                                timeout,
                                || start.elapsed(),
                                || {
                                    stop.load(Ordering::Relaxed)
                                        || worker_cancel.load(Ordering::Relaxed)
                                },
                                TcpStream::connect_timeout,
                            )
                        })();
                        let _ = tx.send(result);
                    });
                    self.receiver = Some(Connector {
                        rx,
                        cancel: cancelled,
                        worker: Some(worker),
                    });
                }
                Action::Close { .. } => {
                    if let Some(socket) = self.socket.take() {
                        let _ = socket.shutdown(std::net::Shutdown::Both);
                    }
                    self.deferred_connect = false;
                    if let Some(worker) = self.receiver.take() {
                        worker.cancel.store(true, Ordering::Relaxed);
                        self.closing = Some(worker);
                    }
                    self.record_close(&cause)?;
                }
                Action::Output { bytes, attempt } => {
                    if let Err(error) = self.send(&bytes) {
                        match error {
                            SendError::Recording(message) => return Err(message),
                            SendError::Transport => {
                                if let Some(socket) = self.socket.take() {
                                    let _ = socket.shutdown(std::net::Shutdown::Both);
                                }
                                self.record_close("transport_error")?;
                                let b = self
                                    .session
                                    .transport_end(
                                        self.now(),
                                        self.session.attempt,
                                        "transport_error",
                                    )
                                    .map_err(str::to_owned)?;
                                self.batch(b)?;
                            }
                        }
                    } else {
                        let admitted = self
                            .session
                            .output_admitted(self.now(), attempt)
                            .map_err(str::to_owned)?;
                        self.batch(admitted)?;
                    }
                }
                Action::ArmTimer { .. } | Action::CancelTimer { .. } => {}
            }
        }
        rendering
    }
    fn send(&mut self, bytes: &[u8]) -> Result<(), SendError> {
        let socket = self.socket.as_mut().ok_or(SendError::Transport)?;
        socket
            .set_nonblocking(false)
            .map_err(|_| SendError::Transport)?;
        socket
            .set_write_timeout(Some(Duration::from_millis(250)))
            .map_err(|_| SendError::Transport)?;
        let result = (|| {
            let mut rest = bytes;
            while !rest.is_empty() {
                if self.stop.load(Ordering::Relaxed) {
                    return Err(SendError::Transport);
                }
                let n = socket.write(rest).map_err(|_| SendError::Transport)?;
                if n == 0 {
                    return Err(SendError::Transport);
                }
                if let Some(recorder) = &mut self.recorder {
                    recorder
                        .wire(self.clock.elapsed().as_secs_f64(), "out", &rest[..n])
                        .map_err(|e| SendError::Recording(format!("recording failed: {e}")))?;
                }
                rest = &rest[n..];
            }
            Ok(())
        })();
        socket
            .set_nonblocking(true)
            .map_err(|_| SendError::Transport)?;
        result
    }
    fn timers(&mut self, now: u64, equal: bool) -> Result<(), String> {
        while let Some((at, t)) = self.session.next_timer() {
            let now = now.max(self.now());
            if at > now || (at == now && !equal && t != Timer::StableReady) {
                break;
            }
            let b = self.session.fire(t, now).map_err(str::to_owned)?;
            self.batch(b)?;
        }
        Ok(())
    }
}
fn os_random() -> Result<u64, String> {
    getrandom::u64().map_err(|e| format!("OS randomness failed: {e}"))
}
// QGDXC-DSP-0004: the attempt occupies 53 bits, leaving 11 fresh OS-random
// bits. Disjoint attempt ranges ensure uniqueness without retaining history.
fn readiness_token(
    attempt: u64,
    random: impl FnOnce() -> Result<u64, String>,
) -> Result<String, String> {
    if attempt == 0 || attempt > qso_graph_dxcluster::values::MAX_INTEGER {
        return Err("attempt limit".into());
    }
    let random = random()?;
    Ok(format!("QGDXC{:016X}", (attempt << 11) | (random & 0x7ff)))
}
fn retry_jitter(
    maximum_ms: u64,
    mut random: impl FnMut() -> Result<u64, String>,
) -> Result<u64, String> {
    if maximum_ms > 60000 {
        return Err("invalid jitter bound".into());
    }
    let range = maximum_ms + 1;
    let threshold = range.wrapping_neg() % range;
    loop {
        let random = random()?;
        if random >= threshold {
            return Ok(random % range);
        }
    }
}
fn connect(config: Config, recorder: Option<Recorder<File>>, json: bool) -> Result<u8, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let handler = stop.clone();
    ctrlc::set_handler(move || handler.store(true, Ordering::Relaxed))
        .map_err(|e| e.to_string())?;
    run_live(
        config,
        recorder,
        json,
        stop,
        Delivery::Text(Output::new(io::stdout())),
    )
}
fn run_live(
    config: Config,
    recorder: Option<Recorder<File>>,
    json: bool,
    stop: Arc<AtomicBool>,
    output: Delivery,
) -> Result<u8, String> {
    let session = Session::new(config).map_err(str::to_owned)?;
    let mut live = Live {
        session,
        socket: None,
        recorder,
        clock: Instant::now(),
        json,
        output,
        receiver: None,
        closing: None,
        deferred_connect: false,
        stop,
    };
    if let Some(recorder) = &mut live.recorder {
        recorder.note(0.0,&format!("qg-dxc configuration: {}",serde_json::json!({"call":live.session.config.call,"profile":if live.session.config.profile==Profile::Auto{"auto"}else{live.session.config.profile.name()}}))).map_err(|e|format!("recording failed: {e}"))?;
    }
    let b = live.session.start(0).map_err(str::to_owned)?;
    live.batch(b)?;
    let result: Result<(), String> = (|| {
        loop {
            let now = live.now();
            if live.stop.load(Ordering::Relaxed) {
                let b = live.session.cancel(live.now()).map_err(str::to_owned)?;
                live.batch(b)?;
                break;
            }
            live.output.flush()?;
            live.timers(now, false)?;
            let closed = live
                .closing
                .as_ref()
                .is_some_and(|c| !matches!(c.rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
            if closed {
                live.closing = None;
                if live.deferred_connect {
                    live.deferred_connect = false;
                    live.batch(Batch {
                        events: vec![],
                        actions: vec![Action::Connect {
                            attempt: live.session.attempt,
                        }],
                    })?;
                }
            }
            let connected = live.receiver.as_ref().and_then(|r| match r.rx.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err(io::Error::other("connection worker ended")))
                }
                Err(_) => None,
            });
            if let Some(result) = connected {
                live.receiver = None;
                match result {
                    Ok(socket) => {
                        socket.set_nonblocking(true).map_err(|e| e.to_string())?;
                        live.socket = Some(socket);
                        if let Some(r) = &mut live.recorder {
                            r.open(live.clock.elapsed().as_secs_f64())
                                .map_err(|e| format!("recording failed: {e}"))?;
                        }
                        let b = live
                            .session
                            .connected(live.now(), live.session.attempt)
                            .map_err(str::to_owned)?;
                        live.batch(b)?;
                    }
                    Err(_) => {
                        let b = live
                            .session
                            .transport_end(live.now(), live.session.attempt, "transport_error")
                            .map_err(str::to_owned)?;
                        live.batch(b)?;
                    }
                }
            }
            // QGDXC-IO-0005: drain the admitted chunk before reading again.
            // Rendering backpressure leaves its remainder inside the session.
            for _ in 0..64 {
                if live.output.backpressured() || live.stop.load(Ordering::Relaxed) {
                    break;
                }
                if live.session.has_pending_input() {
                    live.drain_input()?;
                    continue;
                }
                let Some(socket) = &mut live.socket else {
                    break;
                };
                let mut bytes = [0u8; 4096];
                match socket.read(&mut bytes) {
                    Ok(0) => {
                        live.socket = None;
                        live.record_close("eof")?;
                        let b = live
                            .session
                            .transport_end(live.now(), live.session.attempt, "eof")
                            .map_err(str::to_owned)?;
                        live.batch(b)?;
                        break;
                    }
                    Ok(n) => {
                        if let Some(r) = &mut live.recorder {
                            r.wire(live.clock.elapsed().as_secs_f64(), "in", &bytes[..n])
                                .map_err(|e| format!("recording failed: {e}"))?;
                        }
                        live.input(&bytes[..n])?;
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(_) => {
                        live.socket = None;
                        live.record_close("transport_error")?;
                        let b = live
                            .session
                            .transport_end(live.now(), live.session.attempt, "transport_error")
                            .map_err(str::to_owned)?;
                        live.batch(b)?;
                        break;
                    }
                }
            }
            live.timers(now, true)?;
            if matches!(live.session.state, State::Blocked | State::Disconnected) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    })();
    if result.is_err() {
        if let Ok(b) = live.session.cancel(live.now()) {
            let _ = live.batch(b);
        }
        let _ = live.record_close("cancelled");
    }
    // Finalize transport and recording before waiting for terminal output.
    if let Some(socket) = live.socket.take() {
        let _ = socket.shutdown(std::net::Shutdown::Both);
    }
    live.record_close("cancelled")?;
    drop(live.recorder.take());
    let stop = live.stop.clone();
    let drained = std::mem::replace(&mut live.output, Delivery::Closed)
        .finish(|| stop.load(Ordering::Relaxed));
    if stop.load(Ordering::Relaxed) {
        return Ok(130);
    }
    result?;
    drained?;
    Ok(0)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fresh_os_entropy_and_unique_tokens_without_history() {
        // QGDXC-DSP-0004 QGDXC-SES-0010
        let mut tokens = std::collections::HashSet::new();
        for attempt in (1..=4096).chain([qso_graph_dxcluster::values::MAX_INTEGER]) {
            let token = readiness_token(attempt, os_random).unwrap();
            assert!(qso_graph_dxcluster::values::token(&token));
            assert!(tokens.insert(token));
            let jitter = retry_jitter(1000, os_random).unwrap();
            assert!(jitter <= 1000);
        }
        let mut calls = 0;
        let mut repeated = std::collections::HashSet::new();
        for attempt in 1..=4096 {
            let token = readiness_token(attempt, || {
                calls += 1;
                Ok(7)
            })
            .unwrap();
            assert!(repeated.insert(token));
        }
        assert_eq!(calls, 4096);
        assert!(readiness_token(1, || Err("entropy unavailable".into())).is_err());
        assert!(retry_jitter(1000, || Err("entropy unavailable".into())).is_err());
        // Rejection sampling retries low values and includes both bounds.
        let mut samples = [0, 3, 10].into_iter();
        assert_eq!(
            retry_jitter(10, || Ok(samples.next().unwrap())).unwrap(),
            10
        );
        assert_eq!(retry_jitter(0, || Ok(0)).unwrap(), 0);
        assert!(readiness_token(0, || Ok(0)).is_err());
        assert!(retry_jitter(60001, || Ok(0)).is_err());
    }
    #[test]
    fn diagnostics_share_event_privacy_and_control_escaping() {
        // QGDXC-SEC-0003 QGDXC-SEC-0005: an adapter's error can contain
        // arbitrary text. The top-level error handler always uses this path.
        let message = "recording failed: third party 192.0.2.1 [2001:db8::1]\u{001b}\n\u{009b}";
        assert_eq!(
            terminal_text(message, false),
            "recording failed: third party <ip> [<ip>]\\u{001b}\\u{000a}\\u{009b}"
        );
        assert_eq!(terminal_text(message, true), escape(message));
        let mut slots = vec![""; 25];
        slots[0] = "CC11";
        slots[22] = "+12.3456/-45.6789";
        slots[23] = "192.0.2.1";
        let cc11 = slots.join("^");
        let expected = format!("CC11{}", "^".repeat(24));
        assert_eq!(terminal_text(&cc11, false), expected);
        assert_eq!(terminal_text(&format!("error: {cc11}"), false), "");
        assert_eq!(terminal_text(&cc11, true), cc11);
        let mut event = Event::new(Profile::Auto, "unrecognized_line");
        event.raw_line = message.as_bytes().to_vec();
        event.set("text", message);
        for json in [false, true] {
            let output = rendered(&event, json, false);
            assert!(!output.contains("192.0.2.1"));
            assert!(!output.contains("2001:db8::1"));
            assert!(!output.chars().any(char::is_control));
            if json {
                let value: serde_json::Value = serde_json::from_str(&output).unwrap();
                let raw = model::unbase64(value["raw_line"].as_str().unwrap()).unwrap();
                assert_eq!(
                    raw,
                    b"recording failed: third party <ip> [<ip>]\x1b\n\xc2\x9b"
                );
            }
        }
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn library_and_live_drain_one_mib_under_memory_ceiling() {
        // QGDXC-IO-0005 QGDXC-SEC-0001
        if !crate::test_support::memory_child(&[
            "tests",
            "library_and_live_drain_one_mib_under_memory_ceiling",
        ]) {
            return;
        }
        let bytes = vec![b'\n'; INPUT_BYTES];
        let mut session = Session::new(Config::new(
            "example.invalid".into(),
            7300,
            "Q1ABC".into(),
            Profile::Rbn,
        ))
        .unwrap();
        session.start(0).unwrap();
        session.connected(0, session.attempt).unwrap();
        let mut b = session
            .input_chunk(0, session.attempt, &bytes, None)
            .unwrap();
        let mut count = 0;
        loop {
            assert!(b.events.len() <= INPUT_EVENTS);
            count += b.events.len();
            drop(b);
            if !session.has_pending_input() {
                break;
            }
            b = session.drain_input().unwrap();
        }
        assert_eq!(count, INPUT_BYTES);
        struct Count(Arc<std::sync::atomic::AtomicUsize>);
        impl Write for Count {
            fn write(&mut self, b: &[u8]) -> io::Result<usize> {
                self.0
                    .fetch_add(b.iter().filter(|b| **b == b'\n').count(), Ordering::Relaxed);
                Ok(b.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut live = Live {
            session,
            socket: None,
            recorder: None,
            clock: Instant::now(),
            json: false,
            output: Delivery::Text(Output::new(Count(count.clone()))),
            receiver: None,
            closing: None,
            deferred_connect: false,
            stop: Arc::new(AtomicBool::new(false)),
        };
        live.input(&bytes).unwrap();
        while live.session.has_pending_input() {
            live.output.flush().unwrap();
            if live.output.backpressured() {
                std::thread::yield_now();
            } else {
                live.drain_input().unwrap();
            }
        }
        std::mem::replace(&mut live.output, Delivery::Closed)
            .finish(|| false)
            .unwrap();
        assert_eq!(count.load(Ordering::Relaxed), INPUT_BYTES);
    }
    #[test]
    fn live_deadline_closes_transport_while_rendering_is_stalled() {
        // QGDXC-IO-0002 QGDXC-IO-0005 QGDXC-SES-0007
        struct Paused(mpsc::Receiver<()>);
        impl Write for Paused {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.0.recv().map_err(io::Error::other)?;
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let listener = std::net::TcpListener::bind(("localhost", 0)).unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut peer, _) = listener.accept().unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        let mut config = Config::new("Q".into(), 7300, "Q1ABC".into(), Profile::ArCluster);
        config.login_ms = 5000;
        let mut session = Session::new(config).unwrap();
        session.start(0).unwrap();
        session.connected(0, session.attempt).unwrap();
        let path = format!("../../target/stalled-timer-{}.ndjson", std::process::id());
        let mut recorder = Recorder::new(File::create(&path).unwrap(), true).unwrap();
        recorder.open(0.0).unwrap();
        let (resume, rx) = mpsc::channel();
        let mut output = Output::new(Paused(rx));
        while !output.backpressured() {
            output.enqueue("Q".into()).unwrap();
        }
        let mut live = Live {
            session,
            socket: Some(client),
            recorder: Some(recorder),
            clock: Instant::now() - Duration::from_millis(5000),
            json: false,
            output: Delivery::Text(output),
            receiver: None,
            closing: None,
            deferred_connect: false,
            stop: Arc::new(AtomicBool::new(false)),
        };
        let started = Instant::now();
        live.timers(5000, true).unwrap();
        assert!(started.elapsed() < Duration::from_millis(500));
        assert_eq!(live.session.state, State::Backoff);
        assert!(live.socket.is_none());
        assert_eq!(peer.read(&mut [0; 1]).unwrap(), 0);
        let text = std::fs::read_to_string(&path).unwrap();
        let last: serde_json::Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
        assert_eq!(last["conn"], "close");
        assert_eq!(last["reason"], "timeout");
        // Disconnecting the test writer releases every subsequent write.
        drop(resume);
        assert!(
            std::mem::replace(&mut live.output, Delivery::Closed)
                .finish(|| false)
                .is_err()
        );
        drop(live.recorder.take());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn sequential_address_fallback_survives_a_blackhole() {
        // QGDXC-SES-0007 QGDXC-SES-0009
        let elapsed = std::cell::Cell::new(Duration::ZERO);
        let mut calls = vec![];
        let result = connect_addresses(
            &[0, 1],
            Duration::from_secs(30),
            || elapsed.get(),
            || false,
            |address, budget| {
                calls.push((*address, budget));
                if *address == 0 {
                    elapsed.set(elapsed.get() + budget);
                    Err(io::Error::new(io::ErrorKind::TimedOut, "no answer"))
                } else {
                    Ok(*address)
                }
            },
        )
        .unwrap();
        assert_eq!(result, 1);
        assert_eq!(
            calls,
            [(0, Duration::from_secs(15)), (1, Duration::from_secs(15))]
        );
    }
    #[test]
    fn address_budgets_respect_minimum_and_overall_deadline() {
        // QGDXC-SES-0007 QGDXC-SES-0009
        let elapsed = std::cell::Cell::new(Duration::ZERO);
        let mut budgets = vec![];
        let result: io::Result<()> = connect_addresses(
            &[0, 1, 2, 3],
            Duration::from_secs(5),
            || elapsed.get(),
            || false,
            |_, budget| {
                budgets.push(budget);
                elapsed.set(elapsed.get() + budget);
                Err(io::Error::new(io::ErrorKind::TimedOut, "no answer"))
            },
        );
        assert!(result.is_err());
        assert_eq!(budgets, [Duration::from_secs(3), Duration::from_secs(2)]);
    }
    #[test]
    fn address_budget_redistributes_unused_time_and_honors_cancellation() {
        // QGDXC-SES-0009
        let elapsed = std::cell::Cell::new(Duration::from_secs(3));
        let cancelled = std::cell::Cell::new(false);
        let mut calls = vec![];
        let result = connect_addresses(
            &[0, 1, 2],
            Duration::from_secs(30),
            || elapsed.get(),
            || cancelled.get(),
            |address, budget| {
                calls.push((*address, budget));
                if *address == 0 {
                    elapsed.set(Duration::from_secs(4));
                    Err(io::Error::other("refused"))
                } else {
                    cancelled.set(true);
                    Ok(*address)
                }
            },
        );
        assert!(result.is_err());
        assert_eq!(
            calls,
            [(0, Duration::from_secs(9)), (1, Duration::from_secs(13))]
        );
    }
    #[test]
    fn help_leads_with_purpose() {
        // QGDXC-IO-0002
        assert!(HELP.starts_with("qg-dxc reads live DX spots for radio operators"));
    }
}

#[cfg(all(test, target_os = "linux"))]
mod test_support;
