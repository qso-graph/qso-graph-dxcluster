// SPDX-License-Identifier: GPL-3.0-or-later
use super::model::{Editor, Model, Spot, clip, frequency, frequency_text, sanitize};
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
pub fn widths(width: u16, source: u16) -> [u16; 9] {
    let fixed = [4, 12, 10, 10, 6, 7, 7, source, 0];
    let mut widths = fixed;
    widths[8] = width.saturating_sub(fixed.iter().sum::<u16>() + 8);
    widths
}
pub fn row(frame: &mut Frame<'_>, area: Rect, spot: &Spot, label: &str, widths: [u16; 9]) {
    let event = &spot.event;
    let time = event
        .get("time_of_day")
        .unwrap_or_default()
        .replace(':', "");
    let hz = frequency(event);
    let freq = if event.fields.contains_key("frequency_hz") {
        frequency_text(hz)
    } else {
        String::new()
    };
    let fields = [
        time.as_str(),
        &freq,
        event.get("call").unwrap_or_default(),
        event.get("spotter_call").unwrap_or_default(),
        spot.band,
        &spot.mode,
        event.get("spot_kind").unwrap_or_default(),
        label,
        event.get("comment").unwrap_or_default(),
    ];
    let mut x = area.x;
    for (i, (text, width)) in fields.into_iter().zip(widths).enumerate() {
        field(frame, Rect::new(x, area.y, width, 1), text, i != 8);
        x += width + 1;
    }
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
    match &model.editor {
        Editor::Calls { text, error } => {
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
            let rows = area.height as usize - 2;
            let start = cursor.saturating_sub(rows - 1);
            for (index, value) in options.iter().enumerate().skip(start).take(rows) {
                line(
                    frame,
                    area,
                    (index - start + 2) as u16,
                    &format!(
                        "{} [{}] {}",
                        if index == *cursor { ">" } else { " " },
                        if selected.contains(value) { "x" } else { " " },
                        value
                    ),
                );
            }
            return;
        }
        Editor::Help => {
            for (y, text) in [
                "QG DX Cluster: receive-only multi-source spot viewer",
                "q / Ctrl-C: quit   b: bands   m: modes   /: callsign patterns",
                "k: all/human/skimmer   1-8: hide/show source   c: clear filters",
                "p: pause/resume scrolling (reception continues)",
                "Up/Down/PageUp/PageDown/Home: scroll   ?: help",
                "Callsign editor: printable keys are data; Backspace deletes",
                "Enter commits; Escape cancels. Wildcards: * any run, ? one character",
                "Band/mode: Up/Down moves, Space toggles, Enter commits, Esc cancels",
                "Esc / Enter / ?: return   Ctrl-C quits from every screen",
            ]
            .iter()
            .enumerate()
            {
                line(frame, area, y as u16, text);
            }
            return;
        }
        Editor::Normal => {}
    }
    line(
        frame,
        area,
        0,
        &format!("QG DX Cluster  {}", model.summary()),
    );
    line(frame, area, 1, &model.filters.summary());
    for (index, source) in model.sources.iter().enumerate() {
        line(
            frame,
            area,
            index as u16 + 2,
            &format!("{} {}", index + 1, source.line(now)),
        );
    }
    let header_y = 2 + model.sources.len() as u16;
    let source_width = model
        .sources
        .iter()
        .map(|s| s.label.len())
        .max()
        .unwrap_or(1)
        .max("Source".len()) as u16;
    let widths = widths(area.width, source_width);
    let mut x = area.x;
    for (text, width) in [
        "UTC", "kHz", "DX", "Spotter", "Band", "Mode", "Kind", "Source", "Comment",
    ]
    .iter()
    .zip(widths)
    {
        field(frame, Rect::new(x, header_y, width, 1), text, true);
        x += width + 1;
    }
    let matching = model.matching();
    for (i, spot) in matching
        .iter()
        .skip(model.offset(&matching))
        .take((area.height - header_y - 2) as usize)
        .enumerate()
    {
        row(
            frame,
            Rect::new(area.x, header_y + 1 + i as u16, area.width, 1),
            spot,
            &model.sources[spot.source].label,
            widths,
        );
    }
    line(
        frame,
        area,
        area.height - 1,
        "q quit  b band  m mode  / call  k kind  1-8 source  c clear  p pause  ? help",
    );
}
