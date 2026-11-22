// SPDX-License-Identifier: GPL-3.0-or-later
//! Space-weather history (QGDXC-TUI-0121, QGDXC-TUI-0122, QGDXC-TUI-0124).

use super::{Appearance, Frame, Model, Rect, Role, put, space_weather, utc_parts};
use crate::tui::theme::Theme;
use crate::tui::weather::{Data, Sample};
use qso_graph_spaceweather::Product;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout},
    style::{Modifier, Style},
    symbols::Marker,
    text::Line,
    widgets::{Axis, Bar, BarChart, BarGroup, Block, Chart, Dataset, GraphType, Paragraph, Widget},
};
use std::time::Duration;

const DAY: i64 = 86_400;
const HOUR: i64 = 3_600;

pub(super) fn draw(frame: &mut Frame<'_>, area: Rect, model: &Model, now: Duration) -> bool {
    if area.height < 4 {
        return false;
    }
    let columns = if frame.area().width >= 120 { 2 } else { 1 };
    let rows = (area.height / 4).min(4 / columns);
    let mut heights = [area.height / rows; 4];
    heights[usize::from(rows - 1)] += area.height % rows;
    if columns == 1 && rows >= 3 {
        // Give the X-ray axis five plot rows when the other panels can spare them.
        for other in [0, 1, 3].into_iter().filter(|&i| i < usize::from(rows)) {
            let transfer = heights[other]
                .saturating_sub(4)
                .min(7_u16.saturating_sub(heights[2]));
            heights[other] -= transfer;
            heights[2] += transfer;
        }
    }
    let column_width = area.width / columns;
    for (i, draw) in (0..columns * rows).zip([flux, kp, xrays, wind]) {
        let (column, row) = (i % columns, i / columns);
        let x = area.x + column * column_width;
        let y = area.y + heights[..usize::from(row)].iter().sum::<u16>();
        let width = if column + 1 == columns {
            area.right() - x
        } else {
            column_width
        };
        draw(
            frame,
            Rect::new(x, y, width, heights[usize::from(row)]),
            model,
            now,
        );
    }
    true
}

struct Panel {
    area: Rect,
    appearance: Appearance,
    stale: bool,
    failed: bool,
}

impl Panel {
    fn new(area: Rect, appearance: Appearance) -> Self {
        Self {
            area,
            appearance,
            stale: false,
            failed: false,
        }
    }

    fn product(mut self, model: &Model, product: Product, now: Duration) -> (Self, &Sample) {
        let (feed, sample) = model.weather_data.sample(product);
        self.stale |= sample.stale(feed, now);
        self.failed |= sample.failure(now).is_some();
        (self, sample)
    }

    fn style(&self, role: Role) -> Style {
        if self.stale {
            self.appearance.style(Role::Dim).add_modifier(Modifier::DIM)
        } else {
            self.appearance.style(role)
        }
    }

    fn open(&self, frame: &mut Frame<'_>, title: &str, window: Option<Window>) -> Rect {
        let stale = if self.stale { " · stale" } else { "" };
        let mut block = Block::bordered()
            .border_style(self.style(Role::Border))
            .title_top(Line::styled(
                format!(" {title}{stale} "),
                self.style(Role::Title),
            ));
        if self.failed {
            block = block
                .title_top(Line::styled(" failed ", self.style(Role::Warning)).right_aligned());
        }
        if let Some(window) = window {
            block = block
                .title_bottom(Line::styled(
                    format!("  {} ", window.label(window.start)),
                    self.style(Role::Dim),
                ))
                .title_bottom(Line::styled(" UTC ", self.style(Role::Dim)).centered())
                .title_bottom(
                    Line::styled(
                        format!(" {} ", window.label(window.end)),
                        self.style(Role::Dim),
                    )
                    .right_aligned(),
                );
        }
        let inner = block.inner(self.area);
        frame.render_widget(block, self.area);
        if window.is_none() {
            frame.render_widget(
                Paragraph::new("waiting")
                    .style(self.style(Role::Dim))
                    .alignment(Alignment::Center),
                inner,
            );
        }
        inner
    }

    fn line<'a>(&self, points: &'a [(f64, f64)], role: Role) -> Dataset<'a> {
        Dataset::default()
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(self.style(role))
            .data(points)
    }

    fn plot<'a>(&self, window: Window, lines: Vec<Dataset<'a>>, axis: Axis<'a>) -> Chart<'a> {
        Chart::new(lines)
            .style(self.style(Role::Text))
            .legend_position(None)
            .x_axis(Axis::default().bounds([window.start as f64, window.end as f64]))
            .y_axis(axis.style(self.style(Role::Dim)))
    }
}

#[derive(Clone, Copy)]
struct Window {
    start: i64,
    end: i64,
    days: bool,
}

impl Window {
    fn label(self, time: i64) -> String {
        let time = time.max(0) as u64;
        if self.days {
            let (_, month, day) = utc_parts(time);
            format!("{month:02}-{day:02}")
        } else {
            format!("{:02}:{:02}", time / 3600 % 24, time / 60 % 60)
        }
    }
}

fn value_axis(points: impl Iterator<Item = f64>, step: f64, floor: f64) -> Axis<'static> {
    let (min, max) = points.fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), v| {
        (min.min(v), max.max(v))
    });
    let low = ((min / step).floor() * step - step).max(floor);
    let high = (max / step).ceil() * step + step;
    Axis::default()
        .bounds([low, high])
        .labels([format!("{low}"), format!("{high}")])
}

fn flux(frame: &mut Frame<'_>, area: Rect, model: &Model, now: Duration) {
    let (panel, sample) =
        Panel::new(area, model.appearance).product(model, Product::SolarFluxHistory, now);
    let title = "Solar flux (sfu) · 30 days";
    let Some((Data::SolarFluxHistory(values), _)) = &sample.good else {
        panel.open(frame, title, None);
        return;
    };
    let end = values.last().expect("validated flux history").time;
    let window = Window {
        start: end / DAY * DAY - 29 * DAY,
        end,
        days: true,
    };
    let recent = values.iter().filter(|value| value.time >= window.start);
    let points: Vec<_> = recent
        .clone()
        .map(|value| (value.time as f64, value.flux))
        .collect();
    // A null mean leaves a gap in the line.
    let means: Vec<Vec<_>> = recent
        .collect::<Vec<_>>()
        .split(|value| value.ninety_day_mean.is_none())
        .filter(|run| !run.is_empty())
        .map(|run| {
            run.iter()
                .filter_map(|value| value.ninety_day_mean.map(|mean| (value.time as f64, mean)))
                .collect()
        })
        .collect();
    let axis = value_axis(
        points.iter().chain(means.iter().flatten()).map(|p| p.1),
        10.0,
        0.0,
    );
    let inner = panel.open(frame, title, Some(window));
    let mut lines: Vec<_> = means.iter().map(|run| panel.line(run, Role::Dim)).collect();
    let primary = panel.line(&points, Role::Accent);
    if panel.appearance.theme == Theme::Mono && !panel.stale && !lines.is_empty() {
        // Chart's canvas uses dataset foreground colors. Apply the dim modifier
        // to mean-only cells when both series use the terminal foreground.
        let mut mean = Buffer::empty(inner);
        panel
            .plot(window, lines, axis.clone())
            .render(inner, &mut mean);
        frame.render_widget(panel.plot(window, vec![primary], axis), inner);
        let style = panel.style(Role::Dim).add_modifier(Modifier::DIM);
        for y in inner.top()..inner.bottom() {
            for x in inner.left()..inner.right() {
                let symbol = mean[(x, y)].symbol();
                let cell = &mut frame.buffer_mut()[(x, y)];
                if cell.symbol() == " "
                    && symbol.starts_with(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch))
                {
                    cell.set_symbol(symbol).set_style(style);
                }
            }
        }
    } else {
        lines.push(primary);
        frame.render_widget(panel.plot(window, lines, axis), inner);
    }
}

fn kp(frame: &mut Frame<'_>, area: Rect, model: &Model, now: Duration) {
    const SLOT: i64 = 3 * HOUR;
    const SLOTS: usize = 56;
    let (panel, sample) =
        Panel::new(area, model.appearance).product(model, Product::PlanetaryK, now);
    let title = "Kp (index) · 7 days";
    let Some((Data::PlanetaryK(values), _)) = &sample.good else {
        panel.open(frame, title, None);
        return;
    };
    let end = values.last().expect("validated Kp").time;
    let window = Window {
        start: end - 7 * DAY,
        end,
        days: true,
    };
    let inner = panel.open(frame, title, Some(window));
    // The baseline is on the bottom border, leaving two plot rows at minimum height.
    for (value, label) in [(9, "9"), (5, "5"), (3, "3"), (0, "0")] {
        if value != 3 || inner.height >= 3 {
            let offset = (u32::from(inner.height) * (9 - value) + 4) / 9;
            put(
                frame,
                inner.x,
                inner.y + offset as u16,
                1,
                label,
                panel.style(Role::Dim),
            );
        }
    }
    let mut bars = vec![Bar::default(); SLOTS];
    for value in values.iter().filter(|value| value.time > window.start) {
        let slot = SLOTS - 1 - ((end - value.time) / SLOT) as usize;
        bars[slot] = Bar::new((value.kp * 100.0).round() as u64)
            .style(panel.style(space_weather::kp_role(value.kp)))
            .text_value("");
    }
    let bar_space = inner.width - 1;
    let width = bar_space / SLOTS as u16;
    let used = width * SLOTS as u16;
    let plot = Rect::new(
        inner.x + 1 + (bar_space - used) / 2,
        inner.y,
        used,
        inner.height,
    );
    frame.render_widget(
        BarChart::default()
            .data(BarGroup::default().bars(&bars))
            .style(panel.style(Role::Text))
            .bar_width(width)
            .bar_gap(0)
            .max(900),
        plot,
    );
}

fn xrays(frame: &mut Frame<'_>, area: Rect, model: &Model, now: Duration) {
    let (panel, sample) = Panel::new(area, model.appearance).product(model, Product::Xrays, now);
    let title = "X-rays (W/m²) · 6 hours";
    let Some((Data::Xrays(values), _)) = &sample.good else {
        panel.open(frame, title, None);
        return;
    };
    let end = values.last().expect("validated X-rays").time;
    let window = Window {
        start: end - 6 * HOUR,
        end,
        days: false,
    };
    let points: Vec<_> = values
        .iter()
        .filter(|value| value.time >= window.start)
        .map(|value| (value.time as f64, value.flux.log10()))
        .collect();
    let (low, high) = points
        .iter()
        .fold((-8_i32, -4_i32), |(low, high), &(_, flux)| {
            (low.min(flux.floor() as i32), high.max(flux.ceil() as i32))
        });
    let labels = (low..=high).map(|tick| match tick {
        -8 => "A",
        -7 => "B",
        -6 => "C",
        -5 => "M",
        -4 => "X",
        _ => "",
    });
    let title = if i32::from(area.height) - 2 < high - low + 1 {
        "X-rays (W/m²) · 6 hours · A B C M X"
    } else {
        title
    };
    let inner = panel.open(frame, title, Some(window));
    let axis = Axis::default()
        .bounds([f64::from(low), f64::from(high)])
        .labels(labels);
    frame.render_widget(
        panel.plot(window, vec![panel.line(&points, Role::Accent)], axis),
        inner,
    );
}

fn wind(frame: &mut Frame<'_>, area: Rect, model: &Model, now: Duration) {
    let (panel, _) = Panel::new(area, model.appearance).product(model, Product::WindSpeed, now);
    let (panel, _) = panel.product(model, Product::WindField, now);
    let title = "Solar wind (km/s, nT) · since start";
    let history = &model.wind_history;
    let Some(start) = [&history.speed, &history.bz]
        .into_iter()
        .filter_map(|s| s.front())
        .map(|p| p.0)
        .min()
    else {
        panel.open(frame, title, None);
        return;
    };
    let end = model.utc_epoch.saturating_add(now.as_secs());
    let window = Window {
        start: start as i64,
        end: end.max(start.saturating_add(1)) as i64,
        days: end.saturating_sub(start) >= DAY as u64,
    };
    let inner = panel.open(frame, title, Some(window));
    let areas: [Rect; 2] = Layout::vertical([Constraint::Ratio(1, 2); 2]).areas(inner);
    let series = [
        (&history.speed, "Speed", Role::Accent, 50.0, 0.0),
        (&history.bz, "Bz", Role::Text, 5.0, f64::NEG_INFINITY),
    ];
    for (area, (series, label, role, step, floor)) in areas.into_iter().zip(series) {
        let points: Vec<_> = series
            .iter()
            .map(|&(time, value)| (time as f64, value))
            .collect();
        let style = panel.style(role);
        put(frame, area.x, area.y, 5, label, style);
        if points.is_empty() {
            put(
                frame,
                area.x + 6,
                area.y,
                7,
                "waiting",
                panel.style(Role::Dim),
            );
            continue;
        }
        let mut axis = value_axis(points.iter().map(|p| p.1), step, floor);
        if area.height == 1 {
            axis = axis.labels(std::iter::empty::<Line<'_>>());
        }
        let plot = Rect::new(area.x + 6, area.y, area.width - 6, area.height);
        frame.render_widget(
            panel.plot(window, vec![panel.line(&points, role)], axis),
            plot,
        );
    }
}
