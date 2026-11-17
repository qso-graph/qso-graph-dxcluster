// SPDX-License-Identifier: GPL-3.0-or-later
use qso_graph_dxcluster::{
    model::{base64, unbase64},
    *,
};
use serde_json::{Value, json};
use std::io::{self, BufRead, Read, Write};

pub struct Recorder<W: Write> {
    writer: W,
    open: bool,
    last: f64,
}
impl<W: Write> Recorder<W> {
    pub fn new(writer: W, retain: bool) -> io::Result<Self> {
        if !retain {
            return Err(io::Error::other(
                "recording requires --retain-third-party-data",
            ));
        }
        Ok(Self {
            writer,
            open: false,
            last: 0.0,
        })
    }
    fn write(&mut self, t: f64, value: Value) -> io::Result<()> {
        if !t.is_finite() || t < self.last {
            return Err(io::Error::other("invalid recording time"));
        }
        self.last = t;
        serde_json::to_writer(&mut self.writer, &value)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()
    }
    pub fn note(&mut self, t: f64, note: &str) -> io::Result<()> {
        self.write(t, json!({"t":t,"note":note}))
    }
    pub fn open(&mut self, t: f64) -> io::Result<()> {
        if self.open {
            return Err(io::Error::other("overlapping recording connections"));
        }
        self.write(t, json!({"t":t,"conn":"open"}))?;
        self.open = true;
        Ok(())
    }
    pub fn close(&mut self, t: f64, reason: &str) -> io::Result<()> {
        if !self.open {
            return Ok(());
        }
        self.write(t, json!({"t":t,"conn":"close","reason":reason}))?;
        self.open = false;
        Ok(())
    }
    pub fn wire(&mut self, t: f64, dir: &str, b: &[u8]) -> io::Result<()> {
        if !self.open || !matches!(dir, "in" | "out") {
            return Err(io::Error::other("wire record outside connection"));
        }
        self.write(t, json!({"t":t,"dir":dir,"b64":base64(b)}))
    }
}
const REASONS: &[&str] = &[
    "started",
    "connected",
    "login_sent",
    "identified",
    "setup_complete",
    "eof",
    "transport_error",
    "cancelled",
    "timeout",
    "profile_unknown",
    "profile_mismatch",
    "password_unsupported",
    "setup_failed",
    "refused",
    "displaced",
    "silence_timeout",
    "resource_limit",
    "unsupported_transport",
    "retry_scheduled",
    "retry_exhausted",
];
/// Validates physical records independently of protocol framing. Each record has
/// a finite bound; a recording can contain arbitrarily many bounded records.
pub struct Records<R: BufRead> {
    reader: R,
    last: f64,
    open: bool,
    pub line: usize,
    done: bool,
}
impl<R: BufRead> Records<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            last: 0.0,
            open: false,
            line: 0,
            done: false,
        }
    }
    pub fn next(&mut self) -> Result<Option<Value>, String> {
        if self.done {
            return Ok(None);
        }
        let mut buf = Vec::new();
        let n = std::io::Read::by_ref(&mut self.reader)
            .take(1_048_577)
            .read_until(b'\n', &mut buf)
            .map_err(|e| e.to_string())?;
        if n == 0 {
            self.done = true;
            if self.open {
                return Err("recording ends without a close boundary".into());
            }
            return Ok(None);
        }
        self.line += 1;
        if n > 1_048_576 {
            return Err(format!("record {} exceeds 1 MiB", self.line));
        }
        let v: Value = serde_json::from_slice(&buf)
            .map_err(|_| format!("invalid JSON at record {}", self.line))?;
        let obj = v.as_object().ok_or("record must be an object")?;
        let t = v["t"]
            .as_f64()
            .filter(|t| t.is_finite() && *t >= self.last)
            .ok_or("record time must be finite, nonnegative and nondecreasing")?;
        self.last = t;
        let keys = if v.get("note").is_some() {
            if !v["note"].is_string() {
                return Err("note must be text".into());
            }
            vec!["t", "note"]
        } else if let Some(conn) = v["conn"].as_str() {
            match conn {
                "open" => {
                    if self.open {
                        return Err("overlapping open boundaries".into());
                    }
                    self.open = true;
                    vec!["t", "conn"]
                }
                "close" => {
                    if !self.open || !v["reason"].as_str().is_some_and(|r| REASONS.contains(&r)) {
                        return Err("invalid close boundary".into());
                    }
                    self.open = false;
                    vec!["t", "conn", "reason"]
                }
                _ => return Err("invalid boundary".into()),
            }
        } else {
            if !self.open || !matches!(v["dir"].as_str(), Some("in" | "out")) {
                return Err("wire record outside connection".into());
            }
            unbase64(v["b64"].as_str().ok_or("missing wire bytes")?).map_err(str::to_owned)?;
            vec!["t", "dir", "b64"]
        };
        if keys.len() != obj.len() || keys.iter().any(|k| !obj.contains_key(*k)) {
            return Err("invalid record members".into());
        }
        Ok(Some(v))
    }
}
/// Replay drives the same Session as live input. Generated actions are consumed
/// locally; an outbound record is an observation and is never transmitted.
pub struct Replay {
    engine: Session,
    last: u64,
}
impl Replay {
    pub fn new(call: String, profile: Profile, tokens: Vec<String>) -> Result<Self, String> {
        let config = Config::new("example.invalid".into(), 7300, call, profile);
        Ok(Self {
            engine: Session::new(config, tokens, vec![0; 1024]).map_err(str::to_owned)?,
            last: 0,
        })
    }
    fn advance(&mut self, at: u64, equal: bool, events: &mut Vec<Event>) -> Result<(), String> {
        while let Some((deadline, timer)) = self.engine.next_timer() {
            if deadline > at || deadline == at && !equal && timer != Timer::StableReady {
                break;
            }
            events.extend(
                self.engine
                    .fire(timer, deadline)
                    .map_err(str::to_owned)?
                    .events,
            );
        }
        Ok(())
    }
    pub fn finish(&mut self) -> Result<Vec<Event>, String> {
        let mut events = vec![];
        self.advance(self.last, true, &mut events)?;
        Ok(events)
    }
    pub fn record(&mut self, v: &Value) -> Result<Vec<Event>, String> {
        let at = (v["t"].as_f64().ok_or("record time")? * 1000.0).floor();
        if at > qso_graph_dxcluster::values::MAX_INTEGER as f64 {
            return Err("elapsed time exceeds exact millisecond range".into());
        }
        let at = at as u64;
        let mut events = vec![];
        self.advance(at, false, &mut events)?;
        self.last = at;
        if v["conn"] == "open" {
            if self.engine.state != State::Connecting {
                let b = if self.engine.attempt == 0 {
                    self.engine.start(at)
                } else {
                    self.engine.restart(at)
                }
                .map_err(str::to_owned)?;
                events.extend(b.events);
            }
            events.extend(
                self.engine
                    .connected(at, self.engine.attempt)
                    .map_err(str::to_owned)?
                    .events,
            );
        } else if v["conn"] == "close" {
            let reason = REASONS
                .iter()
                .copied()
                .find(|r| Some(*r) == v["reason"].as_str())
                .ok_or("close reason")?;
            events.extend(
                self.engine
                    .transport_end(at, self.engine.attempt, reason)
                    .map_err(str::to_owned)?
                    .events,
            );
        } else if v["dir"] == "in" {
            for byte in unbase64(v["b64"].as_str().ok_or("wire bytes")?).map_err(str::to_owned)? {
                events.extend(
                    self.engine
                        .input(at, self.engine.attempt, byte, None)
                        .map_err(str::to_owned)?
                        .events,
                );
            }
        }
        Ok(events)
    }
}
/// Inspect local configuration observations before replay. This never turns an
/// outbound record or observer note into received identity evidence.
pub fn replay_config<R: BufRead>(reader: R) -> Result<(String, Profile, Vec<String>), String> {
    let mut records = Records::new(reader);
    let mut call = None;
    let mut profile = Profile::Auto;
    let mut tokens = Vec::<Option<String>>::new();
    let mut first_out = false;
    let mut outbound = Vec::<u8>::new();
    while let Some(v) = records.next()? {
        if v["conn"] == "open" {
            if tokens.len() == 1024 {
                return Err("recording exceeds 1024 connections".into());
            }
            tokens.push(None);
            first_out = true;
            outbound.clear();
        }
        if let Some(note) = v["note"]
            .as_str()
            .and_then(|n| n.strip_prefix("qg-dxc configuration: "))
        {
            let c: Value =
                serde_json::from_str(note).map_err(|_| "invalid recorded configuration")?;
            let identity = c["call"]
                .as_str()
                .filter(|s| qso_graph_dxcluster::values::login(s))
                .ok_or("recorded identity")?;
            call = Some(identity.to_owned());
            profile = Profile::parse(c["profile"].as_str().ok_or("recorded profile")?)
                .ok_or("recorded profile")?;
        }
        if v["dir"] == "out" {
            let b = unbase64(v["b64"].as_str().ok_or("outbound observation")?)
                .map_err(str::to_owned)?;
            for byte in b {
                if byte != b'\n' {
                    if outbound.len() == 256 {
                        return Err("outbound observation exceeds protocol line bound".into());
                    }
                    outbound.push(byte);
                    continue;
                }
                if let Ok(text) = std::str::from_utf8(&outbound) {
                    let text = text.trim_end_matches('\r');
                    if first_out {
                        if qso_graph_dxcluster::values::login(text) && call.is_none() {
                            call = Some(text.into());
                        }
                        first_out = false;
                    }
                    if let Some(token) = text
                        .strip_prefix("echo ")
                        .filter(|s| qso_graph_dxcluster::values::token(s))
                        && let Some(slot) = tokens.last_mut()
                    {
                        *slot = Some(token.into());
                    }
                }
                outbound.clear();
            }
        }
    }
    let mut reserved = std::collections::BTreeSet::new();
    for token in tokens.iter().flatten() {
        if !reserved.insert(token.clone()) {
            return Err("repeated readiness token in recording".into());
        }
    }
    let mut next = 0u64;
    let mut resolved = vec![];
    for token in tokens {
        resolved.push(if let Some(token) = token {
            token
        } else {
            loop {
                let token = format!("QGDXC{:016X}", u64::MAX - next);
                next += 1;
                if reserved.insert(token.clone()) {
                    break token;
                }
            }
        });
    }
    Ok((call.unwrap_or_else(|| "Q0NONE".into()), profile, resolved))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recording_boundaries_and_replay() {
        // QGDXC-REC-0001 QGDXC-REC-0002 QGDXC-REC-0003 QGDXC-REC-0004 QGDXC-REC-0005
        assert!(Recorder::new(Vec::new(), false).is_err());
        let mut r = Recorder::new(Vec::new(), true).unwrap();
        for t in [0.0, 10.0] {
            r.open(t).unwrap();
            r.wire(t, "in", b"call:").unwrap();
            r.wire(t, "out", b"Q1").unwrap();
            r.wire(t, "out", b"ABC\r").unwrap();
            r.wire(t, "out", b"\n").unwrap();
            r.wire(t, "in", b"Q1ABC de RELAY 1-Nov-2026 18:32Z >\r")
                .unwrap();
            r.close(t + 1.0, "eof").unwrap();
        }
        let (identity, profile, tokens) = replay_config(std::io::Cursor::new(&r.writer)).unwrap();
        assert_eq!(identity, "Q1ABC");
        assert_eq!(profile, Profile::Auto);
        assert_eq!(tokens.len(), 2);
        let mut rows = Records::new(std::io::Cursor::new(r.writer));
        let mut replay = Replay::new(
            "Q1ABC".into(),
            Profile::Auto,
            vec![
                "QGDXC0000000000000000".into(),
                "QGDXC0000000000000001".into(),
            ],
        )
        .unwrap();
        let mut events = vec![];
        while let Some(v) = rows.next().unwrap() {
            events.extend(replay.record(&v).unwrap());
        }
        assert_eq!(
            events
                .iter()
                .filter(|e| e.get("status") == Some("ready_prompt"))
                .count(),
            2
        );
        for invalid in [
            "{\"t\":0,\"dir\":\"in\",\"b64\":\"\"}\n",
            "{\"t\":0,\"conn\":\"open\"}\n{\"t\":0,\"conn\":\"open\"}\n",
            "{\"t\":0,\"note\":\"x\",\"conn\":\"open\"}\n",
        ] {
            let mut rows = Records::new(std::io::Cursor::new(invalid));
            let mut failed = false;
            for _ in 0..4 {
                match rows.next() {
                    Err(_) => {
                        failed = true;
                        break;
                    }
                    Ok(None) => break,
                    _ => {}
                }
            }
            assert!(failed);
        }
    }
    #[test]
    fn recording_failure_is_visible() {
        struct Fail;
        impl Write for Fail {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("full"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut r = Recorder::new(Fail, true).unwrap();
        assert!(r.open(0.0).is_err());
    }
}
