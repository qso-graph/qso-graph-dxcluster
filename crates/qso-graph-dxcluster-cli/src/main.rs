// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
use qso_graph_dxcluster::{model::escape, *};
use qso_graph_dxcluster_cli::recording;
use recording::{Recorder, Records, Replay};
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
const HELP: &str = "qg-dxc reads live DX spots for radio operators and application developers.\n\nUsage:\n  qg-dxc connect <host> <port> --call <CALL> [--profile auto|dxspider|arcluster|cccluster|rbn]\n                 [--record <file>] [--retain-third-party-data] [--json]\n  qg-dxc replay <file> [--json]\n\nReceive-only. Supply your own node and operator identity. No node directory or password support.\nRecording requires --retain-third-party-data. Replay never opens a socket.\n";
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("qg-dxc: {}", escape(&e));
            std::process::ExitCode::FAILURE
        }
    }
}
fn render(event: &Event, json: bool) -> io::Result<()> {
    let text = if json {
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
    };
    writeln!(io::stdout().lock(), "{text}")
}
fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{HELP}");
        return Ok(());
    }
    if args == ["--version"] {
        println!("qg-dxc 0.1.0");
        return Ok(());
    }
    let json = args.iter().any(|a| a == "--json");
    if args[0] == "replay" {
        if args.len() < 2 || args[1].starts_with('-') || args[2..].iter().any(|a| a != "--json") {
            return Err("usage: qg-dxc replay <file> [--json]".into());
        }
        let mut records = Records::new(BufReader::new(
            File::open(&args[1]).map_err(|e| e.to_string())?,
        ));
        let (identity, profile, tokens) = recording::replay_config(BufReader::new(
            File::open(&args[1]).map_err(|e| e.to_string())?,
        ))?;
        let mut replay = Replay::new(identity, profile, tokens)?;
        while let Some(v) = records.next_record()? {
            if let Some(note) = v["note"].as_str() {
                eprintln!("note {}: {}", records.line, escape(note));
            }
            for event in replay.record(&v)? {
                render(&event, json).map_err(|e| e.to_string())?;
            }
        }
        for event in replay.finish()? {
            render(&event, json).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    if args[0] != "connect" || args.len() < 3 {
        return Err("use --help for connection and replay usage".into());
    }
    let port = args[2].parse::<u16>().map_err(|_| "port must be 1–65535")?;
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
enum SendError {
    Transport,
    Recording(String),
}
struct Connector {
    rx: mpsc::Receiver<io::Result<TcpStream>>,
    cancel: Arc<AtomicBool>,
}
struct Live {
    session: Session,
    socket: Option<TcpStream>,
    recorder: Option<Recorder<File>>,
    clock: Instant,
    json: bool,
    receiver: Option<Connector>,
    closing: Option<Connector>,
    deferred_connect: bool,
    stop: Arc<AtomicBool>,
}
impl Live {
    fn now(&self) -> u64 {
        self.clock
            .elapsed()
            .as_millis()
            .min(u128::from(qso_graph_dxcluster::values::MAX_INTEGER)) as u64
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
        for e in b.events {
            render(&e, self.json).map_err(|e| e.to_string())?;
        }
        for a in b.actions {
            match a {
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
                    std::thread::spawn(move || {
                        let result = (|| {
                            let start = Instant::now();
                            let addresses = (host.as_str(), port).to_socket_addrs()?;
                            let mut last = io::Error::other("endpoint resolved to no addresses");
                            for address in addresses {
                                if stop.load(Ordering::Relaxed)
                                    || worker_cancel.load(Ordering::Relaxed)
                                {
                                    return Err(io::Error::other("cancelled"));
                                }
                                let remaining = timeout.saturating_sub(start.elapsed());
                                if remaining.is_zero() {
                                    break;
                                }
                                match TcpStream::connect_timeout(&address, remaining) {
                                    Ok(stream) => {
                                        if worker_cancel.load(Ordering::Relaxed) {
                                            drop(stream);
                                            return Err(io::Error::other("cancelled"));
                                        }
                                        return Ok(stream);
                                    }
                                    Err(e) => last = e,
                                }
                            }
                            Err(last)
                        })();
                        let _ = tx.send(result);
                    });
                    self.receiver = Some(Connector {
                        rx,
                        cancel: cancelled,
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
                Action::Output { bytes, .. } => {
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
                                        self.session.current_time(),
                                        self.session.attempt,
                                        "transport_error",
                                    )
                                    .map_err(str::to_owned)?;
                                self.batch(b)?;
                            }
                        }
                    }
                }
                Action::ArmTimer { .. } | Action::CancelTimer { .. } => {}
            }
        }
        Ok(())
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
            if at > now || (at == now && !equal && t != Timer::StableReady) {
                break;
            }
            let b = self.session.fire(t, at).map_err(str::to_owned)?;
            self.batch(b)?;
        }
        Ok(())
    }
}
fn connect(config: Config, recorder: Option<Recorder<File>>, json: bool) -> Result<(), String> {
    let stop = Arc::new(AtomicBool::new(false));
    let handler = stop.clone();
    ctrlc::set_handler(move || handler.store(true, Ordering::Relaxed))
        .map_err(|e| e.to_string())?;
    // The clock supplies caller tokens, never network content. Attempt suffixes
    // make tokens unique within this process, without requiring random I/O.
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos() as u64;
    let tokens = (0..1024u64)
        .map(|i| format!("QGDXC{:016X}", seed.wrapping_add(i)))
        .collect();
    let session = Session::new(config, tokens, vec![0; 1024]).map_err(str::to_owned)?;
    let mut live = Live {
        session,
        socket: None,
        recorder,
        clock: Instant::now(),
        json,
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
    let result = (|| {
        loop {
            let now = live.now();
            live.timers(now, false)?;
            if live.stop.load(Ordering::Relaxed) {
                let b = live.session.cancel(now).map_err(str::to_owned)?;
                live.batch(b)?;
                break;
            }
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
                            .connected(now, live.session.attempt)
                            .map_err(str::to_owned)?;
                        live.batch(b)?;
                    }
                    Err(_) => {
                        let b = live
                            .session
                            .transport_end(now, live.session.attempt, "transport_error")
                            .map_err(str::to_owned)?;
                        live.batch(b)?;
                    }
                }
            }
            if let Some(socket) = &mut live.socket {
                let mut bytes = [0u8; 4096];
                match socket.read(&mut bytes) {
                    Ok(0) => {
                        live.socket = None;
                        live.record_close("eof")?;
                        let b = live
                            .session
                            .transport_end(now, live.session.attempt, "eof")
                            .map_err(str::to_owned)?;
                        live.batch(b)?;
                    }
                    Ok(n) => {
                        if let Some(r) = &mut live.recorder {
                            r.wire(live.clock.elapsed().as_secs_f64(), "in", &bytes[..n])
                                .map_err(|e| format!("recording failed: {e}"))?;
                        }
                        for byte in &bytes[..n] {
                            let b = live
                                .session
                                .input(now, live.session.attempt, *byte, None)
                                .map_err(str::to_owned)?;
                            live.batch(b)?;
                        }
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                    Err(_) => {
                        live.socket = None;
                        live.record_close("transport_error")?;
                        let b = live
                            .session
                            .transport_end(now, live.session.attempt, "transport_error")
                            .map_err(str::to_owned)?;
                        live.batch(b)?;
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
        let now = live.now();
        if let Ok(b) = live.session.cancel(now) {
            let _ = live.batch(b);
        }
        let _ = live.record_close("cancelled");
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn help_leads_with_purpose() {
        // QGDXC-IO-0002
        assert!(HELP.starts_with("qg-dxc reads live DX spots for radio operators"));
    }
}
