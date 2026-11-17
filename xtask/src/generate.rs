// SPDX-License-Identifier: GPL-3.0-or-later
use crate::oracle;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
fn callish(s: &str) -> bool {
    s.starts_with('Q')
        || s.bytes().any(|b| b.is_ascii_alphabetic())
            && s.bytes().any(|b| b.is_ascii_digit())
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'#' | b'_'))
}
fn replace_tokens(s: &str, names: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    let mut start = 0;
    for (i, c) in s.char_indices() {
        if !(c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '#' | '_')) {
            if i > start {
                let t = &s[start..i];
                out.push_str(names.get(t).map(String::as_str).unwrap_or(t));
            }
            out.push(c);
            start = i + c.len_utf8();
        }
    }
    if start < s.len() {
        let t = &s[start..];
        out.push_str(names.get(t).map(String::as_str).unwrap_or(t));
    }
    out
}
fn collect_names(s: &str, names: &mut BTreeSet<String>) {
    for t in s.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '#' | '_'))) {
        if callish(t)
            && !matches!(t, "CC11" | "FT8" | "FT4")
            && !(t.len() == 5
                && t.ends_with('Z')
                && t.as_bytes()[..4].iter().all(u8::is_ascii_digit))
            && !t.contains("-Nov-")
        {
            names.insert(t.into());
        }
    }
}
fn role_ranges(s: &str, p: &str) -> Vec<(usize, usize)> {
    if let Some(body) = s.strip_prefix("DX de ") {
        let first = 6 + body.len() - body.trim_start_matches(' ').len();
        if let Some(colon) = s[first..].find(':').map(|n| n + first) {
            let rest = &s[colon + 1..];
            let freq = colon + 1 + rest.len() - rest.trim_start_matches(' ').len();
            if let Some(end) = s[freq..].find(' ').map(|n| freq + n) {
                let rest = &s[end..];
                let call = end + rest.len() - rest.trim_start_matches(' ').len();
                let call_end = s[call..].find(' ').map_or(s.len(), |n| call + n);
                return vec![(first, colon), (call, call_end)];
            }
        }
    }
    if s.starts_with("CC11^") {
        let f: Vec<_> = s.split('^').collect();
        let positions = if p == "dxspider" && f.len() >= 20 {
            vec![2, f.len() - 14, f.len() - 11]
        } else if p == "cccluster" && f.len() == 25 {
            vec![2, 6, 8, 9]
        } else {
            vec![]
        };
        let mut offset = 0;
        let mut out = vec![];
        for (i, field) in f.iter().enumerate() {
            if positions.contains(&i) && !field.is_empty() {
                out.push((offset, offset + field.len()));
            }
            offset += field.len() + 1;
        }
        return out;
    }
    vec![]
}
fn replace_roles(s: &str, p: &str, names: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    let mut start = 0;
    for (a, b) in role_ranges(s, p) {
        out.push_str(&replace_tokens(&s[start..a], names));
        out.push_str(names.get(&s[a..b]).map(String::as_str).unwrap_or(&s[a..b]));
        start = b;
    }
    out.push_str(&replace_tokens(&s[start..], names));
    out
}
fn public_token(s: &str) -> String {
    if s.contains('@') {
        return "<contact>".into();
    }
    if s.contains("://")
        || s.rsplit_once('.').is_some_and(|(a, b)| {
            !a.is_empty() && b.len() >= 2 && b.bytes().all(|b| b.is_ascii_alphabetic())
        })
    {
        return "<endpoint>".into();
    }
    s.into()
}
pub fn vector(id: &str, profile: &str, bytes: &[u8], ending: &str, reason: Option<&str>) -> Value {
    let mut wire = bytes.to_vec();
    wire.extend_from_slice(match ending {
        "crlf" => b"\r\n",
        "cr" => b"\r",
        "lf" => b"\n",
        _ => b"",
    });
    let mut expected = oracle::line(profile, bytes, ending, reason);
    default_privacy(&mut expected, profile, false);
    let steps = if ending == "none" || ending == "cr" {
        json!([{"type":"input","elapsed_ms":0,"b64":oracle::b64(&wire)},{"type":"transport","outcome":"eof","elapsed_ms":0},{"type":"tick","elapsed_ms":0}])
    } else {
        json!([{"type":"input","elapsed_ms":0,"b64":oracle::b64(&wire)},{"type":"tick","elapsed_ms":0}])
    };
    let trigger = if ending == "none" || ending == "cr" {
        1
    } else {
        0
    };
    json!({"SPDX-License-Identifier":"GPL-3.0-or-later","id":id,"spec":"0.1.0","class":"Event parser","profile":profile,"provenance":{"capture":"synthetic","records":[],"sanitization":["Invented Q-prefix identities; sensitive slots removed"],"derived_from":"QGDXC-VEC-0002"},"config":{"line_limit":4096,"profile":profile,"retain_third_party_data":false,"connect_ms":30000,"login_ms":30000,"identity_ms":30000,"setup_ms":30000},"tokens":[],"jitter_ms":[],"steps":steps,"expected_events":[{"step":trigger,"elapsed_ms":0,"event":expected}],"expected_actions":[]})
}
/// An independently implemented privacy oracle, including malformed CC11.
pub fn default_privacy(e: &mut Value, _p: &str, retain: bool) {
    if retain {
        return;
    }
    fn ip(s: &str) -> bool {
        s.parse::<std::net::IpAddr>().is_ok()
    }
    fn redact(s: &str) -> String {
        let mut result = String::new();
        let mut i = 0;
        let b = s.as_bytes();
        while i < b.len() {
            if b[i].is_ascii_hexdigit() || b[i] == b':' {
                let mut end = i;
                while end < b.len() && (b[end].is_ascii_hexdigit() || matches!(b[end], b':' | b'.'))
                {
                    end += 1;
                }
                if (i == 0 || !b[i - 1].is_ascii_alphanumeric()) && s.get(i..end).is_some_and(ip) {
                    result.push_str("<ip>");
                    i = end;
                    continue;
                }
            }
            let c = s[i..].chars().next().expect("character");
            result.push(c);
            i += c.len_utf8();
        }
        result
    }
    fn fields(v: &mut Value) {
        match v {
            Value::String(s) => *s = redact(s),
            Value::Object(o) => {
                for v in o.values_mut() {
                    fields(v)
                }
            }
            _ => {}
        }
    }
    let withheld = ["call", "spotter_call", "from_call", "to_call"]
        .iter()
        .any(|k| e["fields"][k].as_str().is_some_and(ip));
    let raw = oracle::unb64(e["raw_line"].as_str().unwrap()).unwrap();
    let text = String::from_utf8(raw.clone())
        .unwrap_or_else(|_| raw.iter().map(|b| char::from(*b)).collect());
    let mut transformed = text.clone();
    let mut withheld = withheld;
    if transformed.contains("CC11^") {
        withheld |= !transformed.trim_start().starts_with("CC11^");
        let mut slots: Vec<_> = transformed.split('^').map(str::to_owned).collect();
        if slots.len() == 20 {
            // Fixed DXSpider fields contain no coordinate slots.
        } else if slots.len() != 25 || e["fields"]["format"] == "cc11_dxspider" {
            withheld = true;
        } else {
            slots[22].clear();
            slots[23].clear();
            transformed = slots.join("^");
            if let Some(n) = e["fields"].get_mut("native").and_then(Value::as_object_mut) {
                n.remove("slot_22");
                n.remove("spotter_ip");
            }
            if e["fields"].get("text").is_some() {
                e["fields"]["text"] = json!(transformed);
            }
        }
    }
    if withheld {
        e["kind"] = json!("unrecognized_line");
        e["fields"] = json!({"reason":"privacy_withheld","text":""});
        e["raw_line"] = json!("");
        e["raw_status"] = json!("withheld");
        return;
    }
    transformed = redact(&transformed);
    fields(&mut e["fields"]);
    if transformed != text {
        e["raw_status"] = json!("redacted");
        let encoded = if e["encoding"] == "latin1" {
            transformed.chars().map(|c| c as u8).collect()
        } else {
            transformed.into_bytes()
        };
        e["raw_line"] = json!(oracle::b64(&encoded));
    }
}
pub fn generate(captures: Option<&Path>) -> Result<(), String> {
    let mut vectors = synthetic();
    if let Some(dir) = captures {
        let mut paths: Vec<_> = fs::read_dir(dir)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "ndjson"))
            .collect();
        paths.sort();
        let mut captured = vec![];
        let mut names = BTreeSet::new();
        for path in paths {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let p = name.split('-').next().unwrap();
            let profile = if p == "dxspider" {
                "dxspider"
            } else if p == "arcluster" {
                "arcluster"
            } else if p == "cccluster" {
                "cccluster"
            } else {
                "rbn"
            };
            let mut bytes = vec![];
            let mut records = vec![];
            for (i, line) in fs::read_to_string(&path)
                .map_err(|e| e.to_string())?
                .lines()
                .enumerate()
            {
                let r: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
                if r["dir"] != "in" {
                    continue;
                }
                let b = oracle::unb64(r["b64"].as_str().ok_or("capture byte member")?)?;
                records.extend(std::iter::repeat_n(i + 1, b.len()));
                bytes.extend(b);
            }
            let mut at = 0;
            let mut counts = BTreeMap::<String, usize>::new();
            while at < bytes.len() {
                let start = at;
                while at < bytes.len() && !matches!(bytes[at], b'\r' | b'\n') {
                    at += 1;
                }
                let end = at;
                let ending = if bytes.get(at..at + 2) == Some(b"\r\n") {
                    at += 2;
                    "crlf"
                } else if bytes.get(at) == Some(&b'\r') {
                    at += 1;
                    "cr"
                } else if bytes.get(at) == Some(&b'\n') {
                    at += 1;
                    "lf"
                } else {
                    "none"
                };
                let content = &bytes[start..end];
                let text = String::from_utf8_lossy(content);
                if ![
                    "DX de ",
                    "CC11^",
                    "To ALL de ",
                    "WX de ",
                    "WWV de ",
                    "WCY de ",
                ]
                .iter()
                .any(|s| text.starts_with(s))
                {
                    continue;
                }
                let category = if text.starts_with("DX de ") {
                    let words: Vec<_> = text.split_ascii_whitespace().collect();
                    if profile == "rbn" {
                        words.get(5).unwrap_or(&"DX").to_string()
                            + words.get(words.len().saturating_sub(2)).unwrap_or(&"")
                    } else {
                        "dx_de".into()
                    }
                } else {
                    text.split('^')
                        .next()
                        .unwrap_or("other")
                        .chars()
                        .take(8)
                        .collect()
                };
                let count = counts.entry(category).or_default();
                if *count >= 24 {
                    continue;
                }
                *count += 1;
                collect_names(&text, &mut names);
                for (a, b) in role_ranges(&text, profile) {
                    names.insert(text[a..b].to_owned());
                }
                if text.starts_with("CC11^") {
                    let f: Vec<_> = text.split('^').collect();
                    let positions = if profile == "dxspider" && f.len() >= 20 {
                        vec![f.len() - 11]
                    } else if profile == "cccluster" && f.len() == 25 {
                        vec![8, 9]
                    } else {
                        vec![]
                    };
                    for i in positions {
                        if !f[i].is_empty() {
                            names.insert(f[i].to_owned());
                        }
                    }
                }
                let provenance = (
                    records.get(start).copied().unwrap_or(1),
                    records.get(end.saturating_sub(1)).copied().unwrap_or(1),
                );
                captured.push((
                    name.clone(),
                    profile.to_owned(),
                    provenance,
                    text.into_owned(),
                    ending,
                ));
            }
        }
        let mapping: BTreeMap<_, _> = names
            .into_iter()
            .enumerate()
            .map(|(i, n)| {
                let suffix = if n.ends_with("-#") { "-#" } else { "" };
                (n, format!("Q{:06}{suffix}", i + 1))
            })
            .collect();
        for (i, (name, p, (first, last), text, ending)) in captured.into_iter().enumerate() {
            let mut sanitized = replace_roles(&text, &p, &mapping);
            if text.starts_with("CC11^") {
                let original: Vec<_> = text.split('^').collect();
                let mut parts: Vec<String> = sanitized.split('^').map(str::to_owned).collect();
                let positions = if p == "dxspider" && original.len() >= 20 {
                    vec![original.len() - 11]
                } else if p == "cccluster" && original.len() == 25 {
                    vec![8, 9]
                } else {
                    vec![]
                };
                for i in positions {
                    if let Some(mapped) = mapping.get(original[i]) {
                        parts[i] = mapped.clone();
                    }
                }
                sanitized = parts.join("^");
            }
            let mut safe = String::new();
            let mut at = 0;
            for (i, c) in sanitized.char_indices() {
                if c.is_ascii_whitespace() {
                    safe.push_str(&public_token(&sanitized[at..i]));
                    safe.push(c);
                    at = i + c.len_utf8();
                }
            }
            safe.push_str(&public_token(&sanitized[at..]));
            sanitized = safe;
            // Remove all literal IPs and precise CC Cluster slots before publication.
            let mut event = oracle::line(&p, sanitized.as_bytes(), ending, None);
            default_privacy(&mut event, &p, false);
            let redacted = oracle::unb64(event["raw_line"].as_str().unwrap())?;
            if redacted.is_empty() {
                continue;
            }
            sanitized = String::from_utf8(redacted).map_err(|e| e.to_string())?;
            let mut v = vector(
                &format!("captured-{i:04}"),
                &p,
                sanitized.as_bytes(),
                ending,
                None,
            );
            v["provenance"] = json!({"capture":name,"records":(first..=last).collect::<Vec<_>>(),"sanitization":["Deterministic sorted token map to Q-prefix identities, including node tokens","IP literals removed; sensitive CC11 slots emptied before publication","SPDX-License-Identifier: GPL-3.0-or-later"]});
            vectors.push(v);
        }
    }
    write("vectors/events.ndjson", &vectors)?;
    let sessions = crate::scenarios::vectors();
    write("vectors/sessions.ndjson", &sessions)?;
    println!(
        "generated {} event vectors and {} session vectors",
        vectors.len(),
        sessions.len()
    );
    Ok(())
}
fn write(path: &str, vectors: &[Value]) -> Result<(), String> {
    let mut text = String::new();
    for v in vectors {
        text.push_str(&serde_json::to_string(v).map_err(|e| e.to_string())?);
        text.push('\n');
    }
    fs::write(path, text).map_err(|e| e.to_string())
}
fn synthetic() -> Vec<Value> {
    let mut out = extra_vectors();
    for p in ["dxspider", "arcluster", "cccluster", "rbn", "auto"] {
        for (label, line) in [
            ("spot", "DX de Q1ABC: 14025.123 Q2ABC/P CQ 1832Z"),
            ("blank", " "),
            ("unknown", "Unknown text"),
            ("malformed", "DX de Q1ABC: -1 Q2ABC CQ 1832Z"),
            ("overlap", "DX de broken: talk"),
            ("announcement", "To ALL de Q1ABC: Hello"),
            ("wx", "WX de Q1ABC: Rain"),
            ("wwv", "WWV de Q1ABC <23>: SFI=99, A=x, K=3, quiet"),
            (
                "wcy",
                "WCY de Q1ABC <0> : K=3 expK=4 A=1 R=5 SFI=99 SA=q GMF=q Au=no",
            ),
            ("talk", "Q2ABC de Q1ABC: Hi"),
            ("history", "14025 Q1ABC 1-Nov-2026 1832Z text <Q2ABC>"),
        ] {
            out.push(vector(
                &format!("synthetic-{p}-{label}"),
                p,
                line.as_bytes(),
                "lf",
                None,
            ));
        }
    }
    for (i, f) in [
        "0.001",
        "999999999999.999",
        "0",
        "1.",
        "1.0001",
        "-1",
        "+1",
        "1e3",
        "1,000",
        "1000000000000",
    ]
    .iter()
    .enumerate()
    {
        out.push(vector(
            &format!("frequency-{i}"),
            "arcluster",
            format!("DX de Q1ABC: {f} Q2ABC 0000Z").as_bytes(),
            "lf",
            None,
        ));
    }
    for (i, call) in [
        "!",
        "_",
        "123",
        "a",
        "q1abc",
        "Q9XYZ-2-#",
        &"Q".repeat(64),
        &"Q".repeat(65),
    ]
    .iter()
    .enumerate()
    {
        out.push(vector(
            &format!("lexical-{i}"),
            "arcluster",
            format!("DX de {call}: 1 {call} 0000Z").as_bytes(),
            "crlf",
            None,
        ));
    }
    for (i, c) in [
        "CW -12 dB 23 WPM NCDXF B",
        "RTTY 3 dB 45 BPS DX",
        "FT8 -2 dB AA00 CQ",
        "FT4 12 dB 1844 CQ",
        "FT8 0 dB CQ",
        "FT8 32768 dB CQ",
        "CW 1 dB -1 WPM CQ",
        "CW 1 dB 20 BPS CQ",
        "JT65 0 dB CQ",
        "FT8 9999999 dB CQ",
    ]
    .iter()
    .enumerate()
    {
        out.push(vector(
            &format!("rbn-comment-{i}"),
            "rbn",
            format!("DX de Q9XYZ-2-#: 14074 Q2ABC {c} 1832Z").as_bytes(),
            "lf",
            None,
        ));
    }
    for (i, extra) in [
        "   7  ", " AA00 ", " XX ", "", " ", " 100 ", " X ", "\x07", "\x07\x07",
    ]
    .iter()
    .enumerate()
    {
        out.push(vector(
            &format!("spider-suffix-{i}"),
            "dxspider",
            format!("DX de Q1ABC: 1 Q2ABC CQ 0000Z{extra}").as_bytes(),
            "lf",
            None,
        ));
    }
    for ending in ["lf", "crlf", "cr", "none"] {
        let mut v = vector(
            &format!("ending-{ending}"),
            "arcluster",
            b"content",
            ending,
            (ending == "none").then_some("partial"),
        );
        if ending == "cr" {
            v["steps"] = json!([{"type":"input","elapsed_ms":0,"b64":oracle::b64(b"content\r")},{"type":"transport","elapsed_ms":0,"outcome":"eof"},{"type":"tick","elapsed_ms":0}]);
        }
        out.push(v);
    }
    out.push(vector("latin1", "arcluster", b"\xfe\x1b\x07", "lf", None));
    for (p, line) in [
        (
            "dxspider",
            "CC11^1^Q2ABC^1-Nov-2026^1832Z^A^B^Q1ABC^1^2^Q3ABC^4^5^6^7^^^QX^QY^AA00^",
        ),
        (
            "cccluster",
            "CC11^1^Q2ABC^1-Nov-2026^1832^^Q1ABC^^^^^^^^^^^^^^^^^^",
        ),
        ("dxspider", "CC11^1"),
        ("cccluster", "CC11^1"),
    ] {
        out.push(vector(
            &format!("enhanced-{p}-{}", out.len()),
            p,
            line.as_bytes(),
            "lf",
            None,
        ));
    }
    out
}

fn extra_vectors() -> Vec<Value> {
    let mut out = vec![];
    // Manually specified negative oracle: do not ask the spot parser whether
    // its own missing-separator interpretation is correct.
    out.push(vector(
        "manual-missing-colon-space",
        "arcluster",
        b"DX de Q1ABC:14025 Q2ABC CQ 1832Z",
        "lf",
        Some("malformed"),
    ));

    let prompt = "Q1ABC de RELAY 1-Nov-2026 18:32Z >";
    let spot = b"DX de Q1ABC-#: 14074 Q2ABC FT8 -12 dB AA00 CQ 1832Z";
    let mut auto = vector("event-auto-detection-context", "auto", b"", "lf", None);
    let mut bytes = prompt.as_bytes().to_vec();
    bytes.extend_from_slice(spot);
    bytes.push(b'\n');
    auto["steps"][0]["b64"] = json!(oracle::b64(&bytes));
    auto["expected_events"] = json!([
        {"step":0,"elapsed_ms":0,"event":oracle::event("auto","prompt_status",json!({"status":"ready_prompt","text":prompt}),prompt.as_bytes(),"none","exact")},
        {"step":0,"elapsed_ms":0,"event":oracle::line("rbn",spot,"lf",None)},
    ]);
    out.push(auto);

    let ordinary = b"DX de Q1ABC: 1 Q2ABC 0000Z";
    let mut long = vector(
        "line-limit-recovery",
        "arcluster",
        &vec![b'x'; 256],
        "none",
        Some("line_too_long"),
    );
    long["config"]["line_limit"] = json!(256);
    let mut wire = vec![b'x'; 257];
    wire.extend_from_slice(b"DX de Q1ABC: 1 Q2ABC 0000Z\n");
    wire.extend_from_slice(ordinary);
    wire.push(b'\n');
    long["steps"] = json!([{"type":"input","elapsed_ms":0,"b64":oracle::b64(&wire)},{"type":"tick","elapsed_ms":0}]);
    long["expected_events"][0]["step"] = json!(0);
    long["expected_events"].as_array_mut().unwrap().push(
        json!({"step":0,"elapsed_ms":0,"event":oracle::line("arcluster",ordinary,"lf",None)}),
    );
    out.push(long);
    for (i, cmd) in [
        vec![255, 251, 1],
        vec![255, 252, 255],
        vec![255, 253, 1],
        vec![255, 254, 1],
        vec![255, 241],
        vec![255, 250, 1, 255, 255, 240, 255, 240],
    ]
    .iter()
    .enumerate()
    {
        let mut v = vector(
            &format!("telnet-command-{i}"),
            "arcluster",
            b"content",
            "lf",
            None,
        );
        let mut b = cmd.clone();
        b.extend_from_slice(b"content\n");
        v["steps"][0]["b64"] = json!(oracle::b64(&b));
        out.push(v);
    }
    let mut sb = vec![255, 250];
    sb.resize(4094, 0);
    sb.extend([255, 240]);
    sb.extend_from_slice(b"content\n");
    let mut v = vector("telnet-exact-sb", "arcluster", b"content", "lf", None);
    v["steps"][0]["b64"] = json!(oracle::b64(&sb));
    out.push(v);
    let mut v = vector("telnet-data-iac", "arcluster", &[255], "lf", None);
    v["steps"][0]["b64"] = json!(oracle::b64(&[255, 255, b'\n']));
    out.push(v);
    for suffix in [vec![255], vec![255, 251]] {
        let mut v = vector(
            &format!("telnet-incomplete-{}", suffix.len()),
            "arcluster",
            b"content",
            "none",
            Some("partial"),
        );
        let mut b = b"content".to_vec();
        b.extend(suffix);
        v["steps"][0]["b64"] = json!(oracle::b64(&b));
        out.push(v);
    }
    let ip = "192.0.2.1";
    for (i, line) in [
        format!("DX de {ip}: 1 Q2ABC 0000Z"),
        format!("DX de Q1ABC: 1 {ip} 0000Z"),
        format!("{ip} de Q1ABC: hi"),
        format!("Q1ABC de {ip}: hi"),
        format!("To ALL de {ip}: hi"),
        format!("DX de Q1ABC: 1 Q2ABC {ip} 2001:db8::1 0000Z"),
    ]
    .iter()
    .enumerate()
    {
        for retain in [false, true] {
            let mut v = vector(
                &format!("privacy-{i}-{retain}"),
                "dxspider",
                line.as_bytes(),
                "lf",
                None,
            );
            v["config"]["retain_third_party_data"] = json!(retain);
            let mut e = oracle::line("dxspider", line.as_bytes(), "lf", None);
            default_privacy(&mut e, "dxspider", retain);
            v["expected_events"][0]["event"] = e;
            out.push(v);
        }
    }
    let mut slots = vec![""; 25];
    slots[0] = "CC11";
    slots[1] = "1";
    slots[2] = "Q2ABC";
    slots[3] = "29-Feb-2000";
    slots[4] = "1832";
    slots[6] = "Q1ABC";
    slots[8] = "Q3ABC";
    slots[9] = "Q4ABC";
    slots[22] = "12.34/-56.78";
    slots[23] = ip;
    for mutation in [
        "valid",
        "bad-frequency",
        "bad-date",
        "bad-time",
        "ip-call",
        "wrong-count",
    ] {
        for retain in [false, true] {
            let mut s = slots.clone();
            match mutation {
                "bad-frequency" => s[1] = "0",
                "bad-date" => s[3] = "29-Feb-1900",
                "bad-time" => s[4] = "2460",
                "ip-call" => s[2] = ip,
                "wrong-count" => {
                    s.pop();
                }
                _ => {}
            }
            let line = s.join("^");
            let mut v = vector(
                &format!("cc11-{mutation}-{retain}"),
                "cccluster",
                line.as_bytes(),
                "lf",
                None,
            );
            v["config"]["retain_third_party_data"] = json!(retain);
            let mut e = oracle::line("cccluster", line.as_bytes(), "lf", None);
            default_privacy(&mut e, "cccluster", retain);
            v["expected_events"][0]["event"] = e;
            out.push(v);
        }
    }
    for (i, line) in [
        "DX de Q1ABC: 1 Q2ABC 2359Z",
        "DX de Q1ABC: 1 Q2ABC 2400Z",
        "To ALL de : bad",
        "WX de : bad",
        "WWV de Q1ABC <24>: bad",
        "WCY de Q1ABC <> : bad",
        "Capabilities: ve7cc rbn",
        "running DXSpider",
        "Operation successful",
        "Name:",
        "DX de Q1ABC: 1 Q2ABC café 0000Z",
    ]
    .iter()
    .enumerate()
    {
        out.push(vector(
            &format!("dispatch-boundary-{i}"),
            "dxspider",
            line.as_bytes(),
            "lf",
            None,
        ));
    }
    out
}
