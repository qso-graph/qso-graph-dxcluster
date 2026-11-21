// SPDX-License-Identifier: GPL-3.0-or-later
use super::{
    model::{CHART_BANDS, Editor, Model, Spot, clip, frequency, frequency_text, sanitize},
    theme::{Appearance, Role},
};
use qso_graph_dxcluster::Value;
use ratatui::style::{Modifier, Style};
use ratatui::{Frame, layout::Rect, widgets::Paragraph};
use std::time::Duration;

pub fn field(frame: &mut Frame<'_>, rect: Rect, text: &str, marker: bool) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    let text = clip(&sanitize(text), rect.width as usize, marker);
    frame.render_widget(Paragraph::new(text), rect);
}
fn line(frame: &mut Frame<'_>, area: Rect, y: u16, text: &str) {
    field(frame, Rect::new(area.x, y, area.width, 1), text, false);
}
pub fn draw(frame: &mut Frame<'_>, model: &Model, now: Duration) {
    let area = frame.area();
    if area.width < 80 || area.height < 24 {
        if area.height > 0 {
            line(
                frame,
                area,
                area.y,
                "Enlarge window to at least 80 columns and 24 rows. q: quit",
            );
        }
        return;
    }
    for cell in &mut frame.buffer_mut().content {
        cell.set_style(model.appearance.style(Role::Text));
    }
    match &model.editor {
        Editor::Limits {
            values,
            cursor,
            error,
        } => {
            line(
                frame,
                area,
                0,
                "Limits: Enter commits, Esc cancels, Ctrl-C quits",
            );
            line(
                frame,
                area,
                1,
                "Up/Down/Tab moves; digits and Backspace edit",
            );
            for (index, label) in ["Human", "Skimmer"].iter().enumerate() {
                line(
                    frame,
                    area,
                    3 + index as u16,
                    &format!(
                        "{} {label}: {}",
                        if index == *cursor { ">" } else { " " },
                        values[index]
                    ),
                );
            }
            line(frame, area, area.height - 2, error);
            return;
        }
        Editor::Calls { text, error, .. } => {
            line(
                frame,
                area,
                0,
                "Callsign filter: Enter commits, Esc cancels, Ctrl-C quits",
            );
            let safe = sanitize(text);
            let rows = area.height as usize - 5;
            let capacity = rows * area.width as usize;
            let start = safe.len().saturating_sub(capacity);
            for (index, chunk) in safe.as_bytes()[start..]
                .chunks(area.width as usize)
                .enumerate()
            {
                // Sanitization guarantees ASCII, including for hostile pastes.
                let text = std::str::from_utf8(chunk).expect("sanitized ASCII");
                line(frame, area, index as u16 + 2, text);
            }
            line(frame, area, area.height - 2, error);
            return;
        }
        Editor::Select {
            band,
            cursor,
            selected,
        } => {
            line(
                frame,
                area,
                0,
                if *band {
                    "Band filter: Up/Down, Space toggles, Enter commits, Esc cancels"
                } else {
                    "Mode filter: Up/Down, Space toggles, Enter commits, Esc cancels"
                },
            );
            let options = Model::selections(*band);
            let all = match selected.len() {
                0 => " ",
                n if n == options.len() => "x",
                _ => "-",
            };
            let rows = area.height as usize - 2;
            let start = cursor.saturating_sub(rows - 1);
            let entries = std::iter::once(("All", all)).chain(
                options
                    .iter()
                    .map(|v| (v.as_str(), if selected.contains(v) { "x" } else { " " })),
            );
            for (index, (value, mark)) in entries.enumerate().skip(start).take(rows) {
                line(
                    frame,
                    area,
                    (index - start + 2) as u16,
                    &format!(
                        "{} [{mark}] {value}",
                        if index == *cursor { ">" } else { " " },
                    ),
                );
            }
            return;
        }
        Editor::Help => {
            for (y, text) in [
                "QG DX Cluster: receive-only multi-source spot viewer",
                "q / Ctrl-C: quit   b: bands   m: modes   /: callsign patterns",
                "k: all/human/skimmer   s: hide self-spots   1-8: hide/show source   c: clear",
                "p: pause/resume scrolling (reception continues)   t: theme   M: map   d: dupes",
                "Up/Down/PageUp/PageDown/Home: scroll   l limits   ?: help",
                "Callsign editor: printable keys are data; Backspace deletes",
                "Enter commits; Escape cancels. Wildcards: * any run, ? one character",
                "Band/mode: Up/Down moves, Space toggles (All: every value), Enter, Esc cancels",
                "Limits: Up/Down/Tab moves, digits/Backspace edit, Enter commits, Esc cancels",
                "Esc / Enter / ?: return   Ctrl-C quits from every screen",
            ]
            .iter()
            .enumerate()
            {
                line(frame, area, y as u16, text);
            }
            for (i, source) in model.sources.iter().enumerate() {
                line(frame, area, 10 + i as u16, &source.line(now));
            }
            line(frame, area, 19, &model.summary());
            return;
        }
        Editor::Normal => {}
    }
    dashboard(frame, model, now);
}

const SUP: [&str; 8] = ["¹", "²", "³", "⁴", "⁵", "⁶", "⁷", "⁸"];
const BLOCK: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
const BAND_ROLES: [Role; 11] = [
    Role::B160,
    Role::B80,
    Role::B60,
    Role::B40,
    Role::B30,
    Role::B20,
    Role::B17,
    Role::B15,
    Role::B12,
    Role::B10,
    Role::B6,
];

// Only this writer accepts trusted UI glyphs. All external values use safe().
struct Row<'a, 'b> {
    frame: &'a mut Frame<'b>,
    x: u16,
    y: u16,
    end: u16,
}
impl<'a, 'b> Row<'a, 'b> {
    fn new(frame: &'a mut Frame<'b>, x: u16, y: u16, width: u16) -> Self {
        Self {
            frame,
            x,
            y,
            end: x.saturating_add(width),
        }
    }
    fn ui(&mut self, text: &str, style: Style) {
        for ch in text.chars() {
            if self.x >= self.end {
                break;
            }
            if let Some(cell) = self.frame.buffer_mut().cell_mut((self.x, self.y)) {
                cell.set_char(ch).set_style(style);
            }
            self.x += 1;
        }
    }
    fn safe(&mut self, text: &str, width: usize, right: bool, style: Style, marker: bool) {
        let value = clip(&sanitize(text), width, marker);
        let value = if right {
            format!("{value:>width$}")
        } else {
            format!("{value:<width$}")
        };
        self.ui(&value, style);
    }
}
fn put(frame: &mut Frame<'_>, x: u16, y: u16, width: u16, text: &str, style: Style) {
    Row::new(frame, x, y, width).ui(text, style);
}
fn border(
    frame: &mut Frame<'_>,
    position: (u16, u16, u16),
    top: bool,
    left: &str,
    right: &str,
    a: Appearance,
) {
    let (x, y, width) = position;
    let (l, r) = if top { ("╭", "╮") } else { ("╰", "╯") };
    put(
        frame,
        x,
        y,
        width,
        &format!("{l}{}{r}", "─".repeat(width.saturating_sub(2) as usize)),
        a.style(Role::Border),
    );
    put(
        frame,
        x + 2,
        y,
        width.saturating_sub(4),
        left,
        a.style(Role::Title).add_modifier(Modifier::BOLD),
    );
    let len = right.chars().count().min(width.saturating_sub(4) as usize) as u16;
    put(
        frame,
        x + width - 2 - len,
        y,
        len,
        right,
        a.style(Role::Dim),
    );
}
fn sides(frame: &mut Frame<'_>, x: u16, y: u16, width: u16, a: Appearance) {
    put(frame, x, y, 1, "│", a.style(Role::Border));
    put(frame, x + width - 1, y, 1, "│", a.style(Role::Border));
}
fn panel_rows(sources: usize) -> u16 {
    (sources as u16 * 2).max(5)
}
pub fn list_rows(width: u16, height: u16, sources: usize) -> u16 {
    height.saturating_sub(if width >= 100 {
        panel_rows(sources) + 7
    } else {
        6
    })
}
fn band_role(band: &str) -> Role {
    CHART_BANDS
        .iter()
        .position(|b| *b == band)
        .map_or(Role::Text, |i| BAND_ROLES[i])
}
fn mode_role(mode: &str) -> Role {
    match mode {
        "CW" => Role::Cw,
        "FT8" => Role::Ft8,
        "SSB" => Role::Ssb,
        "RTTY" => Role::Rtty,
        _ => Role::Text,
    }
}
fn integer(spot: &Spot, field: &str) -> Option<i64> {
    match spot.event.fields.get(field) {
        Some(Value::Integer(v)) => Some(*v),
        _ => None,
    }
}
fn snr_role(snr: Option<i64>) -> Role {
    match snr {
        Some(20..) => Role::Good,
        Some(8..) => Role::Text,
        _ => Role::Dim,
    }
}
fn spark(row: &mut Row<'_, '_>, values: &[u64], a: Appearance, hidden: bool) {
    let max = values.iter().copied().max().unwrap_or(0).max(1);
    for v in values {
        let ch = if *v == 0 {
            '▁'
        } else {
            BLOCK[1 + (u128::from(*v) * 7 / u128::from(max)) as usize]
        };
        row.ui(
            &ch.to_string(),
            a.style(if *v == 0 || hidden {
                Role::Dim
            } else {
                Role::Spark
            }),
        );
    }
}
fn history(model: &Model, now: Duration, source: Option<usize>, n: usize) -> Vec<u64> {
    let mut bins = vec![0; n];
    let minute = now.as_secs() / 60;
    for b in &model.activity {
        let age = minute.saturating_sub(b.second / 60) as usize;
        if age < n {
            bins[n - 1 - age] += source.map_or_else(|| b.sources.iter().sum(), |i| b.sources[i]);
        }
    }
    bins
}
fn state(source: &super::model::Status, now: Duration) -> (char, Role, String) {
    if source.hidden {
        return ('○', Role::Dim, "off".into());
    }
    if !source.diagnostic.is_empty() {
        return ('○', Role::Bad, "error".into());
    }
    if let Some(at) = source.retry_at {
        return (
            '◐',
            Role::Warning,
            format!(
                "retry {}s",
                at.saturating_sub(now).as_millis().div_ceil(1000)
            ),
        );
    }
    match source.state.as_str() {
        "ready" => ('●', Role::Good, "ready".into()),
        "connecting" | "await_login" | "await_identity" | "setup" | "negotiating"
        | "authenticating" | "backoff" | "retrying" => ('◐', Role::Warning, source.state.clone()),
        _ => ('○', Role::Bad, source.state.clone()),
    }
}
fn source_detail(source: &super::model::Status, now: Duration) -> Option<String> {
    if !source.diagnostic.is_empty() {
        return Some(format!("error: {}", source.diagnostic));
    }
    if source.complete {
        return Some("replay finished".into());
    }
    let message = if let Some(at) = source.retry_at {
        format!(
            "retrying in {}s",
            at.saturating_sub(now).as_millis().div_ceil(1000)
        )
    } else {
        match source.state.as_str() {
            "ready" => return None,
            "" | "connecting" => "connecting",
            "await_login" => "waiting for login",
            "await_identity" => "waiting for identification",
            "setup" => "completing setup",
            "negotiating" => "negotiating connection",
            "authenticating" => "authenticating",
            "backoff" | "retrying" => "retrying",
            "blocked" => "connection failed",
            "disconnected" => "disconnected",
            _ => "connection pending",
        }
        .into()
    };
    let reason = match source.reason.as_str() {
        "" | "retry_scheduled" | "connected" => String::new(),
        "eof" => "connection closed".into(),
        "retry_exhausted" => "retries exhausted".into(),
        other => other.replace('_', " "),
    };
    Some(if reason.is_empty() {
        message
    } else {
        format!("{message}: {reason}")
    })
}
fn comma(value: u64) -> String {
    let text = value.to_string();
    text.chars()
        .enumerate()
        .fold(String::new(), |mut out, (i, c)| {
            if i > 0 && (text.len() - i).is_multiple_of(3) {
                out.push(',');
            }
            out.push(c);
            out
        })
}
// Gregorian conversion from days since the Unix epoch; UTC uses no local zone.
fn utc(seconds: u64) -> String {
    let days = (seconds / 86400) as i64 + 719468;
    let era = days / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}  {:02}:{:02}:{:02}Z",
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
    )
}
fn dashboard(frame: &mut Frame<'_>, m: &Model, now: Duration) {
    let area = frame.area();
    let w = area.width;
    let wide = w >= 100;
    let a = m.appearance;
    // Set foreground only. Every unused cell retains the terminal background.
    let date = utc(m.utc_epoch.saturating_add(now.as_secs()));
    let date_width = if wide { 21 } else { 21.min(w / 3) };
    let mut row = Row::new(frame, 0, 0, w - date_width - 1);
    row.ui(" QG DX Cluster ", a.chip(true));
    // QGDXC-TUI-0070: the running version, beside the logo.
    row.ui(concat!(" v", env!("CARGO_PKG_VERSION")), a.style(Role::Dim));
    let live = m.sources.iter().filter(|s| s.state == "ready").count();
    row.ui(
        &format!(
            "  {live} {} live",
            if live == 1 { "source" } else { "sources" }
        ),
        a.style(Role::Text),
    );
    if wide {
        row.ui("  ·  ", a.style(Role::Dim));
        row.ui(
            &format!("{} spots", comma(m.spots.len() as u64)),
            a.style(Role::Text),
        );
    }
    row.ui("  ·  ", a.style(Role::Dim));
    let rate: u64 = m
        .activity
        .iter()
        .filter(|b| now.as_secs().saturating_sub(b.second) < 60)
        .map(|b| b.sources.iter().sum::<u64>())
        .sum();
    // Compact: the version needs the room; the sparkline still shows the rate.
    if wide {
        row.ui(&format!("{rate}/min "), a.style(Role::Text));
    }
    spark(&mut row, &history(m, now, None, 12), a, false);
    put(
        frame,
        w - date_width - 1,
        0,
        date_width,
        &date,
        a.style(Role::Title).add_modifier(Modifier::BOLD),
    );
    let title_y = if wide {
        let lw = w / 2;
        let rw = w - lw;
        let rows = panel_rows(m.sources.len());
        let details: Vec<_> = m
            .sources
            .iter()
            .enumerate()
            .filter_map(|(i, source)| source_detail(source, now).map(|text| (i, text)))
            .collect();
        border(frame, (0, 1, lw), true, " sources ", " 1-8 toggles ", a);
        border(frame, (lw, 1, rw), true, " bands ", " last 15 min ", a);
        let mut counts = [0; 11];
        for b in m
            .activity
            .iter()
            .filter(|b| now.as_secs().saturating_sub(b.second) < 900)
        {
            for (i, n) in b.bands.iter().enumerate() {
                counts[i] += n;
            }
        }
        let max = counts.iter().copied().max().unwrap_or(0).max(1);
        for r in 0..rows {
            let y = 2 + r;
            sides(frame, 0, y, lw, a);
            sides(frame, lw, y, rw, a);
            if r < m.sources.len() as u16 {
                let i = r as usize;
                let source = &m.sources[i];
                let (dot, role, state) = state(source, now);
                let text = if source.hidden { Role::Dim } else { Role::Text };
                let mut row = Row::new(frame, 1, y, lw - 2);
                row.ui(&format!(" {} ", SUP[i]), a.style(Role::Accent));
                row.ui(&dot.to_string(), a.style(role));
                row.ui(" ", a.style(Role::Text));
                // Shrink the host on narrower wide screens, keeping activity visible.
                row.safe(
                    &source.label,
                    (lw.saturating_sub(44)).max(6) as usize,
                    false,
                    a.style(text),
                    true,
                );
                row.ui(" ", a.style(Role::Text));
                let profile = match source.server.as_str() {
                    "dxspider" => "DXSpider",
                    "rbn" => "RBN",
                    "arcluster" => "AR-Cluster",
                    "cccluster" => "CC Cluster",
                    other => other,
                };
                row.safe(profile, 11, false, a.style(Role::Dim), true);
                row.safe(&state, 9, false, a.style(role), true);
                row.safe(&comma(source.count), 6, true, a.style(text), true);
                row.ui("  ", a.style(Role::Text));
                spark(&mut row, &history(m, now, Some(i), 8), a, source.hidden);
            } else if let Some((i, detail)) = details.get(r as usize - m.sources.len()) {
                let mut row = Row::new(frame, 2, y, lw - 3);
                row.ui(SUP[*i], a.style(Role::Dim));
                row.ui(" ", a.style(Role::Dim));
                row.safe(detail, (lw - 6) as usize, false, a.style(Role::Dim), false);
            }
            if r < 5 {
                let cell = (rw - 3) / 11;
                for i in 0..11 {
                    let text = if r < 3 {
                        let level = (u128::from(counts[i]) * 24).div_ceil(u128::from(max)) as u16;
                        let v = level.saturating_sub((2 - r) * 8).min(8);
                        let ch = if r == 2 && counts[i] > 0 && v == 0 {
                            '▁'
                        } else {
                            BLOCK[v as usize]
                        };
                        format!(
                            " {} ",
                            ch.to_string().repeat(cell.saturating_sub(2) as usize)
                        )
                    } else if r == 3 {
                        format!(
                            "{:^width$}",
                            CHART_BANDS[i].trim_end_matches('m'),
                            width = cell as usize
                        )
                    } else {
                        format!("{:^width$}", counts[i], width = cell as usize)
                    };
                    put(
                        frame,
                        lw + 2 + i as u16 * cell,
                        y,
                        cell,
                        &text,
                        a.style(if r == 4 { Role::Dim } else { BAND_ROLES[i] }),
                    );
                }
            }
        }
        border(frame, (0, rows + 2, lw), false, "", "", a);
        border(frame, (lw, rows + 2, rw), false, "", "", a);
        rows + 3
    } else {
        let mut row = Row::new(frame, 1, 1, w - 2);
        for (i, source) in m.sources.iter().enumerate() {
            let (dot, role, state) = state(source, now);
            row.ui(SUP[i], a.style(Role::Accent));
            row.ui(&dot.to_string(), a.style(role));
            row.ui(" ", a.style(Role::Text));
            // Allocate each source a bounded share, including eight-source screens.
            let width = ((w - 2) as usize / m.sources.len()).saturating_sub(4);
            let name = source.label.split('.').next().unwrap_or(&source.label);
            let value = format!(
                "{name} {}{}",
                if state == "ready" {
                    comma(source.count)
                } else {
                    state
                },
                if source.complete { " COMPLETE" } else { "" }
            );
            row.safe(&value, width, false, a.style(Role::Text), true);
            row.ui(" ", a.style(Role::Text));
        }
        2
    };
    let hidden = m.sources.iter().filter(|s| s.hidden).count();
    let summary = m.filters.summary();
    let active = hidden > 0 || summary != "Filters: off";
    let filters = if !active {
        " filter all bands · all modes · all kinds ".into()
    } else {
        format!(
            " filter {}{} ",
            summary.strip_prefix("Filters: ").unwrap_or(&summary),
            if hidden > 0 {
                format!(" hidden:{hidden}")
            } else {
                String::new()
            }
        )
    };
    if m.map {
        spot_map(
            frame,
            (title_y, area.height - 2),
            m,
            now,
            &right_filters(m, &filters),
        );
        footer(frame, area.height - 1, w, m);
        return;
    }
    let left = " spots newest first ";
    let right = if m.paused {
        format!(" PAUSED · {} new ", m.paused_new())
    } else {
        filters.clone()
    };
    border(frame, (0, title_y, w), true, left, &right, a);
    put(
        frame,
        9,
        title_y,
        12,
        "newest first",
        a.style(Role::Dim).remove_modifier(Modifier::BOLD),
    );
    if m.paused {
        let len = right.chars().count() as u16;
        put(frame, w - len - 2, title_y, len, &right, a.chip(false));
    }
    let bottom = area.height - 2;
    for y in title_y + 1..bottom {
        sides(frame, 0, y, w, a);
    }
    spot_header(frame, title_y + 1, w, wide, a, source_width(m));
    let matching = m.matching();
    for (i, spot) in matching
        .iter()
        .skip(m.offset(&matching))
        .take(
            list_rows(w, area.height, m.sources.len()).saturating_sub(u16::from(m.paused && active))
                as usize,
        )
        .enumerate()
    {
        spot_row(frame, title_y + 2 + i as u16, w, wide, spot, m, now);
    }
    if m.paused && active {
        let mut row = Row::new(frame, 1, bottom - 1, w - 2);
        row.safe(
            &filters,
            (w - 2) as usize,
            false,
            a.style(Role::Text),
            false,
        );
    }
    border(
        frame,
        (0, bottom, w),
        false,
        &format!(" {} received ", comma(m.total)),
        &format!(
            " {} human · {} skimmer ",
            comma(m.kept_human as u64),
            comma(m.kept_skimmer as u64)
        ),
        a,
    );
    footer(frame, area.height - 1, w, m);
}
fn right_filters(m: &Model, filters: &str) -> String {
    if m.paused {
        format!(" PAUSED · {} new ", m.paused_new())
    } else {
        filters.to_owned()
    }
}
/// How long a path stays on the map, by spot kind (QGDXC-TUI-0085).
pub const HUMAN_PATH: Duration = Duration::from_secs(60);
pub const SKIMMER_PATH: Duration = Duration::from_secs(20);
/// Most paths drawn in one frame, newest first (QGDXC-TUI-0086).
pub const MAX_PATHS: usize = 200;
/// The spots that have a path on the map now, newest first, with each path's role.
pub(crate) fn map_paths(m: &Model, now: Duration) -> Vec<(&Spot, Role)> {
    m.matching()
        .into_iter()
        .filter(|s| s.ends.is_some())
        .filter_map(|s| {
            let age = now.saturating_sub(s.born);
            let human = s.event.get("spot_kind") == Some("human");
            let role = if !human {
                (age < SKIMMER_PATH).then_some(Role::Dim)
            } else if age < HUMAN_PATH / 3 {
                Some(band_role(s.band))
            } else if age < HUMAN_PATH * 2 / 3 {
                Some(Role::Dim)
            } else if age < HUMAN_PATH {
                Some(Role::Border)
            } else {
                None
            };
            role.map(|r| (s, r))
        })
        .take(MAX_PATHS)
        .collect()
}
fn spot_map(
    frame: &mut Frame<'_>,
    (top, bottom): (u16, u16),
    m: &Model,
    now: Duration,
    right: &str,
) {
    use ratatui::symbols::Marker;
    use ratatui::widgets::canvas::{Canvas, Circle, Line, Map, MapResolution, Points};
    let a = m.appearance;
    let w = frame.area().width;
    let paths = map_paths(m, now);
    border(frame, (0, top, w), true, " map ", right, a);
    for y in top + 1..bottom {
        sides(frame, 0, y, w, a);
    }
    border(
        frame,
        (0, bottom, w),
        false,
        &format!(" {} paths · M list ", paths.len()),
        " grid ● · prefix ○ ",
        a,
    );
    let rect = Rect::new(
        1,
        top + 1,
        w.saturating_sub(2),
        bottom.saturating_sub(top + 1),
    );
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    let canvas = Canvas::default()
        .marker(Marker::Braille)
        .x_bounds([-180.0, 180.0])
        .y_bounds([-90.0, 90.0])
        .paint(|ctx| {
            ctx.draw(&Map {
                color: a.color(Role::Border),
                resolution: MapResolution::High,
            });
            ctx.layer();
            // Oldest first, so the newest path is drawn on top.
            for (spot, role) in paths.iter().rev() {
                let (from, to) = spot.ends.expect("located spot");
                let color = a.color(*role);
                for line in super::geo::great_circle(from.place, to.place) {
                    for pair in line.windows(2) {
                        ctx.draw(&Line {
                            x1: pair[0].0,
                            y1: pair[0].1,
                            x2: pair[1].0,
                            y2: pair[1].1,
                            color,
                        });
                    }
                }
                for end in [from, to] {
                    if end.exact {
                        ctx.draw(&Points {
                            coords: &[(end.place.lon, end.place.lat)],
                            color,
                        });
                    } else {
                        ctx.draw(&Circle {
                            x: end.place.lon,
                            y: end.place.lat,
                            radius: 2.0,
                            color,
                        });
                    }
                }
            }
        });
    frame.render_widget(canvas, rect);
}
fn spot_header(frame: &mut Frame<'_>, y: u16, w: u16, wide: bool, a: Appearance, src: usize) {
    let mut row = Row::new(frame, 1, y, w - 2);
    let style = a.style(Role::Heading).add_modifier(Modifier::UNDERLINED);
    row.ui(" ", style);
    for (text, width, gap, right) in if wide {
        vec![
            ("UTC", 4, 2, false),
            ("Freq kHz", FREQ_WIDE, 2, false),
            ("DX", 10, 1, false),
            ("Spotter", SPOTTER_WIDE, 2, false),
            ("Band", 4, 2, false),
            ("Mode", MODE_WIDE, 2, false),
            ("SNR", 5, 2, false),
            ("WPM", 3, 2, false),
            ("Src", src, 2, false),
        ]
    } else {
        vec![
            ("UTC", 4, 1, false),
            ("kHz", 8, 2, false),
            ("DX", 10, 1, false),
            ("Spotter", 10, 1, false),
            ("Band", 4, 1, false),
            ("Mode", 4, 1, false),
        ]
    } {
        row.safe(text, width, right, style, false);
        row.ui(&" ".repeat(gap), a.style(Role::Text));
    }
    row.ui("Comment", style);
}
fn spot_row(
    frame: &mut Frame<'_>,
    y: u16,
    w: u16,
    wide: bool,
    spot: &Spot,
    m: &Model,
    now: Duration,
) {
    let a = m.appearance;
    let human = spot.event.get("spot_kind") == Some("human");
    // QGDXC-TUI-0073: only human spots glow; a skimmer feed would keep the list flashing.
    let glow = if human {
        a.highlight(now.saturating_sub(spot.born).as_millis())
    } else {
        ratatui::style::Style::default()
    };
    for x in 1..w - 1 {
        if let Some(cell) = frame.buffer_mut().cell_mut((x, y)) {
            cell.set_style(glow);
        }
    }
    let mut row = Row::new(frame, 1, y, w - 2);
    row.ui(if human { "▎" } else { " " }, a.style(Role::Accent));
    let time = spot
        .event
        .get("time_of_day")
        .unwrap_or_default()
        .replace(':', "");
    row.safe(&time, 4, false, a.style(Role::Time), true);
    row.ui(if wide { "  " } else { " " }, a.style(Role::Text));
    let freq = if spot.event.fields.contains_key("frequency_hz") {
        frequency_text(frequency(&spot.event))
    } else {
        String::new()
    };
    row.safe(
        &freq,
        if wide { FREQ_WIDE } else { 8 },
        true,
        a.style(Role::Frequency),
        true,
    );
    row.ui("  ", a.style(Role::Text));
    row.safe(
        spot.event.get("call").unwrap_or_default(),
        10,
        false,
        a.style(if human { Role::Human } else { Role::Skimmer })
            .add_modifier(if human {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
        true,
    );
    row.ui(" ", a.style(Role::Text));
    row.safe(
        spot.event.get("spotter_call").unwrap_or_default(),
        if wide { SPOTTER_WIDE } else { 10 },
        false,
        a.style(Role::Spotter),
        true,
    );
    row.ui(if wide { "  " } else { " " }, a.style(Role::Text));
    row.safe(spot.band, 4, false, a.style(band_role(spot.band)), true);
    row.ui(if wide { "  " } else { " " }, a.style(Role::Text));
    let mode = if spot.mode == "unknown" {
        ""
    } else {
        spot.mode.as_str()
    };
    row.safe(
        mode,
        if wide { MODE_WIDE } else { 4 },
        false,
        a.style(mode_role(&spot.mode)),
        true,
    );
    row.ui(if wide { "  " } else { " " }, a.style(Role::Text));
    if wide {
        let snr = integer(spot, "snr_db");
        row.safe(
            &snr.map(|n| format!("{n}dB")).unwrap_or_default(),
            5,
            true,
            a.style(snr_role(snr)),
            true,
        );
        row.ui("  ", a.style(Role::Text));
        let speed = if spot.event.get("speed_unit") == Some("wpm") {
            integer(spot, "speed")
                .map(|n| n.to_string())
                .unwrap_or_default()
        } else {
            String::new()
        };
        row.safe(&speed, 3, true, a.style(Role::Dim), true);
        row.ui("  ", a.style(Role::Text));
        let label = short_label(&m.sources[spot.source].label);
        let label = match m.other_copies(spot) {
            0 => label.to_owned(),
            n => format!("{label}+{n}"),
        };
        row.safe(&label, source_width(m), false, a.style(Role::Dim), true);
        row.ui("  ", a.style(Role::Text));
    }
    let remaining = row.end.saturating_sub(row.x) as usize;
    row.safe(
        display_comment(spot, wide),
        remaining,
        false,
        a.style(if human { Role::Text } else { Role::Dim }),
        false,
    );
}
// Remove only a prefix corroborated by fields visible in this layout. Borrowing
// the original suffix preserves its text and leaves retained/recorded data alone.
const SPOTTER_WIDE: usize = 12;
/// Fits 10 GHz and up in kHz with its tenth, e.g. `10489520.0` (QGDXC-TUI-0011).
const FREQ_WIDE: usize = 10;
const MODE_WIDE: usize = 6;
fn short_label(label: &str) -> &str {
    label.split('.').next().unwrap_or_default()
}
/// The Src column fits the longest source label, between 3 and 8 characters, plus
/// room for a `+N` copy count while duplicates are collapsed (QGDXC-TUI-0015).
pub(crate) fn source_width(m: &Model) -> usize {
    let label = m
        .sources
        .iter()
        .map(|s| short_label(&s.label).len())
        .max()
        .unwrap_or(3)
        .clamp(3, 8);
    if m.sources.len() > 1 && !m.filters.dupes {
        label + 2
    } else {
        label
    }
}
fn display_comment(spot: &Spot, wide: bool) -> &str {
    let original = spot.event.get("comment").unwrap_or_default();
    if !wide {
        return original;
    }
    fn token(text: &str) -> (&str, &str) {
        let text = text.trim_start_matches(|c: char| c.is_ascii_whitespace());
        text.split_once(|c: char| c.is_ascii_whitespace())
            .unwrap_or((text, ""))
    }
    let derive = || {
        let (mode, rest) = token(original);
        if Some(mode) != spot.event.get("mode") || mode != spot.mode || mode.len() > MODE_WIDE {
            return None;
        }
        let (snr, rest) = token(rest);
        let expected = integer(spot, "snr_db")?;
        if format!("{expected}dB").len() > 5 {
            return None;
        }
        if snr != expected.to_string() && !(expected >= 0 && snr == format!("+{expected}")) {
            return None;
        }
        let (unit, mut rest) = token(rest);
        if unit != "dB" {
            return None;
        }
        if spot.event.get("speed_unit") == Some("wpm") {
            let (speed, tail) = token(rest);
            if speed != integer(spot, "speed")?.to_string() || speed.len() > 3 {
                return None;
            }
            let (unit, tail) = token(tail);
            if unit != "WPM" {
                return None;
            }
            rest = tail;
        }
        Some(rest.trim_start_matches(|c: char| c.is_ascii_whitespace()))
    };
    // Otherwise, for a spot without typed SNR, a leading word that is exactly the
    // mode shown in this row. A skimmer prefix is all or nothing, as above.
    let mode_word = || {
        let (word, rest) = token(original);
        (integer(spot, "snr_db").is_none()
            && spot.mode != "unknown"
            && spot.mode.len() <= MODE_WIDE
            && word.eq_ignore_ascii_case(&spot.mode))
        .then(|| rest.trim_start_matches(|c: char| c.is_ascii_whitespace()))
    };
    derive().or_else(mode_word).unwrap_or(original)
}
/// From this width every footer key carries its label (QGDXC-TUI-0070).
const FULL_FOOTER: u16 = 120;
/// From this width footer hints are separated by two spaces instead of one.
const ROOMY_FOOTER: u16 = 130;
fn footer(frame: &mut Frame<'_>, y: u16, w: u16, m: &Model) {
    let a = m.appearance;
    let theme = format!("{} ", a.theme.name());
    let mut row = Row::new(frame, 1, y, w - theme.len() as u16 - 2);
    for (key, label) in [
        ("q", "quit"),
        ("b", "band"),
        ("m", "mode"),
        ("/", "call"),
        ("k", "kind"),
        ("d", if w >= FULL_FOOTER { "dupes" } else { "" }),
        ("s", if w >= FULL_FOOTER { "self" } else { "" }),
        (
            "1-8",
            match w {
                FULL_FOOTER.. => "source",
                100.. => "src",
                _ => "",
            },
        ),
        ("p", if m.paused { "resume" } else { "pause" }),
        ("l", "limits"),
        ("t", if w >= FULL_FOOTER { "theme" } else { "" }),
        (
            "M",
            match (w >= 100, m.map) {
                (false, _) => "",
                (true, true) => "list",
                (true, false) => "map",
            },
        ),
        ("?", if w >= 100 { "help" } else { "" }),
    ] {
        row.ui(key, a.style(Role::Accent).add_modifier(Modifier::BOLD));
        if !label.is_empty() {
            row.ui(&format!(" {label}"), a.style(Role::Dim));
        }
        row.ui(
            if w >= ROOMY_FOOTER { "  " } else { " " },
            a.style(Role::Dim),
        );
    }
    put(
        frame,
        w - theme.len() as u16,
        y,
        theme.len() as u16,
        &theme,
        a.style(Role::Dim),
    );
}
