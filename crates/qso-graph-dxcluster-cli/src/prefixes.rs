// SPDX-License-Identifier: GPL-3.0-or-later
//! Callsign prefix to location, read from the country file format (`cty.dat`, country-files.com),
//! for the `qg-dxc tui` map (QGDXC-TUI-0083). The text is untrusted: every bound
//! is checked and any error rejects the whole table. Nothing here does I/O.
use std::collections::BTreeMap;

/// Largest accepted table, in bytes.
pub const MAX_BYTES: usize = 1 << 20;
/// Most entities (header lines) in one table.
pub const MAX_ENTITIES: usize = 1_000;
/// Most prefixes and exact callsigns in one table.
pub const MAX_ALIASES: usize = 100_000;
/// Longest prefix or exact callsign, in bytes.
pub const MAX_ALIAS: usize = 32;

/// The country file shipped with this program (see NOTICE.md), parsed once on
/// first use.
pub fn builtin() -> &'static Prefixes {
    static TABLE: std::sync::OnceLock<Prefixes> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        Prefixes::parse(crate::country_file::COUNTRY_FILE).expect("shipped country file is valid")
    })
}

/// A place in degrees: latitude north positive, longitude east positive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    pub lat: f64,
    pub lon: f64,
}

/// A parsed prefix table.
#[derive(Debug, Default)]
pub struct Prefixes {
    prefixes: BTreeMap<String, Place>,
    exact: BTreeMap<String, Place>,
    longest: usize,
}

impl Prefixes {
    /// Parse a country file. Longitudes in the file are positive west; [`Place`] is
    /// positive east.
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > MAX_BYTES {
            return Err(format!("larger than {MAX_BYTES} bytes"));
        }
        let mut table = Prefixes::default();
        let mut entities = 0usize;
        let mut aliases = 0usize;
        let mut rest = text;
        while !rest.trim_start().is_empty() {
            let line = entities + 1;
            // Header: eight colon-terminated fields.
            let mut fields = Vec::with_capacity(8);
            for _ in 0..8 {
                let (field, tail) = rest
                    .split_once(':')
                    .ok_or_else(|| format!("entity {line}: header needs 8 fields"))?;
                if field.contains(';') {
                    return Err(format!("entity {line}: header needs 8 fields"));
                }
                fields.push(field.trim());
                rest = tail;
            }
            entities += 1;
            if entities > MAX_ENTITIES {
                return Err(format!("more than {MAX_ENTITIES} entities"));
            }
            let lat =
                coordinate(fields[4], 90.0).ok_or_else(|| format!("entity {line}: latitude"))?;
            let west =
                coordinate(fields[5], 180.0).ok_or_else(|| format!("entity {line}: longitude"))?;
            let home = Place { lat, lon: -west };
            let (list, tail) = rest
                .split_once(';')
                .ok_or_else(|| format!("entity {line}: prefix list needs ';'"))?;
            rest = tail;
            for token in list.split(',') {
                let token = token.trim();
                if token.is_empty() {
                    continue;
                }
                aliases += 1;
                if aliases > MAX_ALIASES {
                    return Err(format!("more than {MAX_ALIASES} prefixes"));
                }
                let (exact, body) = match token.strip_prefix('=') {
                    Some(body) => (true, body),
                    None => (false, token),
                };
                let end = body.find(['(', '[', '<', '{', '~']).unwrap_or(body.len());
                let (name, marks) = body.split_at(end);
                if name.is_empty()
                    || name.len() > MAX_ALIAS
                    || !name
                        .bytes()
                        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'/')
                {
                    return Err(format!("entity {line}: invalid prefix {:?}", clip(token)));
                }
                let place = overrides(marks, home).ok_or_else(|| {
                    format!("entity {line}: invalid override on {:?}", clip(token))
                })?;
                let map = if exact {
                    &mut table.exact
                } else {
                    table.longest = table.longest.max(name.len());
                    &mut table.prefixes
                };
                map.entry(name.to_owned()).or_insert(place);
            }
        }
        Ok(table)
    }

    /// Entities with at least one prefix or exact callsign.
    pub fn is_empty(&self) -> bool {
        self.prefixes.is_empty() && self.exact.is_empty()
    }

    /// Where a received callsign is, by exact entry or longest matching prefix.
    /// Skimmer (`-#`) and SSID (`-N`) suffixes are ignored; for a portable call
    /// such as `Q9/Q1ABC` the location prefix is the shorter part.
    pub fn lookup(&self, call: &str) -> Option<Place> {
        let call = call.trim().to_ascii_uppercase();
        let call = call.split('-').next().unwrap_or_default();
        if call.is_empty() || call.len() > 64 {
            return None;
        }
        if let Some(place) = self.exact.get(call) {
            return Some(*place);
        }
        let parts: Vec<&str> = call
            .split('/')
            .filter(|p| {
                !p.is_empty()
                    && !matches!(*p, "P" | "M" | "MM" | "AM" | "QRP" | "A" | "R" | "LH")
                    && !(p.len() == 1 && p.as_bytes()[0].is_ascii_digit())
            })
            .collect();
        let base = match parts.as_slice() {
            [] => return None,
            [one] => *one,
            [a, b, ..] => {
                if b.len() < a.len() {
                    b
                } else {
                    a
                }
            }
        };
        if let Some(place) = self.exact.get(base) {
            return Some(*place);
        }
        (1..=base.len().min(self.longest))
            .rev()
            .find_map(|n| base.get(..n).and_then(|p| self.prefixes.get(p)))
            .copied()
    }
}

fn clip(token: &str) -> String {
    token.chars().take(MAX_ALIAS + 8).collect()
}

fn coordinate(text: &str, limit: f64) -> Option<f64> {
    let value: f64 = text.parse().ok()?;
    (value.is_finite() && value.abs() <= limit).then_some(value)
}

/// Apply `(cq)`, `[itu]`, `<lat/lon>`, `{continent}` and `~offset~` markers; only
/// the location override changes the place. Unknown text is an error.
fn overrides(mut marks: &str, mut place: Place) -> Option<Place> {
    while let Some(open) = marks.chars().next() {
        let close = match open {
            '(' => ')',
            '[' => ']',
            '<' => '>',
            '{' => '}',
            '~' => '~',
            _ => return None,
        };
        let inner_end = marks[1..].find(close)? + 1;
        let inner = &marks[1..inner_end];
        if inner.len() > 24 {
            return None;
        }
        if open == '<' {
            let (lat, west) = inner.split_once('/')?;
            place = Place {
                lat: coordinate(lat.trim(), 90.0)?,
                lon: -coordinate(west.trim(), 180.0)?,
            };
        }
        marks = &marks[inner_end + 1..];
    }
    Some(place)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Invented entities with Q prefixes, in the country file format.
    const TABLE: &str = "Qland:  14:  27:  EU:   50.00:   -10.00:    -1.0:  Q1:
    Q1,Q2,=Q1ABC/MM<10.00/20.00>,Q3(4)[5]{EU}~2.0~;
Farqland:  3:  6:  NA:   40.00:   100.00:    5.0:  QF:
    QF,QFX<30.00/90.00>,=Q9ZZZ;
";

    #[test]
    fn country_file_reads_places_with_west_longitudes_flipped() {
        // QGDXC-TUI-0083
        let t = Prefixes::parse(TABLE).unwrap();
        assert!(!t.is_empty());
        let qland = Place {
            lat: 50.0,
            lon: 10.0,
        };
        let farq = Place {
            lat: 40.0,
            lon: -100.0,
        };
        for (call, place) in [
            ("Q1ABC", Some(qland)),
            ("q2xyz", Some(qland)),
            ("Q3DEF", Some(qland)),
            ("QF1A", Some(farq)),
            (
                "QFX9A",
                Some(Place {
                    lat: 30.0,
                    lon: -90.0,
                }),
            ),
            ("Q9ZZZ", Some(farq)),
            ("Q9ZZY", None),
            (
                "Q1ABC/MM",
                Some(Place {
                    lat: 10.0,
                    lon: -20.0,
                }),
            ),
            ("Q1ABC-#", Some(qland)),
            ("Q1ABC-2", Some(qland)),
            ("QF/Q1ABC", Some(farq)),
            ("Q1ABC/QF", Some(farq)),
            ("Q1ABC/P", Some(qland)),
            ("Q1ABC/7", Some(qland)),
            ("QX1ZZ", None),
            ("", None),
            ("/", None),
        ] {
            assert_eq!(t.lookup(call), place, "{call}");
        }
    }

    #[test]
    fn shipped_country_file_parses_and_places_major_prefixes() {
        // QGDXC-TUI-0083
        let t = builtin();
        // Shipped without exact-callsign entries (scripts/strip-cty.sh).
        assert!(!crate::country_file::COUNTRY_FILE.contains('='));
        assert_eq!(crate::country_file::VERSION.len(), 8);
        for (call, lat, lon) in [
            // Bare prefixes, so no callsign appears here.
            ("K1", 37.6, -91.87),
            ("JA1", 36.4, 138.38),
            ("VK2", -23.7, 132.33),
        ] {
            let p = t.lookup(call).unwrap();
            assert!(
                (p.lat - lat).abs() < 0.01 && (p.lon - lon).abs() < 0.01,
                "{call} {p:?}"
            );
        }
    }

    #[test]
    fn country_file_rejects_the_whole_table_on_any_error() {
        // QGDXC-TUI-0083
        for (text, expected) in [
            ("Qland: 1: 2: EU: 50: -10: 0: Q1:\n Q1", "needs ';'"),
            ("Qland: 1: 2: EU: 50: -10: 0:\n Q1;", "8 fields"),
            ("Qland: 1: 2: EU: 91: -10: 0: Q1:\n Q1;", "latitude"),
            ("Qland: 1: 2: EU: 50: -181: 0: Q1:\n Q1;", "longitude"),
            ("Qland: 1: 2: EU: NaN: -10: 0: Q1:\n Q1;", "latitude"),
            (
                "Qland: 1: 2: EU: 50: -10: 0: Q1:\n Q1,q2;",
                "invalid prefix",
            ),
            (
                "Qland: 1: 2: EU: 50: -10: 0: Q1:\n Q1,Q\u{1b}2;",
                "invalid prefix",
            ),
            (
                "Qland: 1: 2: EU: 50: -10: 0: Q1:\n Q1<95/0>;",
                "invalid override",
            ),
            (
                "Qland: 1: 2: EU: 50: -10: 0: Q1:\n Q1(4;",
                "invalid override",
            ),
            ("Qland: 1: 2: EU: 50: -10: 0: Q1:\n Q1#;", "invalid prefix"),
        ] {
            let error = Prefixes::parse(text).unwrap_err();
            assert!(error.contains(expected), "{expected:?} not in {error:?}");
        }
        let long = format!(
            "Qland: 1: 2: EU: 50: -10: 0: Q1:\n Q{};",
            "1".repeat(MAX_ALIAS)
        );
        assert!(
            Prefixes::parse(&long)
                .unwrap_err()
                .contains("invalid prefix")
        );
        let many = "Qland: 1: 2: EU: 50: -10: 0: Q1:\n Q1;\n".repeat(MAX_ENTITIES + 1);
        assert!(Prefixes::parse(&many).unwrap_err().contains("entities"));
        assert!(
            Prefixes::parse(&" ".repeat(MAX_BYTES + 1))
                .unwrap_err()
                .contains("larger")
        );
        assert!(Prefixes::parse("").unwrap().is_empty());
    }
}
