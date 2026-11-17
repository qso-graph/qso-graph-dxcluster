// SPDX-License-Identifier: GPL-3.0-or-later
//! Independent specification oracle. This module intentionally imports no
//! implementation crate, including its value, framing, privacy, and JSON code.
use serde_json::{Value, json};
pub fn b64(b: &[u8]) -> String {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for chunk in b.chunks(3) {
        let bits =
            chunk.iter().fold(0u32, |n, b| (n << 8) | u32::from(*b)) << (8 * (3 - chunk.len()));
        for shift in [18, 12, 6, 0] {
            let index = 3 - shift / 6;
            s.push(if index > chunk.len() {
                '='
            } else {
                alphabet[((bits >> shift) & 63) as usize] as char
            });
        }
    }
    s
}
pub fn unb64(s: &str) -> Result<Vec<u8>, String> {
    let alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    if !s.len().is_multiple_of(4) {
        return Err("base64 length".into());
    }
    let mut bits = 0u32;
    let mut count = 0;
    let mut out = vec![];
    for b in s.bytes().take_while(|b| *b != b'=') {
        let n = alphabet
            .bytes()
            .position(|a| a == b)
            .ok_or("base64 alphabet")?;
        bits = (bits << 6) | n as u32;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
        }
    }
    if b64(&out) != s {
        return Err("base64 canonical form".into());
    }
    Ok(out)
}
fn number(s: &str) -> Option<i64> {
    if s.len() > 16 {
        return None;
    }
    let a: Vec<_> = s.split('.').collect();
    if a.len() > 2
        || a[0].is_empty()
        || a[0].len() > 12
        || !a[0].bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let mut n = a[0].parse::<i64>().ok()? * 1000;
    if a.len() == 2 {
        if a[1].is_empty() || a[1].len() > 3 || !a[1].bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let fraction = format!("{:0<3}", a[1]);
        n += fraction.parse::<i64>().ok()?;
    }
    (n > 0 && n < 1_000_000_000_000_000).then_some(n)
}
fn call(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b > 32 && b < 127 && b != b':' && b != b'^')
}
fn clock(s: &str, z: bool) -> Option<String> {
    if z && !s.ends_with('Z') {
        return None;
    }
    let s = s.strip_suffix('Z').unwrap_or(s);
    if s.len() != 4 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let h = s[..2].parse::<u8>().ok()?;
    let m = s[2..].parse::<u8>().ok()?;
    (h < 24 && m < 60).then(|| format!("{h:02}:{m:02}"))
}
fn date(s: &str) -> Option<String> {
    let p: Vec<_> = s.trim().split('-').collect();
    if p.len() != 3 || p[2].len() != 4 {
        return None;
    }
    let month = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ]
    .iter()
    .position(|m| p[1].eq_ignore_ascii_case(m))?
        + 1;
    let year = p[2].parse::<u32>().ok()?;
    let day = p[0].parse::<u32>().ok()?;
    let limit = match month {
        2 => {
            if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    (year > 0 && year <= 9999 && day > 0 && day <= limit)
        .then(|| format!("{year:04}-{month:02}-{day:02}"))
}
fn grid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 4
        && b[..2]
            .iter()
            .all(|b| (b'A'..=b'R').contains(&b.to_ascii_uppercase()))
        && b[2..].iter().all(u8::is_ascii_digit)
}
pub fn event(
    profile: &str,
    kind: &str,
    fields: Value,
    b: &[u8],
    ending: &str,
    status: &str,
) -> Value {
    let encoding = if std::str::from_utf8(b).is_ok() {
        "utf8"
    } else {
        "latin1"
    };
    json!({"spec":"0.1.0","server_type":if profile=="auto"{"unknown"}else{profile},"kind":kind,"fields":fields,"raw_line":b64(b),"raw_ending":ending,"raw_status":status,"encoding":encoding})
}
pub fn local(profile: &str, state: &str, reason: &str, retry: Option<u64>) -> Value {
    let mut e = event(
        profile,
        "connection_state",
        json!({"state":state,"reason":reason}),
        b"",
        "none",
        "synthetic",
    );
    e["encoding"] = json!("none");
    if let Some(ms) = retry {
        e["fields"]["retry_in_ms"] = json!(ms);
    }
    e
}
pub fn line(profile: &str, b: &[u8], ending: &str, reason: Option<&str>) -> Value {
    let text = std::str::from_utf8(b)
        .map(str::to_owned)
        .unwrap_or_else(|_| b.iter().map(|b| char::from(*b)).collect());
    let s = text.as_str();
    let status = if reason == Some("line_too_long") {
        "truncated"
    } else {
        "exact"
    };
    let bad = |reason: &str| {
        event(
            profile,
            "unrecognized_line",
            json!({"reason":reason,"text":s}),
            b,
            ending,
            status,
        )
    };
    if let Some(reason) = reason {
        return bad(reason);
    }
    if s.bytes().all(|b| b == b' ') {
        return bad("blank");
    }
    if profile == "auto" && (s.starts_with("DX de ") || s.starts_with("CC11^")) {
        return bad("before_identification");
    }
    if s.starts_with("DX de ") {
        let result = (|| {
            let (spotter, tail) = s[6..].trim_start_matches(' ').split_once(':')?;
            if !tail.starts_with(' ') {
                return None;
            }
            let mut parts = tail
                .trim_start_matches(' ')
                .splitn(3, ' ')
                .filter(|p| !p.is_empty());
            let f = parts.next()?;
            let rest = &tail.trim_start_matches(' ')[f.len()..].trim_start_matches(' ');
            let at = rest.find(' ')?;
            let dx = &rest[..at];
            let content = rest[at..].trim_start_matches(' ');
            let content = if profile == "dxspider" {
                content.strip_suffix("\x07\x07").unwrap_or(content)
            } else {
                content
            };
            if !call(spotter) || !call(dx) {
                return None;
            }
            let hz = number(f)?;
            let mut candidates = vec![];
            for (i, _) in content.char_indices() {
                if i > 0 && content.as_bytes()[i - 1] != b' ' {
                    continue;
                }
                if let Some(time) = content.get(i..i + 5).and_then(|s| clock(s, true)) {
                    let suffix = &content[i + 5..];
                    let x = suffix.trim_matches(' ');
                    if x.is_empty()
                        || (profile == "dxspider"
                            && suffix.starts_with(' ')
                            && (grid(x)
                                || (x.len() == 2 && x.bytes().all(|b| b.is_ascii_alphabetic()))
                                || (!x.is_empty()
                                    && x.len() <= 2
                                    && x.bytes().all(|b| b.is_ascii_digit()))))
                    {
                        candidates.push((i, time, suffix));
                    }
                }
            }
            if candidates.len() != 1 {
                return None;
            }
            let (i, time, suffix) = candidates.remove(0);
            let comment = content[..i].trim_matches(' ');
            let mut fields = spot(profile, "dx_de", spotter, dx, hz, &time, comment);
            if !suffix.trim_matches(' ').is_empty() {
                fields["native"] = json!({"trailing_extra":suffix});
            }
            if profile == "rbn" {
                structured(comment, &mut fields);
            }
            Some(fields)
        })();
        return result.map_or_else(
            || bad("malformed"),
            |fields| event(profile, "spot", fields, b, ending, status),
        );
    }
    if s.starts_with("CC11^") {
        if !["dxspider", "cccluster"].contains(&profile) {
            return bad("unsupported_layout");
        }
        let f: Vec<_> = s.split('^').collect();
        if (profile == "cccluster" && f.len() != 25) || (profile == "dxspider" && f.len() < 20) {
            return bad("unsupported_layout");
        }
        let result = (|| {
            let (comment, tail) = if profile == "dxspider" {
                (f[5..f.len() - 14].join("^"), f[f.len() - 14..].to_vec())
            } else {
                (f[5].to_owned(), f[6..].to_vec())
            };
            if !call(f[2]) || !call(tail[0]) {
                return None;
            }
            let hz = number(f[1])?;
            let day = date(f[3])?;
            let time = clock(f[4], profile == "dxspider")?;
            let mut fields = spot(
                profile,
                if profile == "dxspider" {
                    "cc11_dxspider"
                } else {
                    "cc11_cccluster"
                },
                tail[0],
                f[2],
                hz,
                &time,
                &comment,
            );
            fields["date"] = json!(day);
            let mut native = json!({});
            if profile == "dxspider" {
                for (k, val) in [
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
                ]
                .iter()
                .zip(&tail[1..])
                {
                    native[k] = json!(val);
                }
                if grid(tail[12]) {
                    fields["grid"] = json!(tail[12]);
                }
            } else {
                for (i, val) in f.iter().enumerate().skip(7) {
                    native[if i == 23 {
                        "spotter_ip".to_owned()
                    } else {
                        format!("slot_{i:02}")
                    }] = json!(val);
                }
            }
            fields["native"] = native;
            Some(fields)
        })();
        return result.map_or_else(
            || bad("malformed"),
            |fields| event(profile, "spot", fields, b, ending, status),
        );
    }
    if ["To ALL de ", "WX de ", "WWV de ", "WCY de "]
        .iter()
        .any(|p| s.starts_with(p))
        || profile == "dxspider" && s.contains(" de ")
    {
        if profile != "dxspider" && !(profile == "cccluster" && s.starts_with("To ALL de ")) {
            return bad("unsupported_layout");
        }
        let clean = s.strip_suffix("\x07\x07").unwrap_or(s);
        if let Some((kind, fields)) = other(clean, profile) {
            return event(profile, kind, fields, b, ending, status);
        }
        return bad("malformed");
    }
    let p: Vec<_> = s.split_ascii_whitespace().collect();
    if profile == "arcluster"
        && p.len() >= 5
        && number(p[0]).is_some()
        && date(p[2]).is_some()
        && clock(p[3], true).is_some()
        && p.last()
            .is_some_and(|p| p.starts_with('<') && p.ends_with('>'))
    {
        return bad("unsupported_layout");
    }
    bad("unknown")
}
fn spot(p: &str, format: &str, by: &str, dx: &str, hz: i64, t: &str, comment: &str) -> Value {
    json!({"spot_kind":if p=="rbn"||by.ends_with("-#"){"skimmer"}else{"human"},"source":if p=="rbn"{"rbn"}else{"cluster"},"format":format,"spotter_call":by,"call":dx,"frequency_hz":hz,"time_of_day":t,"comment":comment})
}
fn structured(comment: &str, f: &mut Value) {
    let p: Vec<_> = comment.split_ascii_whitespace().collect();
    if p.len() < 4
        || !["CW", "RTTY", "FT8", "FT4"].contains(&p[0])
        || p[2] != "dB"
        || p[1].len() > 6
    {
        return;
    }
    let Ok(n) = p[1].parse::<i16>() else {
        return;
    };
    let end = if p.ends_with(&["NCDXF", "B"]) {
        p.len() - 2
    } else {
        p.len() - 1
    };
    let ty = p[end..].join(" ");
    if !["CQ", "DX", "BEACON", "NCDXF B"].contains(&ty.as_str()) {
        return;
    }
    if p[0] == "CW" || p[0] == "RTTY" {
        if end != 5
            || p[3].len() > 6
            || !p[3].bytes().all(|b| b.is_ascii_digit())
            || p[4] != if p[0] == "CW" { "WPM" } else { "BPS" }
        {
            return;
        }
        let Ok(speed) = p[3].parse::<u16>() else {
            return;
        };
        f["speed"] = json!(speed);
        f["speed_unit"] = json!(if p[0] == "CW" { "wpm" } else { "bps" });
    } else {
        if !(3..=4).contains(&end) {
            return;
        }
        if end == 4 {
            if grid(p[3]) {
                f["grid"] = json!(p[3]);
            } else {
                f["native"] = json!({"rbn_slot":p[3]});
            }
        }
    }
    f["mode"] = json!(p[0]);
    f["snr_db"] = json!(n);
    f["spot_type"] = json!(ty);
}
fn other<'a>(s: &str, p: &str) -> Option<(&'a str, Value)> {
    if p == "cccluster" {
        let s = s.strip_prefix("To ALL de ")?;
        let (h, text) = s.split_once(':')?;
        let h: Vec<_> = h.split_ascii_whitespace().collect();
        if h.len() != 2 || !call(h[0]) {
            return None;
        }
        let via = h[1].strip_prefix('<')?.strip_suffix('>')?;
        if !call(via) {
            return None;
        }
        return Some((
            "announcement",
            json!({"from_call":h[0],"via_call":via,"target":"ALL","category":"all","text":text.trim_matches(' ')}),
        ));
    }
    if s.starts_with("WWV de ") || s.starts_with("WCY de ") {
        let wwv = s.starts_with("WWV");
        let (h, text) = s[7..].split_once(':')?;
        let h: Vec<_> = h.split_ascii_whitespace().collect();
        if h.len() != 2 || !call(h[0]) {
            return None;
        }
        let hour = h[1]
            .strip_prefix('<')?
            .strip_suffix('>')?
            .parse::<u8>()
            .ok()?;
        if hour > 23 {
            return None;
        }
        let mut fields = json!({"from_call":h[0],"hour":hour,"text":text.trim_matches(' ')});
        for part in text.split_ascii_whitespace() {
            if let Some((k, v)) = part.trim_end_matches(',').split_once('=') {
                let key = match k {
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
                if ["sa", "gmf", "au"].contains(&key) {
                    fields[key] = json!(v);
                } else if let Ok(n) = v.parse::<i64>()
                    && n.unsigned_abs() <= 9007199254740991
                {
                    fields[key] = json!(n);
                }
            }
        }
        let parts: Vec<_> = text.splitn(4, ',').collect();
        if wwv && parts.len() == 4 {
            fields["forecast"] = json!(parts[3].trim_matches(' '));
        }
        return Some((if wwv { "wwv" } else { "wcy" }, fields));
    }
    let (target, category, rest) = if let Some(r) = s.strip_prefix("To ALL de ") {
        (Some("ALL"), "all", r)
    } else if let Some(r) = s.strip_prefix("WX de ") {
        (Some("WX"), "weather", r)
    } else {
        (None, "", s)
    };
    let (h, text) = rest.split_once(':')?;
    let mut f = json!({"text":text.trim_matches(' ')});
    if let Some(t) = target {
        let by = h.trim_matches(' ');
        if !call(by) {
            return None;
        }
        f["from_call"] = json!(by);
        f["target"] = json!(t);
        f["category"] = json!(category);
        Some(("announcement", f))
    } else {
        let h: Vec<_> = h.split_ascii_whitespace().collect();
        if h.len() != 3 || h[1] != "de" || !call(h[0]) || !call(h[2]) {
            return None;
        }
        f["to_call"] = json!(h[0]);
        f["from_call"] = json!(h[2]);
        Some(("talk", f))
    }
}
