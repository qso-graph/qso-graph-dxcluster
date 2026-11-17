// SPDX-License-Identifier: GPL-3.0-or-later
use std::{net::IpAddr, ops::Range};

// QGDXC-SEC-0005/0006: scan ASCII literal runs at their start and after
// colons/dots, including hexadecimal label suffixes. Each boundary tests at
// most 45 bytes, longest first. Ignore suffixes already covered by a literal.
pub fn visit(s: &str, mut accept: impl FnMut(Range<usize>, IpAddr) -> bool) -> bool {
    let mut offset = 0;
    for run in s.split(|c: char| !(c.is_ascii_hexdigit() || matches!(c, ':' | '.' | '%'))) {
        let mut covered_end = 0;
        for start in 0..run.len() {
            if start != 0 && !matches!(run.as_bytes()[start - 1], b':' | b'.') {
                continue;
            }
            for end in (start + 1..=run.len().min(start + 45)).rev() {
                if end <= covered_end {
                    break;
                }
                if let Ok(ip) = run[start..end].parse::<IpAddr>() {
                    if !accept(offset + start..offset + end, ip) {
                        return false;
                    }
                    covered_end = end;
                    break;
                }
            }
        }
        // Every delimiter is one Unicode scalar; account for its byte width.
        offset += run.len();
        if offset < s.len() {
            offset += s[offset..].chars().next().expect("delimiter").len_utf8();
        }
    }
    true
}

pub fn redact(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut copied_to = 0;
    visit(s, |range, _| {
        if range.start >= copied_to {
            out.push_str(&s[copied_to..range.start]);
            out.push_str("<ip>");
        }
        copied_to = range.end;
        true
    });
    out.push_str(&s[copied_to..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_runtime_scanner_matches_public_scanner_at_boundaries() {
        // QGDXC-SEC-0001 QGDXC-SEC-0005 QGDXC-SEC-0006
        let v4 = [192, 0, 2, 1].map(|n| n.to_string()).join(".");
        let v6 = ["2001", "db8", "", "1"].join(":");
        let mapped = format!("{}:{v4}", ["", "", "ffff"].join(":"));
        let full = ["2001", "db8", "1", "2", "3", "4", "5", "6"].join(":");
        for address in [v4, v6, mapped, full] {
            for before in [
                "",
                "peer:",
                "dead:",
                concat!("std", ":", ":"),
                "peer.",
                "x",
                "é",
                "[",
                "('",
            ] {
                for after in ["", ".", ",", "]:7300", "%zone", ".'),", "/path", ".7", "Z"] {
                    let text = format!("{before}{address}{after}");
                    let expected = redact(&text);
                    let (actual, changed) = qso_graph_dxcluster::privacy::redact(text.as_bytes());
                    assert_eq!(actual, expected.as_bytes(), "{text}");
                    assert!(changed, "{text}");
                    assert!(!expected.contains(&address), "{text}");
                }
            }
        }
    }
}
