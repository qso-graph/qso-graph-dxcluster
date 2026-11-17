// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    Event, Profile, Value,
    framing::{Frame, Framer, Telnet},
    values as v,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Context {
    pub profile: Profile,
    pub identity: String,
    pub login: bool,
    pub post_login: bool,
    pub password: bool,
    pub identity_context: bool,
    pub ar_banner: bool,
    pub custom_prompt: Option<String>,
    pub expected_reply: Option<String>,
    pub setup_errors: Vec<String>,
    pub retain_third_party_data: bool,
}
impl Context {
    pub fn ready(profile: Profile) -> Self {
        Self {
            profile,
            identity: String::new(),
            login: false,
            post_login: true,
            password: false,
            identity_context: false,
            ar_banner: false,
            custom_prompt: None,
            expected_reply: None,
            setup_errors: Vec::new(),
            retain_third_party_data: false,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Parser {
    pub context: Context,
    framer: Framer,
    telnet: Telnet,
    application: bool,
}
impl Parser {
    pub fn new(context: Context, limit: usize) -> Result<Self, &'static str> {
        Ok(Self {
            context,
            framer: Framer::new(limit)?,
            telnet: Telnet::default(),
            application: false,
        })
    }
    /// At most three events per byte; the caller drains before its next call.
    pub fn push(&mut self, b: u8, received_at: Option<&str>) -> Result<Vec<Event>, &'static str> {
        if received_at.is_some_and(|s| !v::receipt(s)) {
            return Err("invalid UTC receipt timestamp");
        }
        self.application = false;
        let Some(b) = self.telnet.push(b)? else {
            return Ok(Vec::new());
        };
        self.application = true;
        let mut out = Vec::new();
        for f in self.framer.push(b) {
            let e = self.event(f, received_at);
            out.push(e);
        }
        if self.prompt_fragment() {
            let f = self.framer.consume_prompt();
            out.push(self.event(f, received_at));
        }
        Ok(out)
    }
    pub fn last_application(&self) -> bool {
        self.application
    }
    pub fn eof(&mut self, received_at: Option<&str>) -> (Vec<Event>, bool) {
        let failed = self.telnet.eof().is_err();
        let out = self
            .framer
            .eof()
            .map(|f| self.event(f, received_at))
            .into_iter()
            .collect();
        (out, failed)
    }
    fn prompt_fragment(&self) -> bool {
        let b = self.framer.content();
        if !matches!(b.last(), Some(b':' | b'>')) {
            return false;
        }
        let (s, _) = v::decode(b);
        if message_prefix(&s, &self.context) {
            return false;
        }
        if self.context.password && boundary(&s, "password:") {
            return true;
        }
        if self.context.login && (boundary(&s, "login:") || boundary(&s, "call:")) {
            return true;
        }
        self.context.post_login && ready_profile(&s, &self.context).is_some()
    }
    fn event(&mut self, frame: Frame, received_at: Option<&str>) -> Event {
        let semantic = frame.reason.is_none();
        let event = parse_frame(frame, &self.context, received_at);
        let text = v::decode(&event.raw_line).0;
        if semantic && ar_banner(&text) {
            self.context.ar_banner = true;
        }
        if self.context.profile == Profile::Auto {
            let identified = match event.get("status") {
                Some("ready_prompt") => ready_profile(&text, &self.context),
                Some("server_identity") => identity_profile(&text, &self.context),
                _ => None,
            };
            if let Some(profile) = identified {
                self.context.profile = profile;
            }
        }
        event
    }
}
pub fn boundary(s: &str, token: &str) -> bool {
    s.get(s.len().saturating_sub(token.len())..)
        .is_some_and(|tail| tail.eq_ignore_ascii_case(token))
        && (s.len() == token.len()
            || s.as_bytes()
                .get(s.len().saturating_sub(token.len() + 1))
                .is_some_and(u8::is_ascii_whitespace))
}
/// Complete ready signatures, shared by incremental consumption and line dispatch.
pub fn ready_profile(s: &str, c: &Context) -> Option<Profile> {
    if c.profile == Profile::DxSpider && c.custom_prompt.as_deref() == Some(s) {
        return Some(Profile::DxSpider);
    }
    let p: Vec<_> = s.split_ascii_whitespace().collect();
    if p.len() < 6
        || !p[0].eq_ignore_ascii_case(&c.identity)
        || p[1] != "de"
        || !v::received_call(p[2])
    {
        return None;
    }
    if p.len() == 7
        && p[5] == "dxspider"
        && p[6] == ">"
        && v::date(p[3]).is_some()
        && v::time(p[4], true).is_some()
    {
        return Some(Profile::DxSpider);
    }
    if p.len() == 7
        && p[5] == "CCC"
        && p[6] == ">"
        && v::date(p[3]).is_some()
        && v::time(p[4], true).is_some()
    {
        return Some(Profile::CcCluster);
    }
    if p.len() == 6 && p[2] == "RELAY" && p[5] == ">" && v::date(p[3]).is_some() {
        let clock = p[4];
        if clock.len() == 6
            && clock.as_bytes()[2] == b':'
            && v::time(&clock.replace(':', ""), true).is_some()
        {
            return Some(Profile::Rbn);
        }
    }
    if p.len() == 6 && v::short_date(p[3]) && v::time(p[4], true).is_some() {
        let digit = p[5].strip_prefix("arc")?.strip_suffix('>')?;
        if !digit.is_empty()
            && digit.bytes().all(|b| b.is_ascii_digit())
            && (digit == "6" || c.ar_banner)
        {
            return Some(Profile::ArCluster);
        }
    }
    None
}
pub fn ar_banner(s: &str) -> bool {
    ["running AR-Cluster ", "Running AR-Cluster Version "]
        .iter()
        .any(|marker| {
            s.split_once(marker).is_some_and(|(_, tail)| {
                tail.split_ascii_whitespace().next().is_some_and(|v| {
                    v.as_bytes().first().is_some_and(u8::is_ascii_digit)
                        && v.bytes().all(|b| b.is_ascii_digit() || b == b'.')
                })
            })
        })
}
pub fn identity_profile(s: &str, c: &Context) -> Option<Profile> {
    if c.identity_context && s.starts_with("Capabilities:") {
        let p: Vec<_> = s[13..].split_ascii_whitespace().collect();
        if p.contains(&"ve7cc") && p.contains(&"rbn") {
            return Some(Profile::DxSpider);
        }
    }
    if (c.login || c.identity_context) && s.starts_with("Running CC Cluster software version ") {
        return Some(Profile::CcCluster);
    }
    None
}
pub fn notice(s: &str, c: &Context) -> Option<&'static str> {
    let s = s.trim_matches(' ');
    if (c.login || c.password)
        && matches!(c.profile, Profile::Auto | Profile::DxSpider)
        && let Some(call) = s
            .strip_prefix("Sorry ")
            .and_then(|s| s.strip_suffix(" is an invalid callsign"))
        && call.eq_ignore_ascii_case(&c.identity)
    {
        return Some("refused");
    }
    if c.post_login
        && c.profile == Profile::DxSpider
        && let Some((call, addr)) = s
            .strip_prefix("Reconnected as ")
            .and_then(|s| s.strip_suffix(", this instance is disconnected"))
            .and_then(|s| s.split_once(" at "))
        && call.eq_ignore_ascii_case(&c.identity)
        && !addr.is_empty()
    {
        return Some("displaced");
    }
    None
}
// Reserved message layouts cannot act as session prompts or setup replies.
fn message_prefix(s: &str, c: &Context) -> bool {
    [
        "DX de ",
        "CC11^",
        "To ALL de ",
        "WX de ",
        "WWV de ",
        "WCY de ",
    ]
    .iter()
    .any(|prefix| s.starts_with(prefix))
        || c.profile == Profile::DxSpider
            && s.split_once(':').is_some_and(|(header, _)| {
                let h: Vec<_> = header.split_ascii_whitespace().collect();
                h.len() == 3 && h[1] == "de"
            })
}
pub fn parse_frame(frame: Frame, c: &Context, received_at: Option<&str>) -> Event {
    let (text, encoding) = v::decode(&frame.bytes);
    let mut e = Event::new(c.profile, "unrecognized_line");
    e.raw_line = frame.bytes;
    e.raw_ending = frame.ending;
    e.encoding = encoding;
    e.raw_status = if frame.reason == Some("line_too_long") {
        "truncated"
    } else {
        "exact"
    };
    e.received_at = received_at.map(str::to_owned);
    let s = text.as_str();
    let mut reason = frame.reason;
    if reason.is_none() {
        let status = if message_prefix(s, c) {
            None
        } else if c.password && boundary(s, "password:") {
            Some("password_prompt")
        } else if c.login && (boundary(s, "login:") || boundary(s, "call:")) {
            Some("login_prompt")
        } else if notice(s, c).is_some() {
            Some("notice")
        } else if c.post_login && ready_profile(s, c).is_some() {
            Some("ready_prompt")
        } else if identity_profile(s, c).is_some() {
            Some("server_identity")
        } else if c.expected_reply.as_deref() == Some(s) {
            Some("setup_reply")
        } else if c.setup_errors.iter().any(|error| error == s) {
            Some("setup_error")
        } else {
            None
        };
        if let Some(status) = status {
            e.kind = "prompt_status";
            e.set("status", status);
            e.set("text", s);
        } else if c.profile == Profile::Auto && (s.starts_with("DX de ") || s.starts_with("CC11^"))
        {
            reason = Some("before_identification");
        } else if s.starts_with("DX de ") {
            if !dx(s, c.profile, &mut e) {
                reason = Some("malformed");
            }
        } else if s.starts_with("CC11^") {
            if !matches!(c.profile, Profile::DxSpider | Profile::CcCluster) {
                reason = Some("unsupported_layout");
            } else {
                reason = cc11(s, c.profile, &mut e).err();
            }
        } else if ["To ALL de ", "WX de ", "WWV de ", "WCY de "]
            .iter()
            .any(|p| s.starts_with(p))
        {
            if c.profile == Profile::DxSpider
                || c.profile == Profile::CcCluster && s.starts_with("To ALL de ")
            {
                if !other(s, c.profile, &mut e) {
                    reason = Some("malformed");
                }
            } else {
                reason = Some("unsupported_layout");
            }
        } else if c.profile == Profile::DxSpider && s.contains(" de ") {
            if !other(s, c.profile, &mut e) {
                reason = Some("malformed");
            }
        } else if c.profile == Profile::ArCluster && history(s) {
            reason = Some("unsupported_layout");
        } else {
            reason = Some("unknown");
        }
    }
    if let Some(reason) = reason {
        e.kind = "unrecognized_line";
        e.fields.clear();
        e.set("reason", reason);
        e.set("text", s);
    }
    crate::privacy::apply(&mut e, c.retain_third_party_data);
    e
}
fn spot(
    e: &mut Event,
    p: Profile,
    format: &str,
    calls: (&str, &str),
    freq: i64,
    time: String,
    comment: &str,
) {
    let (spotter, call) = calls;
    e.kind = "spot";
    e.set(
        "spot_kind",
        if p == Profile::Rbn || spotter.ends_with("-#") {
            "skimmer"
        } else {
            "human"
        },
    );
    e.set("source", if p == Profile::Rbn { "rbn" } else { "cluster" });
    e.set("format", format);
    e.set("spotter_call", spotter);
    e.set("call", call);
    e.set("frequency_hz", Value::Integer(freq));
    e.set("time_of_day", time);
    e.set("comment", comment);
}
fn word(s: &str) -> Option<(&str, &str)> {
    let end = s.find(' ').unwrap_or(s.len());
    (end > 0).then(|| (&s[..end], s[end..].trim_start_matches(' ')))
}
fn dx(s: &str, p: Profile, e: &mut Event) -> bool {
    let Some(s) = s.strip_prefix("DX de ") else {
        return false;
    };
    let Some((spotter, rest)) = s.trim_start_matches(' ').split_once(':') else {
        return false;
    };
    if !v::received_call(spotter) || !rest.starts_with(' ') {
        return false;
    }
    let Some((f, rest)) = word(rest.trim_start_matches(' ')) else {
        return false;
    };
    let Some(freq) = v::frequency(f) else {
        return false;
    };
    let Some((call, rest)) = word(rest) else {
        return false;
    };
    if !v::received_call(call) {
        return false;
    }
    let rest = if p == Profile::DxSpider {
        rest.strip_suffix("\x07\x07").unwrap_or(rest)
    } else {
        rest
    };
    let mut found = None;
    for (i, _) in rest.char_indices().filter(|(_, c)| *c != ' ') {
        if i > 0 && rest.as_bytes()[i - 1] != b' ' {
            continue;
        }
        let Some(t) = rest.get(i..i + 5).and_then(|t| v::time(t, true)) else {
            continue;
        };
        let suffix = &rest[i + 5..];
        let extra = suffix.trim_matches(' ');
        let valid = if extra.is_empty() {
            true
        } else {
            p == Profile::DxSpider
                && suffix.starts_with(' ')
                && (v::grid(extra)
                    || (extra.len() == 2 && extra.bytes().all(|b| b.is_ascii_alphabetic()))
                    || ((1..=2).contains(&extra.len())
                        && extra.bytes().all(|b| b.is_ascii_digit())))
        };
        if valid {
            if found.is_some() {
                return false;
            }
            found = Some((i, t, suffix));
        }
    }
    let Some((i, time, suffix)) = found else {
        return false;
    };
    let comment = rest[..i].trim_matches(' ');
    spot(e, p, "dx_de", (spotter, call), freq, time, comment);
    if !suffix.trim_matches(' ').is_empty() {
        e.set(
            "native",
            Value::Object(BTreeMap::from([(
                "trailing_extra".into(),
                Value::from(suffix),
            )])),
        );
    }
    if p == Profile::Rbn {
        rbn(comment, e);
    }
    true
}
fn cc11(s: &str, p: Profile, e: &mut Event) -> Result<(), &'static str> {
    let parts: Vec<_> = s.split('^').collect();
    if (p == Profile::CcCluster && parts.len() != 25)
        || (p == Profile::DxSpider && parts.len() < 20)
    {
        return Err("unsupported_layout");
    }
    let mut f = parts.clone();
    let comment = if p == Profile::DxSpider {
        let tail = parts.len() - 14;
        let comment = parts[5..tail].join("^");
        f = parts[..5]
            .iter()
            .chain(std::iter::once(&""))
            .chain(parts[tail..].iter())
            .copied()
            .collect();
        comment
    } else {
        parts[5].to_owned()
    };
    let freq = v::frequency(f[1]).ok_or("malformed")?;
    let date = v::date(f[3]).ok_or("malformed")?;
    let time = v::time(f[4], p == Profile::DxSpider).ok_or("malformed")?;
    if !v::received_call(f[2]) || !v::received_call(f[6]) {
        return Err("malformed");
    }
    spot(
        e,
        p,
        if p == Profile::DxSpider {
            "cc11_dxspider"
        } else {
            "cc11_cccluster"
        },
        (f[6], f[2]),
        freq,
        time,
        &comment,
    );
    e.set("date", date);
    let mut native = BTreeMap::new();
    if p == Profile::DxSpider {
        let names = [
            "dx_country_code",
            "spotter_country_code",
            "origin",
            "dx_itu",
            "dx_cq",
            "spotter_itu",
            "spotter_cq",
            "dx_state",
            "spotter_state",
            "dx_country_text",
            "spotter_country_text",
            "dx_grid",
            "spotter_grid",
        ];
        for (name, field) in names.iter().zip(&f[7..]) {
            native.insert((*name).into(), Value::from(*field));
        }
        if v::grid(f[18]) {
            e.set("grid", f[18]);
        }
    } else {
        for (i, field) in f.iter().enumerate().skip(7) {
            if (i == 22 || i == 23) && e.raw_status != "exact" {
                continue;
            }
            native.insert(
                if i == 23 {
                    "spotter_ip".into()
                } else {
                    format!("slot_{i:02}")
                },
                Value::from(*field),
            );
        }
    }
    e.set("native", Value::Object(native));
    Ok(())
}
fn rbn(comment: &str, e: &mut Event) {
    let p: Vec<_> = comment.split_ascii_whitespace().collect();
    if p.len() < 4 {
        return;
    }
    let mode = p[0];
    if !["CW", "RTTY", "FT8", "FT4"].contains(&mode) || p[1].len() > 6 || p[2] != "dB" {
        return;
    }
    let Ok(snr) = p[1].parse::<i16>() else {
        return;
    };
    if !p[1]
        .trim_start_matches(['+', '-'])
        .bytes()
        .all(|b| b.is_ascii_digit())
    {
        return;
    }
    let end = if p.ends_with(&["NCDXF", "B"]) { 2 } else { 1 };
    let ty = p[p.len() - end..].join(" ");
    if !["CQ", "BEACON", "NCDXF B", "DX"].contains(&ty.as_str()) {
        return;
    }
    let middle = &p[3..p.len() - end];
    if matches!(mode, "CW" | "RTTY") {
        if middle.len() != 2
            || middle[1] != if mode == "CW" { "WPM" } else { "BPS" }
            || middle[0].len() > 6
            || !middle[0].bytes().all(|b| b.is_ascii_digit())
        {
            return;
        }
        let Ok(speed) = middle[0].parse::<u16>() else {
            return;
        };
        e.set("speed", Value::Integer(i64::from(speed)));
        e.set("speed_unit", if mode == "CW" { "wpm" } else { "bps" });
    } else {
        if middle.len() > 1 {
            return;
        }
        if let Some(slot) = middle.first() {
            if v::grid(slot) {
                e.set("grid", *slot);
            } else {
                e.set(
                    "native",
                    Value::Object(BTreeMap::from([("rbn_slot".into(), Value::from(*slot))])),
                );
            }
        }
    }
    e.set("mode", mode);
    e.set("snr_db", Value::Integer(i64::from(snr)));
    e.set("spot_type", ty);
}
fn history(s: &str) -> bool {
    let p: Vec<_> = s.split_ascii_whitespace().collect();
    p.len() >= 5
        && v::frequency(p[0]).is_some()
        && v::received_call(p[1])
        && v::date(p[2]).is_some()
        && v::time(p[3], true).is_some()
        && p.last()
            .is_some_and(|p| p.starts_with('<') && p.ends_with('>'))
}
fn other(s: &str, p: Profile, e: &mut Event) -> bool {
    let s = s.strip_suffix("\x07\x07").unwrap_or(s);
    if p == Profile::CcCluster {
        let Some(rest) = s.strip_prefix("To ALL de ") else {
            return false;
        };
        let Some((header, text)) = rest.split_once(':') else {
            return false;
        };
        let h: Vec<_> = header.split_ascii_whitespace().collect();
        if h.len() != 2 || !v::received_call(h[0]) {
            return false;
        }
        let Some(via) = h[1].strip_prefix('<').and_then(|s| s.strip_suffix('>')) else {
            return false;
        };
        if !v::received_call(via) {
            return false;
        }
        e.kind = "announcement";
        e.set("text", text.trim_matches(' '));
        e.set("from_call", h[0]);
        e.set("via_call", via);
        e.set("target", "ALL");
        e.set("category", "all");
        return true;
    }
    if s.starts_with("WWV de ") || s.starts_with("WCY de ") {
        return propagation(s, e);
    }
    let (rest, target, category) = if let Some(s) = s.strip_prefix("To ALL de ") {
        (s, Some("ALL"), Some("all"))
    } else if let Some(s) = s.strip_prefix("WX de ") {
        (s, Some("WX"), Some("weather"))
    } else {
        (s, None, None)
    };
    let Some((header, text)) = rest.split_once(':') else {
        return false;
    };
    if let Some(target) = target {
        let from = header.trim_matches(' ');
        if !v::received_call(from) {
            return false;
        }
        e.kind = "announcement";
        e.set("from_call", from);
        e.set("target", target);
        e.set("category", category.unwrap_or("all"));
    } else {
        let h: Vec<_> = header.split_ascii_whitespace().collect();
        if h.len() != 3 || h[1] != "de" || !v::received_call(h[0]) || !v::received_call(h[2]) {
            return false;
        }
        e.kind = "talk";
        e.set("to_call", h[0]);
        e.set("from_call", h[2]);
    }
    e.set("text", text.trim_matches(' '));
    true
}
fn propagation(s: &str, e: &mut Event) -> bool {
    let wwv = s.starts_with("WWV de ");
    let Some((header, text)) = s[7..].split_once(':') else {
        return false;
    };
    let h: Vec<_> = header.split_ascii_whitespace().collect();
    if h.len() != 2 || !v::received_call(h[0]) {
        return false;
    }
    let Some(hour) = h[1]
        .strip_prefix('<')
        .and_then(|s| s.strip_suffix('>'))
        .and_then(|s| s.parse::<u8>().ok())
        .filter(|h| *h < 24)
    else {
        return false;
    };
    e.kind = if wwv { "wwv" } else { "wcy" };
    e.set("from_call", h[0]);
    e.set("hour", Value::Integer(i64::from(hour)));
    e.set("text", text.trim_matches(' '));
    let p: Vec<_> = text.split_ascii_whitespace().collect();
    for token in &p {
        if let Some((key, val)) = token.trim_end_matches(',').split_once('=') {
            let name = match key {
                "SFI" => "sfi",
                "A" => "a",
                "K" => "k",
                "expK" if !wwv => "expected_k",
                "R" if !wwv => "r",
                "SA" if !wwv => "sa",
                "GMF" if !wwv => "gmf",
                "Au" if !wwv => "au",
                _ => continue,
            };
            if matches!(name, "sa" | "gmf" | "au") {
                e.set(name, val);
            } else if let Ok(n) = val.parse::<i64>()
                && n.unsigned_abs() <= v::MAX_INTEGER
            {
                e.set(name, Value::Integer(n));
            }
        }
    }
    if wwv {
        let chunks: Vec<_> = text.splitn(4, ',').collect();
        if chunks.len() == 4 {
            e.set("forecast", chunks[3].trim_matches(' '));
        }
    }
    true
}
