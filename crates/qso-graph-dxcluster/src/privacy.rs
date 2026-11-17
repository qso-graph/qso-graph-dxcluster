// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{Event, Value};

fn ipv4(s: &str) -> bool {
    let mut parts = s.split('.');
    for _ in 0..4 {
        let Some(p) = parts.next() else {
            return false;
        };
        if p.is_empty()
            || p.len() > 3
            || (p.len() > 1 && p.starts_with('0'))
            || !p.bytes().all(|b| b.is_ascii_digit())
            || p.parse::<u8>().is_err()
        {
            return false;
        }
    }
    parts.next().is_none()
}
/// Literal recognition uses lexical IP syntax without invoking a network API.
pub fn is_ip(s: &str) -> bool {
    let s = s.trim_matches(['[', ']']);
    if s.len() > 45 {
        return false;
    }
    if !s.contains(':') {
        return ipv4(s);
    }
    if s.contains(":::") || s.matches("::").count() > 1 {
        return false;
    }
    if s.strip_prefix(':')
        .is_some_and(|rest| !rest.starts_with(':'))
        || s.strip_suffix(':').is_some_and(|rest| !rest.ends_with(':'))
    {
        return false;
    }
    let mut parts = s.split(':').peekable();
    let mut units = 0;
    while let Some(p) = parts.next() {
        if p.is_empty() {
            continue;
        }
        if p.contains('.') {
            if parts.peek().is_some() {
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
        units < 8
    } else {
        units == 8 && !s.starts_with(':') && !s.ends_with(':')
    }
}
/// Return only redacted data. The original never enters an event queue.
pub fn redact(bytes: &[u8]) -> (Vec<u8>, bool) {
    let mut out = Vec::with_capacity(bytes.len());
    let mut offset = 0;
    let mut copied_to = 0;
    // QGDXC-SEC-0005: the same bounded boundary extraction as the public
    // sanitization gate, with independent, dependency-free recognition.
    for run in bytes.split(|b| !(b.is_ascii_hexdigit() || matches!(b, b':' | b'.' | b'%'))) {
        for start in 0..run.len() {
            if start != 0 && !matches!(run[start - 1], b':' | b'.') {
                continue;
            }
            // An IPv6 literal with a dotted tail has at most 45 bytes.
            for end in (start + 1..=run.len().min(start + 45)).rev() {
                if offset + end <= copied_to {
                    break;
                }
                if std::str::from_utf8(&run[start..end]).is_ok_and(is_ip) {
                    if offset + start >= copied_to {
                        out.extend_from_slice(&bytes[copied_to..offset + start]);
                        out.extend_from_slice(b"<ip>");
                    }
                    copied_to = offset + end;
                    break;
                }
            }
        }
        offset += run.len() + 1;
    }
    out.extend_from_slice(&bytes[copied_to..]);
    (out, copied_to != 0)
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
    if e.raw_line.windows(5).any(|w| w == b"CC11^") {
        if !e.raw_line.trim_ascii_start().starts_with(b"CC11^") {
            withhold(e);
            return;
        }
        let mut slots: Vec<Vec<u8>> = e
            .raw_line
            .split(|b| *b == b'^')
            .map(<[u8]>::to_vec)
            .collect();
        // A variable-length DXSpider comment can overlap an unknown CC
        // layout. Without retention, only fixed boundaries are safe.
        if slots.len() == 20 {
            // Fixed DXSpider layout: no designated coordinate slots.
        } else if slots.len() != 25 || e.get("format") == Some("cc11_dxspider") {
            withhold(e);
            return;
        }
        if slots.len() == 25 && (!slots[22].is_empty() || !slots[23].is_empty()) {
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
