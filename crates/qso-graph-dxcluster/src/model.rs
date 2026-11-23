// SPDX-License-Identifier: GPL-3.0-or-later
use std::collections::BTreeMap;

/// Exact Annex A values; no floating-point protocol numbers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Text(String),
    Integer(i64),
    Object(BTreeMap<String, Value>),
}
impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self::Text(s.to_owned())
    }
}
impl From<String> for Value {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}
impl Value {
    pub fn text(&self) -> Option<&str> {
        if let Self::Text(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn json(&self) -> String {
        match self {
            Self::Text(s) => quote(s),
            Self::Integer(n) => n.to_string(),
            Self::Object(o) => format!(
                "{{{}}}",
                o.iter()
                    .map(|(k, v)| format!("{}:{}", quote(k), v.json()))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}
pub fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Auto,
    DxSpider,
    ArCluster,
    CcCluster,
    Rbn,
    /// A synthetic POTA event source (QGDXC-TUI-0130), outside cluster sessions.
    Pota,
}
impl Profile {
    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "unknown",
            Self::DxSpider => "dxspider",
            Self::ArCluster => "arcluster",
            Self::CcCluster => "cccluster",
            Self::Rbn => "rbn",
            Self::Pota => "pota",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "auto" | "unknown" => Self::Auto,
            "dxspider" => Self::DxSpider,
            "arcluster" => Self::ArCluster,
            "cccluster" => Self::CcCluster,
            "rbn" => Self::Rbn,
            _ => return None,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub server_type: Profile,
    pub kind: &'static str,
    pub fields: BTreeMap<String, Value>,
    pub raw_line: Vec<u8>,
    pub raw_ending: &'static str,
    pub raw_status: &'static str,
    pub encoding: &'static str,
    pub received_at: Option<String>,
}
impl Event {
    pub fn new(profile: Profile, kind: &'static str) -> Self {
        Self {
            server_type: profile,
            kind,
            fields: BTreeMap::new(),
            raw_line: Vec::new(),
            raw_ending: "none",
            raw_status: "synthetic",
            encoding: "none",
            received_at: None,
        }
    }
    pub fn set(&mut self, key: &str, value: impl Into<Value>) {
        self.fields.insert(key.to_owned(), value.into());
    }
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.get(key).and_then(Value::text)
    }
    pub fn json(&self) -> String {
        let mut obj = BTreeMap::from([
            ("spec".into(), Value::from("0.1.0")),
            ("server_type".into(), Value::from(self.server_type.name())),
            ("kind".into(), Value::from(self.kind)),
            ("fields".into(), Value::Object(self.fields.clone())),
            ("raw_line".into(), Value::from(base64(&self.raw_line))),
            ("raw_ending".into(), Value::from(self.raw_ending)),
            ("raw_status".into(), Value::from(self.raw_status)),
            ("encoding".into(), Value::from(self.encoding)),
        ]);
        if let Some(s) = &self.received_at {
            obj.insert("received_at".into(), Value::from(s.clone()));
        }
        Value::Object(obj).json()
    }
}
const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
pub fn base64(bytes: &[u8]) -> String {
    let mut s = String::new();
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        s.push(B64[((n >> 18) & 63) as usize] as char);
        s.push(B64[((n >> 12) & 63) as usize] as char);
        s.push(if c.len() > 1 {
            B64[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        s.push(if c.len() > 2 {
            B64[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    s
}
pub fn unbase64(s: &str) -> Result<Vec<u8>, &'static str> {
    if !s.len().is_multiple_of(4) {
        return Err("invalid base64 length");
    }
    let mut out = Vec::new();
    for c in s.as_bytes().chunks(4) {
        let mut n = 0u32;
        for b in c {
            n = (n << 6)
                | if *b == b'=' {
                    0
                } else {
                    B64.iter().position(|x| x == b).ok_or("invalid base64")? as u32
                };
        }
        out.push((n >> 16) as u8);
        if c[2] != b'=' {
            out.push((n >> 8) as u8);
        }
        if c[3] != b'=' {
            out.push(n as u8);
        }
    }
    if base64(&out) != s {
        return Err("noncanonical base64");
    }
    Ok(out)
}
/// Terminal escaping is deliberately separate from stored event text.
pub fn escape(s: &str) -> String {
    s.chars().map(|c| if c.is_control() || matches!(c,'\u{061c}'|'\u{200b}'..='\u{200f}'|'\u{2028}'..='\u{202e}'|'\u{2060}'..='\u{206f}'|'\u{feff}') {format!("\\u{{{:04x}}}",c as u32)} else {c.to_string()}).collect()
}
