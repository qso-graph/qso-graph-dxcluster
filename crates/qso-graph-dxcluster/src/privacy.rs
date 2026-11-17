// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{Event, Profile, Value};

fn ipv4(s: &str) -> bool {
    let p: Vec<_> = s.split('.').collect();
    p.len() == 4
        && p.iter().all(|p| {
            !p.is_empty()
                && p.len() <= 3
                && p.bytes().all(|b| b.is_ascii_digit())
                && p.parse::<u16>().is_ok_and(|v| v <= 255)
        })
}
/// Literal recognition uses lexical IP syntax without invoking a network API.
pub fn is_ip(s: &str) -> bool {
    let s = s.trim_matches(['[', ']']);
    if ipv4(s) {
        return true;
    }
    if !s.contains(':') || s.matches("::").count() > 1 {
        return false;
    }
    let p: Vec<_> = s.split(':').collect();
    let mut units = 0;
    for (i, p) in p.iter().enumerate() {
        if p.is_empty() {
            continue;
        }
        if p.contains('.') {
            if i + 1 != s.split(':').count() {
                return false;
            }
            if !ipv4(p) {
                return false;
            }
            units += 2;
        } else {
            if p.len() > 4 || !p.bytes().all(|b| b.is_ascii_hexdigit()) {
                return false;
            }
            units += 1;
        }
    }
    if s.contains("::") {
        units < 8 && !s.contains(":::")
    } else {
        units == 8 && !s.starts_with(':') && !s.ends_with(':')
    }
}
/// Return only redacted data. The original never enters an event queue.
pub fn redact(bytes: &[u8]) -> (Vec<u8>, bool) {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let mut changed = false;
    while i < bytes.len() {
        if (bytes[i].is_ascii_hexdigit() || bytes[i] == b':')
            && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
        {
            let mut end = i;
            while end < bytes.len()
                && (bytes[end].is_ascii_hexdigit() || matches!(bytes[end], b'.' | b':'))
            {
                end += 1;
            }
            let mut found = None;
            for e in (i + 1..=end).rev() {
                if e < bytes.len() && bytes[e].is_ascii_alphanumeric() {
                    continue;
                }
                if std::str::from_utf8(&bytes[i..e]).is_ok_and(is_ip) {
                    found = Some(e);
                    break;
                }
            }
            if let Some(e) = found {
                out.extend_from_slice(b"<ip>");
                changed = true;
                i = e;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    (out, changed)
}
fn value(v: &mut Value) -> bool {
    match v {
        Value::Text(s) => {
            let (b, c) = redact(s.as_bytes());
            if c {
                *s = String::from_utf8(b).unwrap_or_default();
            }
            c
        }
        Value::Object(o) => {
            let mut changed = false;
            for v in o.values_mut() {
                changed |= value(v);
            }
            changed
        }
        _ => false,
    }
}
pub fn apply(e: &mut Event, retain: bool) {
    if retain {
        return;
    }
    if ["spotter_call", "call", "from_call", "to_call"]
        .iter()
        .any(|k| e.get(k).is_some_and(is_ip))
    {
        withhold(e);
        return;
    }
    let mut changed = false;
    if e.raw_line.starts_with(b"CC11^")
        && matches!(e.server_type, Profile::CcCluster | Profile::Auto)
    {
        let mut slots: Vec<Vec<u8>> = e
            .raw_line
            .split(|b| *b == b'^')
            .map(<[u8]>::to_vec)
            .collect();
        if slots.len() != 25 {
            withhold(e);
            return;
        }
        if !slots[22].is_empty() || !slots[23].is_empty() {
            slots[22].clear();
            slots[23].clear();
            e.raw_line = slots.join(&b'^');
            changed = true;
        }
        if let Some(Value::Object(n)) = e.fields.get_mut("native") {
            n.remove("slot_22");
            n.remove("spotter_ip");
        }
        // Unknown/malformed text includes the original CC11 slots.
        if e.fields.contains_key("text") {
            e.set("text", crate::values::decode(&e.raw_line).0);
        }
    }
    let (raw, c) = redact(&e.raw_line);
    e.raw_line = raw;
    changed |= c;
    for v in e.fields.values_mut() {
        changed |= value(v);
    }
    if changed {
        e.raw_status = "redacted";
    }
}
pub fn withhold(e: &mut Event) {
    e.kind = "unrecognized_line";
    e.fields.clear();
    e.set("reason", "privacy_withheld");
    e.set("text", "");
    e.raw_line.clear();
    e.raw_status = "withheld";
}
