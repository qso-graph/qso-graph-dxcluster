// SPDX-License-Identifier: GPL-3.0-or-later
use qso_graph_dxcluster::{
    model::{base64, unbase64},
    *,
};
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

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
    pub fn into_inner(self) -> W {
        self.writer
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
    bytes: usize,
}
impl<R: BufRead> Records<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            last: 0.0,
            open: false,
            line: 0,
            done: false,
            bytes: 0,
        }
    }
    pub fn next_record(&mut self) -> Result<Option<Value>, String> {
        self.next_record_bounded(crate::bounded::RECORD_BYTES)
    }
    fn next_record_bounded(&mut self, limit: usize) -> Result<Option<Value>, String> {
        if self.done {
            return Ok(None);
        }
        let Some(buf) = crate::bounded::line(&mut self.reader, limit)? else {
            self.done = true;
            if self.open {
                return Err("recording ends without a close boundary".into());
            }
            return Ok(None);
        };
        self.line += 1;
        self.bytes = buf.len();
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
struct Replay {
    engine: Session,
    last: u64,
    open: bool,
    outbound: Vec<u8>,
}
impl Replay {
    #[cfg(test)]
    fn new(call: String, profile: Profile) -> Result<Self, String> {
        Self::new_with_retention(call, profile, false)
    }
    /// Apply the caller's explicit QGDXC-SEC-0005 retention choice to replay.
    fn new_with_retention(call: String, profile: Profile, retain: bool) -> Result<Self, String> {
        let mut config = Config::new("example.invalid".into(), 7300, call, profile);
        config.retain_third_party_data = retain;
        Ok(Self {
            engine: Session::new(config)
                .map_err(str::to_owned)?
                .with_recorded_transport(),
            last: 0,
            open: false,
            outbound: vec![],
        })
    }
    fn supply(
        &mut self,
        b: qso_graph_dxcluster::Batch,
        token: &mut impl FnMut() -> Result<String, String>,
    ) -> Result<Vec<Event>, String> {
        let mut events = b.events;
        for action in b.actions {
            let b = match action {
                qso_graph_dxcluster::Action::ReadinessToken { attempt } => {
                    let token = token()?;
                    self.engine.readiness_token(self.last, attempt, token)
                }
                qso_graph_dxcluster::Action::RetryJitter { .. } => {
                    self.engine.retry_jitter(self.last, 0)
                }
                _ => continue,
            }
            .map_err(str::to_owned)?;
            events.extend(self.supply(b, token)?);
        }
        Ok(events)
    }
    fn advance(&mut self, at: u64, events: &mut Vec<Event>) -> Result<(), String> {
        if !self.open {
            return Ok(());
        }
        // QGDXC-REC-0004: only this nonterminating timer is synthetic. Output
        // admission and termination follow records, even after delayed I/O.
        if let Some(deadline) = self.engine.deadline(Timer::StableReady)
            && deadline <= at
        {
            events.extend(
                self.engine
                    .fire(Timer::StableReady, deadline)
                    .map_err(str::to_owned)?
                    .events,
            );
        }
        Ok(())
    }
    pub fn finish(&mut self) -> Result<Vec<Event>, String> {
        let mut events = vec![];
        self.advance(self.last, &mut events)?;
        Ok(events)
    }
    #[cfg(test)]
    fn record(&mut self, v: &Value) -> Result<Vec<Event>, String> {
        self.record_with_token(v, &mut || Err("missing recorded readiness token".into()))
    }
    fn record_with_token(
        &mut self,
        v: &Value,
        token: &mut impl FnMut() -> Result<String, String>,
    ) -> Result<Vec<Event>, String> {
        let at = (v["t"].as_f64().ok_or("record time")? * 1000.0).floor();
        if at > qso_graph_dxcluster::values::MAX_INTEGER as f64 {
            return Err("elapsed time exceeds exact millisecond range".into());
        }
        let at = at as u64;
        let mut events = vec![];
        self.advance(at, &mut events)?;
        self.last = at;
        if v["conn"] == "open" {
            self.open = true;
            self.outbound.clear();
            if self.engine.state != State::Connecting {
                let b = if self.engine.state == State::Backoff {
                    self.engine.recorded_retry(at)
                } else {
                    self.engine.start(at)
                }
                .map_err(str::to_owned)?;
                events.extend(self.supply(b, token)?);
            }
            events.extend(
                self.engine
                    .connected(at, self.engine.attempt)
                    .map_err(str::to_owned)?
                    .events,
            );
        } else if v["conn"] == "close" {
            self.open = false;
            self.outbound.clear();
            let reason = REASONS
                .iter()
                .copied()
                .find(|r| Some(*r) == v["reason"].as_str())
                .ok_or("close reason")?;
            let b = self
                .engine
                .transport_end(at, self.engine.attempt, reason)
                .map_err(str::to_owned)?;
            events.extend(self.supply(b, token)?);
        } else if v["dir"] == "out" {
            for byte in unbase64(v["b64"].as_str().ok_or("wire bytes")?).map_err(str::to_owned)? {
                if self.outbound.len() == 256 {
                    return Err("outbound observation exceeds protocol line bound".into());
                }
                self.outbound.push(byte);
                if byte == b'\n' {
                    events.extend(
                        self.engine
                            .recorded_output(at, self.engine.attempt, &self.outbound)
                            .map_err(str::to_owned)?
                            .events,
                    );
                    self.outbound.clear();
                }
            }
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
/// QGDXC-REC-0004 and QGDXC-SEC-0001: bounded, single-pass replay.
/// The lookahead budget includes encoded record bytes and a per-record charge.
const LOOKAHEAD_BYTES: usize = crate::bounded::RECORD_BYTES;
struct Lookahead<R: BufRead> {
    records: Records<R>,
    pending: std::collections::VecDeque<(usize, Value)>,
    bytes: usize,
}
impl<R: BufRead> Lookahead<R> {
    fn read_ahead(&mut self) -> Result<Option<Value>, String> {
        // Restrict the physical read too, so a large next record cannot overshoot
        // the remaining budget. Restore the normal per-record cap afterward.
        let remaining = LOOKAHEAD_BYTES.saturating_sub(self.bytes + 256);
        let v = self.records.next_record_bounded(remaining)?;
        if let Some(v) = &v {
            self.bytes += self.records.bytes + 256;
            self.pending.push_back((self.records.line, v.clone()));
        }
        Ok(v)
    }
    fn next(&mut self) -> Result<Option<(usize, Value)>, String> {
        if let Some((line, v)) = self.pending.pop_front() {
            // Drop all accounting when the queue drains; encoded length is not
            // needed during draining because no lookahead occurs until an open.
            if self.pending.is_empty() {
                self.bytes = 0;
            }
            return Ok(Some((line, v)));
        }
        Ok(self.records.next_record()?.map(|v| (self.records.line, v)))
    }
    fn token(&mut self) -> Result<String, String> {
        let mut outbound = Vec::new();
        let mut index = 0;
        loop {
            let v = if let Some((_, v)) = self.pending.get(index) {
                v.clone()
            } else {
                self.read_ahead()
                    .map_err(|e| {
                        format!(
                            "readiness token not found within current connection lookahead: {e}"
                        )
                    })?
                    .ok_or("missing recorded readiness token before end of connection")?
            };
            index += 1;
            if v["conn"] == "close" || v["conn"] == "open" {
                return Err("missing recorded readiness token before end of connection".into());
            }
            if v["dir"] != "out" {
                continue;
            }
            for byte in unbase64(v["b64"].as_str().ok_or("wire bytes")?).map_err(str::to_owned)? {
                if byte == b'\n' {
                    if let Ok(line) = std::str::from_utf8(&outbound)
                        && let Some(token) = line.trim_end_matches('\r').strip_prefix("echo ")
                        && qso_graph_dxcluster::values::token(token)
                    {
                        return Ok(token.into());
                    }
                    outbound.clear();
                } else {
                    if outbound.len() == 256 {
                        return Err("outbound observation exceeds protocol line bound".into());
                    }
                    outbound.push(byte);
                }
            }
        }
    }
}
/// Owns the forward-only recording reader and current session. Notes keep their
/// physical record numbers even when configuration or token discovery looks ahead.
pub struct ReplayStream<R: BufRead> {
    source: Lookahead<R>,
    replay: Replay,
}
impl<R: BufRead> ReplayStream<R> {
    pub fn new(reader: R, retain: bool) -> Result<Self, String> {
        let mut source = Lookahead {
            records: Records::new(reader),
            pending: Default::default(),
            bytes: 0,
        };
        let mut call = "Q0NONE".to_owned();
        let mut profile = Profile::Auto;
        let mut outbound = Vec::new();
        // Configuration is local metadata, never received identity evidence.
        // Legacy recordings infer the call only from the first outbound line.
        while let Some(v) = source
            .read_ahead()
            .map_err(|e| format!("configuration lookahead: {e}"))?
        {
            if let Some(note) = v["note"]
                .as_str()
                .and_then(|n| n.strip_prefix("qg-dxc configuration: "))
            {
                let c: Value =
                    serde_json::from_str(note).map_err(|_| "invalid recorded configuration")?;
                call = c["call"]
                    .as_str()
                    .filter(|s| qso_graph_dxcluster::values::login(s))
                    .ok_or("recorded identity")?
                    .into();
                profile = Profile::parse(c["profile"].as_str().ok_or("recorded profile")?)
                    .ok_or("recorded profile")?;
                break;
            }
            if v["conn"] == "close" {
                break;
            }
            if v["dir"] == "out" {
                let b = unbase64(v["b64"].as_str().ok_or("outbound observation")?)
                    .map_err(str::to_owned)?;
                let mut complete = false;
                for byte in b {
                    if byte == b'\n' {
                        complete = true;
                        break;
                    }
                    if outbound.len() == 256 {
                        return Err("outbound observation exceeds protocol line bound".into());
                    }
                    outbound.push(byte);
                }
                if complete {
                    if let Ok(line) = std::str::from_utf8(&outbound)
                        && qso_graph_dxcluster::values::login(line.trim_end_matches('\r'))
                    {
                        call = line.trim_end_matches('\r').into();
                    }
                    break;
                }
            }
        }
        Ok(Self {
            source,
            replay: Replay::new_with_retention(call, profile, retain)?,
        })
    }
    pub fn next_record(&mut self) -> Result<Option<(usize, Value, Vec<Event>)>, String> {
        let Some((line, v)) = self.source.next()? else {
            return Ok(None);
        };
        let attempt = self.replay.engine.attempt + u64::from(v["conn"] == "open");
        let source = &mut self.source;
        let events = self
            .replay
            .record_with_token(&v, &mut || source.token())
            .map_err(|e| format!("connection {} at record {line}: {e}", attempt))?;
        Ok(Some((line, v, events)))
    }
    pub fn finish(&mut self) -> Result<Vec<Event>, String> {
        self.replay.finish()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn configured(profile: &str) -> Recorder<Vec<u8>> {
        let mut r = Recorder::new(Vec::new(), true).unwrap();
        r.note(
            0.0,
            &format!("qg-dxc configuration: {{\"call\":\"Q1ABC\",\"profile\":\"{profile}\"}}"),
        )
        .unwrap();
        r
    }
    #[test]
    fn replay_stream_looks_up_fragmented_tokens_only_in_the_current_connection() {
        // QGDXC-REC-0004 QGDXC-REC-0005 QGDXC-SEC-0001
        let mut r = configured("auto");
        for (i, token) in ["QGDXC0000000000000001", "QGDXC0000000000000002"]
            .iter()
            .enumerate()
        {
            let t = i as f64 * 100.0;
            r.open(t).unwrap();
            r.wire(t, "out", b"Q1ABC\r\n").unwrap();
            r.note(t, "observer note").unwrap();
            r.wire(t, "out", format!("echo {}", &token[..8]).as_bytes())
                .unwrap();
            r.wire(t, "out", format!("{}\r", &token[8..]).as_bytes())
                .unwrap();
            r.wire(t, "out", b"\n").unwrap();
            r.close(t, "cancelled").unwrap();
        }
        let mut replay = ReplayStream::new(std::io::Cursor::new(r.writer), false).unwrap();
        assert_eq!(
            replay.source.records.line, 1,
            "no whole-file configuration pass"
        );
        let mut lines = 0;
        let mut tokens = 0;
        while let Some((line, v, _)) = replay.next_record().unwrap() {
            lines += 1;
            assert_eq!(line, lines);
            if v["conn"] == "open" {
                tokens += 1;
                assert_eq!(replay.replay.engine.attempt, tokens);
                assert!(replay.source.bytes <= LOOKAHEAD_BYTES);
                assert!(
                    replay
                        .source
                        .pending
                        .iter()
                        .all(|(_, v)| v["conn"].is_null())
                );
            }
        }
        assert_eq!(tokens, 2);
        assert!(replay.source.pending.is_empty());
        assert_eq!(replay.source.bytes, 0);
    }
    #[test]
    fn replay_stream_missing_token_does_not_borrow_from_the_next_connection() {
        // QGDXC-REC-0004 QGDXC-SEC-0001
        let mut r = configured("auto");
        r.open(0.0).unwrap();
        r.close(0.0, "cancelled").unwrap();
        r.open(0.0).unwrap();
        r.wire(0.0, "out", b"echo QGDXC0000000000000002\r\n")
            .unwrap();
        r.close(0.0, "cancelled").unwrap();
        let mut replay = ReplayStream::new(std::io::Cursor::new(r.writer), false).unwrap();
        replay.next_record().unwrap();
        let e = replay.next_record().unwrap_err();
        assert!(e.contains("connection 1 at record 2"), "{e}");
        assert!(e.contains("missing recorded readiness token"), "{e}");
        assert_eq!(replay.source.records.line, 3);
    }
    #[test]
    fn replay_stream_caps_lookahead_and_does_not_search_for_passive_profiles() {
        // QGDXC-REC-0004 QGDXC-SEC-0001
        for profile in ["auto", "rbn", "arcluster", "cccluster"] {
            let mut r = configured(profile);
            r.open(0.0).unwrap();
            for _ in 0..32 {
                r.note(0.0, &"x".repeat(65536)).unwrap();
            }
            r.wire(0.0, "out", b"echo QGDXC0000000000000001\r\n")
                .unwrap();
            r.close(0.0, "cancelled").unwrap();
            let mut replay = ReplayStream::new(std::io::Cursor::new(r.writer), false).unwrap();
            replay.next_record().unwrap();
            if profile == "auto" {
                let e = replay.next_record().unwrap_err();
                assert!(e.contains("readiness token not found"), "{e}");
                assert!(e.contains("byte limit"), "{e}");
                assert!(replay.source.bytes <= LOOKAHEAD_BYTES);
            } else {
                replay.next_record().unwrap();
                assert_eq!(replay.source.records.line, 2);
                assert!(replay.source.pending.is_empty());
                assert_eq!(replay.source.bytes, 0);
                while replay.next_record().unwrap().is_some() {}
            }
        }
        // Configuration discovery has the same cap, even without any connection.
        let mut r = Recorder::new(Vec::new(), true).unwrap();
        for _ in 0..32 {
            r.note(0.0, &"x".repeat(65536)).unwrap();
        }
        let e = ReplayStream::new(std::io::Cursor::new(r.writer), false)
            .err()
            .unwrap();
        assert!(e.contains("configuration lookahead"), "{e}");
    }
    fn wire(replay: &mut Replay, t: f64, dir: &str, bytes: &[u8]) -> Vec<Event> {
        replay
            .record(&json!({"t":t,"dir":dir,"b64":base64(bytes)}))
            .unwrap()
    }
    #[test]
    fn replay_preserves_automatic_retry_history_through_exhaustion() {
        // QGDXC-REC-0004 QGDXC-SES-0010
        let mut replay = Replay::new("Q1ABC".into(), Profile::Rbn).unwrap();
        let mut at = 0;
        let mut actual = vec![];
        let mut expected = vec![];
        for delay in [5000, 10000, 20000, 40000, 80000, 160000, 300000, 300000, 0] {
            actual.extend(
                replay
                    .record(&json!({"t":at as f64 / 1000.0,"conn":"open"}))
                    .unwrap(),
            );
            at += 1000;
            actual.extend(
                replay
                    .record(&json!({"t":at as f64 / 1000.0,"conn":"close","reason":"eof"}))
                    .unwrap(),
            );
            expected.extend([
                json!({"state":"connecting","reason":"started"}),
                json!({"state":"await_login","reason":"connected"}),
                json!({"state":"disconnected","reason":"eof"}),
            ]);
            expected.push(if delay == 0 {
                json!({"state":"blocked","reason":"retry_exhausted"})
            } else {
                json!({"state":"backoff","reason":"retry_scheduled","retry_in_ms":delay})
            });
            at += delay;
        }
        let fields: Vec<Value> = actual
            .iter()
            .map(|e| {
                assert_eq!(e.kind, "connection_state");
                serde_json::from_str::<Value>(&e.json()).unwrap()["fields"].clone()
            })
            .collect();
        assert_eq!(fields, expected);
        assert_eq!(replay.engine.state, State::Blocked);
        assert_eq!(replay.engine.attempt, 9);
        assert!(replay.finish().unwrap().is_empty());
    }
    #[test]
    fn replay_uses_last_outbound_fragment_for_admission() {
        // QGDXC-REC-0004 QGDXC-SES-0007
        let mut replay = Replay::new("Q1ABC".into(), Profile::CcCluster).unwrap();
        replay.record(&json!({"t":0,"conn":"open"})).unwrap();
        wire(&mut replay, 0.0, "in", b"login:");
        wire(&mut replay, 0.0, "out", b"Q1ABC\r\n");
        wire(
            &mut replay,
            0.1,
            "in",
            b"Q1ABC de Q2ABC 1-Nov-2026 1832Z CCC >",
        );
        wire(&mut replay, 1.0, "out", b"set/ve7");
        wire(&mut replay, 4.0, "out", b"cc\r");
        assert_eq!(replay.engine.deadline(Timer::Setup), None);
        wire(&mut replay, 5.0, "out", b"\n");
        assert_eq!(replay.engine.deadline(Timer::Setup), Some(35000));
        let events = wire(
            &mut replay,
            33.0,
            "in",
            b"Enhanced Spots Enabled\nQ1ABC de Q2ABC 1-Nov-2026 1832Z CCC >",
        );
        assert!(events.iter().any(|e| e.get("state") == Some("ready")));
        assert_eq!(replay.engine.state, State::Ready);
    }
    #[test]
    fn replay_waits_for_delayed_login_completion() {
        // QGDXC-REC-0004 QGDXC-SES-0007
        let mut replay = Replay::new("Q1ABC".into(), Profile::Rbn).unwrap();
        replay.record(&json!({"t":0,"conn":"open"})).unwrap();
        wire(&mut replay, 0.0, "in", b"login:");
        wire(&mut replay, 1.0, "out", b"Q1ABC\r");
        assert_eq!(replay.engine.deadline(Timer::Identity), None);
        wire(&mut replay, 40.0, "out", b"\n");
        assert_eq!(replay.engine.deadline(Timer::Identity), Some(70000));
        wire(
            &mut replay,
            60.0,
            "in",
            b"Q1ABC de RELAY 1-Nov-2026 18:32Z >",
        );
        assert_eq!(replay.engine.state, State::Ready);
    }
    #[test]
    fn replay_boundaries_own_attempts_and_delayed_termination() {
        // QGDXC-REC-0004 QGDXC-REC-0005 QGDXC-SES-0009
        for reason in ["timeout", "eof"] {
            let mut replay = Replay::new("Q1ABC".into(), Profile::Rbn).unwrap();
            replay.record(&json!({"t":0,"conn":"open"})).unwrap();
            let events = replay
                .record(&json!({"t":39,"note":"delayed boundary"}))
                .unwrap();
            assert!(events.is_empty());
            assert_eq!(replay.engine.attempt, 1);
            assert_eq!(replay.engine.state, State::AwaitLogin);
            let events = replay
                .record(&json!({"t":40,"conn":"close","reason":reason}))
                .unwrap();
            assert!(events.iter().any(|e| e.get("reason") == Some(reason)));
            assert!(!events.iter().any(|e| e.get("state") == Some("connecting")));
            replay.record(&json!({"t":100,"conn":"open"})).unwrap();
            assert_eq!(replay.engine.attempt, 2);
            replay
                .record(&json!({"t":140,"conn":"close","reason":reason}))
                .unwrap();
            replay.finish().unwrap();
            assert_eq!(replay.engine.attempt, 2);
        }
    }
    #[test]
    fn replay_defers_received_termination_until_recorded_close() {
        // QGDXC-REC-0004 QGDXC-SES-0014
        let mut replay = Replay::new("Q1ABC".into(), Profile::Rbn).unwrap();
        replay.record(&json!({"t":0,"conn":"open"})).unwrap();
        let events = wire(&mut replay, 0.0, "in", b"password:");
        assert!(!events.iter().any(|e| e.kind == "connection_state"));
        assert_eq!(replay.engine.state, State::AwaitLogin);
        let events = replay
            .record(&json!({"t":40,"conn":"close","reason":"password_unsupported"}))
            .unwrap();
        assert!(
            events
                .iter()
                .any(|e| e.get("reason") == Some("password_unsupported"))
        );
        assert_eq!(replay.engine.state, State::Blocked);
    }
    #[test]
    fn recording_boundaries_and_replay() {
        // QGDXC-REC-0001 QGDXC-REC-0002 QGDXC-REC-0003 QGDXC-REC-0004 QGDXC-REC-0005
        assert!(Recorder::new(Vec::new(), false).is_err());
        let mut r = Recorder::new(Vec::new(), true).unwrap();
        r.note(
            0.0,
            "qg-dxc configuration: {\"call\":\"Q1ABC\",\"profile\":\"rbn\"}",
        )
        .unwrap();
        for t in [0.0, 100.0] {
            r.open(t).unwrap();
            r.wire(t, "in", b"call:").unwrap();
            r.wire(t, "out", b"Q1").unwrap();
            r.wire(t, "out", b"ABC\r").unwrap();
            r.wire(t, "out", b"\n").unwrap();
            r.wire(t, "in", b"Q1ABC de RELAY 1-Nov-2026 18:32Z >\r")
                .unwrap();
            r.close(t + 1.0, "eof").unwrap();
        }
        let mut replay = ReplayStream::new(std::io::Cursor::new(r.writer), false).unwrap();
        let mut events = vec![];
        while let Some((_, _, batch)) = replay.next_record().unwrap() {
            events.extend(batch);
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
                match rows.next_record() {
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
