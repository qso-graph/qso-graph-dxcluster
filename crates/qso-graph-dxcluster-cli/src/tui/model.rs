// SPDX-License-Identifier: GPL-3.0-or-later
//! Pure display state. No clock, terminal, transport, or file access.
use super::theme::Appearance;
use qso_graph_dxcluster::{Event, Value, privacy};
use std::{
    collections::{BTreeSet, VecDeque},
    time::Duration,
};

pub const DEFAULT_KEEP: usize = 100;
pub const MIN_KEEP: usize = 10;
pub const MAX_KEEP: usize = 10_000;
// QGDXC-TUI-0020: ADIF 3.1.7 Band limits, converted exactly from MHz to Hz.
pub const BANDS: &[(&str, u64, u64)] = &[
    ("2190m", 135700, 137800),
    ("630m", 472000, 479000),
    ("560m", 501000, 504000),
    ("160m", 1800000, 2000000),
    ("80m", 3500000, 4000000),
    ("60m", 5060000, 5450000),
    ("40m", 7000000, 7300000),
    ("30m", 10100000, 10150000),
    ("20m", 14000000, 14350000),
    ("17m", 18068000, 18168000),
    ("15m", 21000000, 21450000),
    ("12m", 24890000, 24990000),
    ("10m", 28000000, 29700000),
    ("8m", 40000000, 45000000),
    ("6m", 50000000, 54000000),
    ("5m", 54000001, 69900000),
    ("4m", 70000000, 71000000),
    ("2m", 144000000, 148000000),
    ("1.25m", 222000000, 225000000),
    ("70cm", 420000000, 450000000),
    ("33cm", 902000000, 928000000),
    ("23cm", 1240000000, 1300000000),
    ("13cm", 2300000000, 2450000000),
    ("9cm", 3300000000, 3500000000),
    ("6cm", 5650000000, 5925000000),
    ("3cm", 10000000000, 10500000000),
    ("1.25cm", 24000000000, 24250000000),
    ("6mm", 47000000000, 47200000000),
    ("4mm", 75500000000, 81000000000),
    ("2.5mm", 119980000000, 123000000000),
    ("2mm", 134000000000, 149000000000),
    ("1mm", 241000000000, 250000000000),
    ("submm", 300000000000, 7500000000000),
];
pub const MODES: &[&str] = &[
    "CW", "SSB", "AM", "FM", "FT8", "FT4", "RTTY", "other", "unknown",
];
/// Every ADIF 3.1.7 Mode and Submode name that is a single word (import-only names
/// excluded). A spot's mode is one of these words in its comment, never its frequency.
const RECOGNIZED: &[&str] = &[
    "8PSK1000",
    "8PSK1000F",
    "8PSK1200F",
    "8PSK125",
    "8PSK125F",
    "8PSK125FL",
    "8PSK250",
    "8PSK250F",
    "8PSK250FL",
    "8PSK500",
    "8PSK500F",
    "AM",
    "AMTORFEC",
    "ARDOP",
    "ASCI",
    "ATV",
    "C4FM",
    "CHIP",
    "CHIP128",
    "CHIP64",
    "CLO",
    "CONTESTI",
    "CW",
    "DIGITALVOICE",
    "DMR",
    "DOM-M",
    "DOM11",
    "DOM16",
    "DOM22",
    "DOM4",
    "DOM44",
    "DOM5",
    "DOM8",
    "DOM88",
    "DOMINO",
    "DOMINOEX",
    "DOMINOF",
    "DSTAR",
    "DYNAMIC",
    "FAX",
    "FM",
    "FMHELL",
    "FREEDATA",
    "FREEDV",
    "FSK",
    "FSK31",
    "FSK441",
    "FSKH105",
    "FSKH245",
    "FSKHELL",
    "FSQCALL",
    "FST4",
    "FST4W",
    "FT2",
    "FT4",
    "FT8",
    "GTOR",
    "HELL",
    "HELL80",
    "HELLX5",
    "HELLX9",
    "HFSK",
    "ISCAT",
    "ISCAT-A",
    "ISCAT-B",
    "JS8",
    "JT4",
    "JT44",
    "JT4A",
    "JT4B",
    "JT4C",
    "JT4D",
    "JT4E",
    "JT4F",
    "JT4G",
    "JT65",
    "JT65A",
    "JT65B",
    "JT65B2",
    "JT65C",
    "JT65C2",
    "JT6M",
    "JT9",
    "JT9-1",
    "JT9-10",
    "JT9-2",
    "JT9-30",
    "JT9-5",
    "JT9A",
    "JT9B",
    "JT9C",
    "JT9D",
    "JT9E",
    "JT9F",
    "JT9G",
    "JT9H",
    "JTMS",
    "LSB",
    "M17",
    "MFSK",
    "MFSK11",
    "MFSK128",
    "MFSK128L",
    "MFSK16",
    "MFSK22",
    "MFSK31",
    "MFSK32",
    "MFSK4",
    "MFSK64",
    "MFSK64L",
    "MFSK8",
    "MSK144",
    "MT63",
    "MTONE",
    "NAVTEX",
    "OFDM",
    "OLIVIA",
    "OPERA",
    "OPERA-BEACON",
    "OPERA-QSO",
    "PAC",
    "PAC2",
    "PAC3",
    "PAC4",
    "PAX",
    "PAX2",
    "PCW",
    "PKT",
    "PSK",
    "PSK10",
    "PSK1000",
    "PSK1000RC2",
    "PSK125",
    "PSK125RC10",
    "PSK125RC12",
    "PSK125RC16",
    "PSK125RC4",
    "PSK125RC5",
    "PSK250",
    "PSK250RC2",
    "PSK250RC3",
    "PSK250RC5",
    "PSK250RC6",
    "PSK250RC7",
    "PSK2K",
    "PSK31",
    "PSK500",
    "PSK500RC2",
    "PSK500RC3",
    "PSK500RC4",
    "PSK63",
    "PSK63F",
    "PSK63RC10",
    "PSK63RC20",
    "PSK63RC32",
    "PSK63RC4",
    "PSK63RC5",
    "PSK800RC2",
    "PSKAM10",
    "PSKAM31",
    "PSKAM50",
    "PSKFEC31",
    "PSKHELL",
    "Q15",
    "Q65",
    "QPSK125",
    "QPSK250",
    "QPSK31",
    "QPSK500",
    "QPSK63",
    "QRA64",
    "QRA64A",
    "QRA64B",
    "QRA64C",
    "QRA64D",
    "QRA64E",
    "RIBBIT_PIX",
    "RIBBIT_SMS",
    "ROS",
    "ROS-EME",
    "ROS-HF",
    "ROS-MF",
    "RTTY",
    "RTTYM",
    "SCAMP_FAST",
    "SCAMP_OO",
    "SCAMP_OO_SLW",
    "SCAMP_SLOW",
    "SCAMP_VSLOW",
    "SIM31",
    "SITORB",
    "SLOWHELL",
    "SSB",
    "SSTV",
    "T10",
    "THOR",
    "THOR-M",
    "THOR100",
    "THOR11",
    "THOR16",
    "THOR22",
    "THOR25X4",
    "THOR4",
    "THOR5",
    "THOR50X1",
    "THOR50X2",
    "THOR8",
    "THRB",
    "THRBX",
    "THRBX1",
    "THRBX2",
    "THRBX4",
    "THROB1",
    "THROB2",
    "THROB4",
    "TOR",
    "USB",
    "V4",
    "VOI",
    "WINMOR",
    "WSPR",
];
pub fn band(hz: u64) -> &'static str {
    BANDS
        .iter()
        .find(|(_, lo, hi)| (*lo..=*hi).contains(&hz))
        .map_or("other", |b| b.0)
}
pub fn mode(event: &Event) -> String {
    let mode = event
        .get("mode")
        .map(str::to_ascii_uppercase)
        .or_else(|| {
            event
                .get("comment")
                .unwrap_or_default()
                .split(' ')
                .find_map(|word| {
                    let upper = word.to_ascii_uppercase();
                    RECOGNIZED.contains(&upper.as_str()).then_some(upper)
                })
        })
        .unwrap_or_else(|| "unknown".into());
    if matches!(mode.as_str(), "USB" | "LSB") {
        "SSB".into()
    } else {
        mode
    }
}
pub fn mode_group(mode: &str) -> &str {
    if MODES.contains(&mode) { mode } else { "other" }
}
pub fn frequency(event: &Event) -> u64 {
    match event.fields.get("frequency_hz") {
        Some(Value::Integer(v)) => u64::try_from(*v).unwrap_or(0),
        _ => 0,
    }
}
pub fn frequency_text(hz: u64) -> String {
    let n = (u128::from(hz) + 50) / 100;
    format!("{}.{:01}", n / 10, n % 10)
}
pub fn sanitize(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if (' '..='~').contains(&c) {
            out.push(c);
        } else {
            use std::fmt::Write;
            let _ = write!(out, "\\u{{{:04x}}}", c as u32);
        }
    }
    out
}
// Accepts sanitized ASCII only; all callers sanitize before measuring.
pub fn clip(text: &str, width: usize, marker: bool) -> String {
    debug_assert!(text.is_ascii());
    if text.len() <= width {
        return text.into();
    }
    if width == 0 {
        return String::new();
    }
    if marker {
        format!("{}~", &text[..width - 1])
    } else {
        text[..width].into()
    }
}

pub fn patterns(entry: &str) -> Result<Vec<String>, String> {
    if entry.len() > 335 {
        return Err("Callsign entry exceeds 335 bytes".into());
    }
    if entry.is_empty() {
        return Ok(vec![]);
    }
    let parts: Vec<_> = entry.split(',').collect();
    if parts.len() > 16
        || parts.iter().any(|p| {
            p.is_empty()
                || p.len() > 20
                || !p
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"/-*?".contains(&b))
        })
    {
        return Err("Use 1–16 patterns of 1–20 ASCII letters, digits, /, -, *, or ?".into());
    }
    Ok(parts.into_iter().map(str::to_ascii_uppercase).collect())
}
// Dynamic programming visits each pattern/call pair once. The operation count
// lets tests measure growth independently of scheduler and CPU clock noise.
pub fn wildcard_work(pattern: &str, call: &str) -> (bool, usize) {
    let mut previous = vec![false; call.len() + 1];
    previous[0] = true;
    let mut work = 0;
    for p in pattern.bytes() {
        let mut next = vec![false; call.len() + 1];
        next[0] = p == b'*' && previous[0];
        for (j, c) in call.bytes().enumerate() {
            work += 1;
            next[j + 1] = if p == b'*' {
                previous[j + 1] || next[j]
            } else {
                previous[j] && (p == b'?' || p.eq_ignore_ascii_case(&c))
            };
        }
        previous = next;
    }
    (previous[call.len()], work)
}
#[derive(Default, Clone)]
pub struct Filters {
    pub bands: Option<BTreeSet<String>>,
    pub modes: Option<BTreeSet<String>>,
    pub calls: Vec<String>,
    pub kind: usize,
}
impl Filters {
    pub fn accepts(&self, spot: &Spot) -> bool {
        self.bands.as_ref().is_none_or(|v| v.contains(spot.band))
            && self
                .modes
                .as_ref()
                .is_none_or(|v| v.contains(mode_group(&spot.mode)))
            && (self.calls.is_empty()
                || self
                    .calls
                    .iter()
                    .any(|p| wildcard_work(p, spot.event.get("call").unwrap_or_default()).0))
            && (self.kind == 0
                || spot.event.get("spot_kind")
                    == Some(if self.kind == 1 { "human" } else { "skimmer" }))
    }
    pub fn summary(&self) -> String {
        let mut parts = vec![];
        if let Some(v) = &self.bands {
            parts.push(format!("band:{}", v.len()));
        }
        if let Some(v) = &self.modes {
            parts.push(format!("mode:{}", v.len()));
        }
        if !self.calls.is_empty() {
            parts.push(format!("call:{}", self.calls.len()));
        }
        if self.kind != 0 {
            parts.push(format!(
                "kind:{}",
                if self.kind == 1 { "human" } else { "skimmer" }
            ));
        }
        format!(
            "Filters: {}",
            if parts.is_empty() {
                "off".into()
            } else {
                parts.join(" ")
            }
        )
    }
}
pub const CHART_BANDS: [&str; 11] = [
    "160m", "80m", "60m", "40m", "30m", "20m", "17m", "15m", "12m", "10m", "6m",
];

// One counter bucket per arrival second, independent of spot retention and filters.
#[derive(Default)]
pub struct Bucket {
    pub second: u64,
    pub sources: [u64; 8],
    pub bands: [u64; 11],
}
pub struct Spot {
    pub born: Duration,
    pub id: u64,
    pub source: usize,
    pub event: Event,
    pub band: &'static str,
    pub mode: String,
}
pub struct Status {
    pub retry_at: Option<Duration>,
    pub label: String,
    pub state: String,
    pub reason: String,
    pub server: String,
    pub count: u64,
    pub last: Option<Duration>,
    pub hidden: bool,
    pub complete: bool,
    pub diagnostic: String,
}
impl Status {
    pub fn line(&self, now: Duration) -> String {
        format!(
            "{} {} {} {} spots:{}{}{}{} {}",
            self.label,
            self.state,
            self.reason,
            self.server,
            self.count,
            self.last
                .map(|at| format!(" age:{}", now.saturating_sub(at).as_secs()))
                .unwrap_or_default(),
            if self.complete { " COMPLETE" } else { "" },
            if self.hidden { " HIDDEN" } else { "" },
            self.diagnostic
        )
    }
}
#[derive(Clone, Debug)]
pub enum Input {
    Char(char),
    Backspace,
    Tab,
    Enter,
    Escape,
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    CtrlC,
}
pub enum Editor {
    Normal,
    Limits {
        values: [String; 2],
        cursor: usize,
        error: String,
    },
    Calls {
        text: String,
        error: String,
        rejected: bool,
    },
    Select {
        band: bool,
        cursor: usize,
        selected: BTreeSet<String>,
    },
    Help,
}
pub struct Model {
    pub appearance: Appearance,
    pub utc_epoch: u64,
    pub activity: VecDeque<Bucket>,
    pub spots: VecDeque<Spot>,
    keep_human: usize,
    keep_skimmer: usize,
    pub kept_human: usize,
    pub kept_skimmer: usize,
    pub sources: Vec<Status>,
    pub total: u64,
    pub filters: Filters,
    pub paused: bool,
    pause_total: u64,
    pub top: Option<u64>,
    pub editor: Editor,
    pub quit: bool,
    pub page: usize,
}
impl Model {
    pub fn new(labels: Vec<String>, keep_human: usize, keep_skimmer: usize) -> Self {
        Self {
            appearance: Appearance::default(),
            utc_epoch: 0,
            activity: VecDeque::new(),
            spots: VecDeque::new(),
            keep_human,
            keep_skimmer,
            kept_human: 0,
            kept_skimmer: 0,
            sources: labels
                .into_iter()
                .map(|label| Status {
                    retry_at: None,
                    label: sanitize(&label),
                    state: String::new(),
                    reason: String::new(),
                    server: "unknown".into(),
                    count: 0,
                    last: None,
                    hidden: false,
                    complete: false,
                    diagnostic: String::new(),
                })
                .collect(),
            total: 0,
            filters: Filters::default(),
            paused: false,
            pause_total: 0,
            top: None,
            editor: Editor::Normal,
            quit: false,
            page: 10,
        }
    }
    pub fn receive(&mut self, source: usize, mut event: Event, now: Duration) {
        // QGDXC-TUI-0014: defensive application of privacy before retention.
        privacy::apply(&mut event, false);
        let status = &mut self.sources[source];
        if event.kind == "connection_state" {
            status.retry_at = match event.fields.get("retry_in_ms") {
                Some(Value::Integer(ms)) if *ms >= 0 => {
                    Some(now.saturating_add(Duration::from_millis(*ms as u64)))
                }
                _ => None,
            };
            status.server = event.server_type.name().into();
            status.state = event.get("state").unwrap_or_default().into();
            status.reason = event.get("reason").unwrap_or_default().into();
        } else if event.kind == "spot" {
            status.count += 1;
            status.last = Some(now);
            self.total += 1;
            while self
                .activity
                .front()
                .is_some_and(|b| now.as_secs().saturating_sub(b.second) >= 900)
            {
                self.activity.pop_front();
            }
            if self
                .activity
                .back()
                .is_none_or(|b| b.second != now.as_secs())
            {
                self.activity.push_back(Bucket {
                    second: now.as_secs(),
                    ..Bucket::default()
                });
            }
            let bucket = self.activity.back_mut().expect("arrival bucket");
            bucket.sources[source] += 1;
            if let Some(index) = CHART_BANDS
                .iter()
                .position(|b| *b == band(frequency(&event)))
            {
                bucket.bands[index] += 1;
            }
            let spot = Spot {
                born: now,
                id: self.total,
                source,
                band: band(frequency(&event)),
                mode: mode(&event),
                event,
            };
            // QGDXC-TUI-0012: unknown kinds share the skimmer allowance.
            let human = spot.event.get("spot_kind") == Some("human");
            let (kept, limit) = if human {
                (&mut self.kept_human, self.keep_human)
            } else {
                (&mut self.kept_skimmer, self.keep_skimmer)
            };
            if *kept == limit {
                let oldest = self
                    .spots
                    .iter()
                    .rposition(|s| (s.event.get("spot_kind") == Some("human")) == human)
                    .expect("retained kind at capacity");
                self.spots.remove(oldest);
            } else {
                *kept += 1;
            }
            self.spots.push_front(spot);
            self.clamp(!self.paused);
        }
    }
    pub fn matching(&self) -> Vec<&Spot> {
        self.spots
            .iter()
            .filter(|s| !self.sources[s.source].hidden && self.filters.accepts(s))
            .collect()
    }
    fn set_limits(&mut self, human: usize, skimmer: usize) {
        self.keep_human = human;
        self.keep_skimmer = skimmer;
        self.kept_human = 0;
        self.kept_skimmer = 0;
        // QGDXC-TUI-0012: retain the newest of each kind, including while paused.
        self.spots.retain(|spot| {
            let (kept, limit) = if spot.event.get("spot_kind") == Some("human") {
                (&mut self.kept_human, human)
            } else {
                (&mut self.kept_skimmer, skimmer)
            };
            if *kept < limit {
                *kept += 1;
                true
            } else {
                false
            }
        });
        self.clamp(false);
    }
    pub fn offset(&self, matching: &[&Spot]) -> usize {
        self.top
            .and_then(|id| matching.iter().position(|s| s.id == id))
            .unwrap_or(0)
    }
    pub fn clamp(&mut self, reset: bool) {
        let accepts = |s: &&Spot| !self.sources[s.source].hidden && self.filters.accepts(s);
        if reset || self.top.is_none() {
            self.top = self.spots.iter().find(accepts).map(|s| s.id);
        } else {
            let retained = self
                .top
                .and_then(|id| self.spots.iter().find(|s| s.id == id));
            if !retained.is_some_and(|s| accepts(&s)) {
                self.top = self.spots.iter().rev().find(accepts).map(|s| s.id);
            }
        }
    }

    pub fn paused_new(&self) -> u64 {
        self.total - self.pause_total
    }
    pub fn summary(&self) -> String {
        format!(
            "retained:{} received:{}{}",
            self.spots.len(),
            self.total,
            if self.paused {
                format!(" PAUSED +{}", self.total - self.pause_total)
            } else {
                String::new()
            }
        )
    }
    pub fn selections(is_band: bool) -> Vec<String> {
        if is_band {
            BANDS
                .iter()
                .map(|b| b.0.to_owned())
                .chain(["other".into()])
                .collect()
        } else {
            MODES.iter().map(|s| (*s).into()).collect()
        }
    }
    pub fn key_at_size(&mut self, input: Input, width: u16, height: u16) {
        if (width < 80 || height < 24) && matches!(input, Input::Char('q')) {
            self.quit = true;
        } else {
            self.key(input);
        }
    }
    pub fn key(&mut self, input: Input) {
        if matches!(input, Input::CtrlC) {
            self.quit = true;
            return;
        }
        match &mut self.editor {
            Editor::Limits {
                values,
                cursor,
                error,
            } => match input {
                Input::Escape => self.editor = Editor::Normal,
                Input::Up => *cursor = cursor.saturating_sub(1),
                Input::Down => *cursor = (*cursor + 1).min(1),
                Input::Tab => *cursor = (*cursor + 1) % 2,
                Input::Backspace => {
                    values[*cursor].pop();
                    error.clear();
                }
                Input::Char(c) if c.is_ascii_digit() && values[*cursor].len() < 5 => {
                    values[*cursor].push(c);
                    error.clear();
                }
                Input::Enter => {
                    let valid = |text: &str| {
                        text.parse::<usize>()
                            .ok()
                            .filter(|n| (MIN_KEEP..=MAX_KEEP).contains(n))
                    };
                    if let (Some(human), Some(skimmer)) = (valid(&values[0]), valid(&values[1])) {
                        self.set_limits(human, skimmer);
                        self.editor = Editor::Normal;
                    } else {
                        *error = "Limits must be 10 to 10,000".into();
                    }
                }
                _ => {}
            },
            Editor::Calls {
                text,
                error,
                rejected,
            } => match input {
                Input::Escape => self.editor = Editor::Normal,
                Input::Enter if *rejected => {}
                Input::Enter => match patterns(text) {
                    Ok(calls) => {
                        self.filters.calls = calls;
                        self.editor = Editor::Normal;
                        self.clamp(true);
                    }
                    Err(e) => *error = e,
                },
                Input::Backspace => {
                    text.pop();
                }
                Input::Char(c) if !c.is_control() => {
                    if text.len() + c.len_utf8() > 335 {
                        *rejected = true;
                        *error = "Callsign edit exceeds 335 bytes; Escape cancels".into();
                    } else {
                        text.push(c);
                        if !*rejected {
                            error.clear();
                        }
                    }
                }
                _ => {}
            },
            Editor::Select {
                band,
                cursor,
                selected,
            } => {
                let options = Self::selections(*band);
                match input {
                    Input::Up => *cursor = cursor.saturating_sub(1),
                    Input::Down => *cursor = (*cursor + 1).min(options.len() - 1),
                    Input::Char(' ') => {
                        let v = &options[*cursor];
                        if !selected.remove(v) {
                            selected.insert(v.clone());
                        }
                    }
                    Input::Escape => self.editor = Editor::Normal,
                    Input::Enter => {
                        if *band {
                            self.filters.bands = Some(selected.clone());
                        } else {
                            self.filters.modes = Some(selected.clone());
                        }
                        self.editor = Editor::Normal;
                        self.clamp(true);
                    }
                    _ => {}
                }
            }
            Editor::Help => {
                if matches!(input, Input::Escape | Input::Char('?') | Input::Enter) {
                    self.editor = Editor::Normal;
                }
            }
            Editor::Normal => match input {
                Input::Char('q') => self.quit = true,
                Input::Char('l') => {
                    self.editor = Editor::Limits {
                        values: [self.keep_human.to_string(), self.keep_skimmer.to_string()],
                        cursor: 0,
                        error: String::new(),
                    };
                }
                Input::Char('t') => self.appearance.theme = self.appearance.theme.next(),
                Input::Char('b' | 'm') => {
                    let is_band = matches!(input, Input::Char('b'));
                    let selected = if is_band {
                        &self.filters.bands
                    } else {
                        &self.filters.modes
                    };
                    self.editor = Editor::Select {
                        band: is_band,
                        cursor: 0,
                        selected: selected.clone().unwrap_or_default(),
                    };
                }
                Input::Char('/') => {
                    self.editor = Editor::Calls {
                        text: self.filters.calls.join(","),
                        error: String::new(),
                        rejected: false,
                    }
                }
                Input::Char('k') => {
                    self.filters.kind = (self.filters.kind + 1) % 3;
                    self.clamp(true);
                }
                Input::Char(c @ '1'..='8') => {
                    if let Some(source) = self.sources.get_mut(c as usize - '1' as usize) {
                        source.hidden = !source.hidden;
                        self.clamp(true);
                    }
                }
                Input::Char('c') => {
                    self.filters = Filters::default();
                    for s in &mut self.sources {
                        s.hidden = false;
                    }
                    self.clamp(true);
                }
                Input::Char('p') => {
                    self.paused = !self.paused;
                    self.pause_total = self.total;
                    if !self.paused {
                        self.clamp(true);
                    }
                }
                Input::Char('?') => self.editor = Editor::Help,
                Input::Up | Input::Down | Input::PageUp | Input::PageDown | Input::Home => {
                    let matching = self.matching();
                    let old = self.offset(&matching);
                    let index = match input {
                        Input::Up => old.saturating_sub(1),
                        Input::Down => old.saturating_add(1),
                        Input::PageUp => old.saturating_sub(self.page),
                        Input::PageDown => old.saturating_add(self.page),
                        _ => 0,
                    }
                    .min(matching.len().saturating_sub(1));
                    self.top = matching.get(index).map(|s| s.id);
                    // Scrolling away from the newest spot holds the view, as `p` does;
                    // Home returns to the newest spot and live updating.
                    if matches!(input, Input::Home) {
                        if self.paused {
                            self.paused = false;
                            self.clamp(true);
                        }
                    } else if index > 0 && !self.paused {
                        self.paused = true;
                        self.pause_total = self.total;
                    }
                }
                _ => {}
            },
        }
    }
}
