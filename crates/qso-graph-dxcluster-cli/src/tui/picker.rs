// SPDX-License-Identifier: GPL-3.0-or-later
//! The Sources screen: choose saved sources before any opens (QGDXC-TUI-0008).
use super::{
    model::{Input, clip, sanitize},
    theme::{Appearance, Role},
};
use qso_graph_dxcluster::Profile;
use qso_graph_dxcluster_cli::saved::Saved;
use ratatui::{Frame, layout::Rect, style::Modifier, text::Span, widgets::Paragraph};

/// Most sources that run at once (QGDXC-TUI-0001).
pub const MAX_CHOSEN: usize = 8;

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Stay,
    Start(Vec<usize>),
    Quit,
}

pub struct Picker {
    pub rows: Vec<(String, String, String)>,
    pub chosen: Vec<bool>,
    pub cursor: usize,
    pub message: Option<&'static str>,
}

impl Picker {
    pub fn new(saved: &Saved) -> Self {
        Self {
            rows: saved
                .entries
                .iter()
                .map(|e| {
                    let profile = if e.profile == Profile::Auto {
                        "auto"
                    } else {
                        e.profile.name()
                    };
                    (e.name.clone(), e.node.clone(), profile.to_owned())
                })
                .collect(),
            chosen: saved.entries.iter().map(|e| e.selected).collect(),
            cursor: 0,
            message: None,
        }
    }
    pub fn key(&mut self, input: Input) -> Outcome {
        match input {
            Input::CtrlC | Input::Escape | Input::Char('q') => return Outcome::Quit,
            Input::Up => self.cursor = self.cursor.saturating_sub(1),
            Input::Down => self.cursor = (self.cursor + 1).min(self.rows.len().saturating_sub(1)),
            Input::Char(' ') => {
                if let Some(c) = self.chosen.get_mut(self.cursor) {
                    *c = !*c;
                }
            }
            Input::Enter => {
                let picks: Vec<usize> =
                    (0..self.chosen.len()).filter(|&i| self.chosen[i]).collect();
                if (1..=MAX_CHOSEN).contains(&picks.len()) {
                    return Outcome::Start(picks);
                }
                self.message = Some("Choose 1 to 8 sources");
                return Outcome::Stay;
            }
            _ => {}
        }
        self.message = None;
        Outcome::Stay
    }
}

fn text(
    frame: &mut Frame<'_>,
    x: u16,
    y: u16,
    width: u16,
    value: &str,
    style: ratatui::style::Style,
) {
    if width == 0 || y >= frame.area().height {
        return;
    }
    let value = clip(&sanitize(value), width as usize, false);
    frame.render_widget(
        Paragraph::new(Span::styled(value, style)),
        Rect::new(x, y, width, 1),
    );
}

pub fn draw(frame: &mut Frame<'_>, picker: &Picker, a: Appearance) {
    let area = frame.area();
    if area.width < 80 || area.height < 24 {
        if area.height > 0 {
            text(
                frame,
                0,
                0,
                area.width,
                "Enlarge window to at least 80 columns and 24 rows. q: quit",
                a.style(Role::Text),
            );
        }
        return;
    }
    for cell in &mut frame.buffer_mut().content {
        cell.set_style(a.style(Role::Text));
    }
    let w = area.width;
    text(frame, 0, 0, 15, " QG DX Cluster ", a.chip(true));
    text(
        frame,
        16,
        0,
        w - 16,
        "Sources",
        a.style(Role::Title).add_modifier(Modifier::BOLD),
    );
    let count = picker.chosen.iter().filter(|c| **c).count();
    text(
        frame,
        0,
        1,
        w,
        &format!(
            "{count} of {} chosen; up to {MAX_CHOSEN} run at once",
            picker.rows.len()
        ),
        a.style(Role::Dim),
    );
    text(frame, 2, 3, 3, "", a.style(Role::Heading));
    text(frame, 6, 3, 33, "Name", a.style(Role::Heading));
    text(frame, 40, 3, w - 52, "Node", a.style(Role::Heading));
    text(frame, w - 11, 3, 10, "Profile", a.style(Role::Heading));
    // Rows 4..h-3 list sources; the window scrolls to keep the cursor visible.
    let visible = (area.height - 7) as usize;
    let first = picker.cursor.saturating_sub(visible.saturating_sub(1));
    for (row, i) in (first..picker.rows.len()).take(visible).enumerate() {
        let y = 4 + row as u16;
        let (name, node, profile) = &picker.rows[i];
        let style = if i == picker.cursor {
            a.style(Role::Text).add_modifier(Modifier::REVERSED)
        } else {
            a.style(Role::Text)
        };
        text(frame, 0, y, w, "", style);
        text(
            frame,
            0,
            y,
            2,
            if i == picker.cursor { ">" } else { " " },
            style,
        );
        text(
            frame,
            2,
            y,
            3,
            if picker.chosen[i] { "[x]" } else { "[ ]" },
            style,
        );
        text(frame, 6, y, 33, name, style);
        text(frame, 40, y, w - 52, node, style);
        text(frame, w - 11, y, 10, profile, style);
    }
    if let Some(message) = picker.message {
        text(
            frame,
            0,
            area.height - 3,
            w,
            message,
            a.style(Role::Warning),
        );
    }
    text(
        frame,
        0,
        area.height - 1,
        w,
        "Up/Down move  Space choose  Enter start  q quit",
        a.style(Role::Dim),
    );
}
