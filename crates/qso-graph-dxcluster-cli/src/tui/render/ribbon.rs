// SPDX-License-Identifier: GPL-3.0-or-later
//! In-force space-weather ribbon (QGDXC-TUI-0125–0126).

use super::{Appearance, Frame, Model, Role, Row, border, sanitize, space_weather};
use crate::tui::weather::{ALERTS, Data};
use qso_graph_spaceweather::{self as swx, Alert, AlertKind};

pub(super) struct Ribbon<'a> {
    alert: &'a Alert,
    label: &'static str,
    role: Role,
    extra: String,
    utc_now: i64,
}

impl<'a> Ribbon<'a> {
    pub(super) fn current(model: &'a Model, utc_now: u64) -> Option<Self> {
        // A good sample can only arrive after the worker starts.
        let (Data::Alerts { values, .. }, _) = model.weather_data.samples[ALERTS].good.as_ref()?
        else {
            return None;
        };
        let utc_now = i64::try_from(utc_now).unwrap_or(i64::MAX);
        let items = swx::in_force(values, utc_now);
        let (&alert, rest) = items.split_first()?;
        use AlertKind::*;
        let (label, role) = match alert.kind? {
            Alert | ContinuedAlert => ("ALERT", Role::Bad),
            Warning | ExtendedWarning => ("WARNING", Role::Warning),
            Watch => ("WATCH", Role::Accent),
            CancelWarning | CancelWatch | Summary => return None,
        };
        let extra = if rest.is_empty() {
            String::new()
        } else {
            format!("  +{}", rest.len())
        };
        Some(Self {
            alert,
            label,
            role,
            extra,
            utc_now,
        })
    }

    pub(super) fn draw(
        &self,
        frame: &mut Frame<'_>,
        (x, y, width): (u16, u16, u16),
        appearance: Appearance,
    ) {
        let wide = frame.area().width >= 100;
        let style = appearance.style(self.role);
        let mut row = Row::new(frame, x, y, width);
        row.ui("▌", style);
        row.ui(self.label, style);
        if wide {
            if let Some((_, headline)) = self
                .alert
                .headline
                .as_deref()
                .and_then(|text| text.split_once(':'))
            {
                let headline = sanitize(headline.trim_matches(' '));
                if !headline.is_empty() {
                    row.ui(" ", style);
                    row.ui(&headline, style);
                }
            }
            if let Some(end) = self.alert.end_time() {
                row.ui(
                    &format!(" · until {}", space_weather::stamp(end, self.utc_now)),
                    style,
                );
            }
        }
        row.ui(&self.extra, style);
    }
}

pub(super) fn panel_top(
    frame: &mut Frame<'_>,
    position: (u16, u16, u16),
    (left, mut right): (&str, &str),
    appearance: Appearance,
    ribbon: Option<&Ribbon<'_>>,
) {
    let (x, y, width) = position;
    let start = 2 + left.chars().count() as u16;
    if let Some(ribbon) = ribbon
        && frame.area().width < 100
    {
        let ribbon_width = 1 + (ribbon.label.len() + ribbon.extra.len()) as u16;
        let right_room = width.saturating_sub(start + ribbon_width + 3);
        if let Some((end, _)) = right.char_indices().nth(usize::from(right_room)) {
            right = &right[..end];
        }
    }
    border(frame, position, true, left, right, appearance);
    if let Some(ribbon) = ribbon {
        let end = width.saturating_sub(2 + right.chars().count() as u16);
        ribbon.draw(frame, (x + start, y, end.saturating_sub(start)), appearance);
    }
}
