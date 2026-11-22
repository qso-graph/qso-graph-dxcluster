// SPDX-License-Identifier: GPL-3.0-or-later
//! NOAA SWPC space-weather view (QGDXC-TUI-0114, QGDXC-TUI-0117–0119).

use super::ribbon::{self, Ribbon};
use super::{Appearance, Frame, Model, Rect, Role, Row, age, border, put, sides, spark, utc_parts};
use crate::tui::weather::{ALERTS, Data, FEEDS, FLUX_HISTORY, Sample};
use qso_graph_spaceweather::Alert;
use std::time::Duration;

const DAY: i64 = 86_400;
const NO_ALERTS: &str = "no alerts in the last 5 days";

pub(super) fn draw(
    frame: &mut Frame<'_>,
    (top, bottom): (u16, u16),
    m: &Model,
    now: Duration,
    ribbon: Option<&Ribbon<'_>>,
) {
    let width = frame.area().width;
    let appearance = m.appearance;
    let data = &m.weather_data;
    let utc_now = i64::try_from(m.utc_epoch.saturating_add(now.as_secs())).unwrap_or(i64::MAX);
    let mut status = if now < data.pause {
        format!(" Retry-After {} ", countdown(data.pause - now))
    } else {
        format!(
            " next refresh {} ",
            countdown(data.next.saturating_sub(now))
        )
    };
    let wide = width >= 120;
    let measurement_feeds = &FEEDS[..ALERTS];
    let alert_feed = FEEDS[ALERTS];
    let alert_sample = &data.samples[ALERTS];
    let recent = recent_alerts(alert_sample, utc_now);
    let available = width - 3;
    let left = if wide {
        (u32::from(available) * 55 / 100) as u16
    } else {
        available
    };
    let alert_x = if wide { 2 + left + 2 } else { 2 };
    let mut first = top + 1;
    let rows = usize::from(bottom - first);
    // Move the headings onto the border when a heading row would displace a figure.
    let border_headings = wide && rows <= measurement_feeds.len();
    if wide && !border_headings {
        first += 1;
    }
    let rows = usize::from(bottom - first);
    let max_columns = if wide { (left / 41).max(1) } else { 3 };
    let columns = measurement_feeds
        .len()
        .div_ceil(rows.max(1))
        .min(usize::from(max_columns));
    let compact = columns > 1;
    let alert_row = match (wide, border_headings) {
        (true, true) => first,
        (true, false) => top + 1,
        (false, _) => first + measurement_feeds.len().div_ceil(columns) as u16,
    };
    if alert_row >= bottom {
        let alert_stale = alert_sample.stale(alert_feed, now);
        match recent.as_deref() {
            None => status = format!(" alerts waiting ·{status}"),
            Some([]) => {
                let stale = if alert_stale { " stale" } else { "" };
                status = format!(" {NO_ALERTS}{stale} ·{status}");
            }
            Some(_) if alert_stale => status = format!(" alerts stale ·{status}"),
            Some(_) => {}
        }
    }
    if let Some((feed, _)) = FEEDS[..FLUX_HISTORY]
        .iter()
        .zip(&data.samples)
        .find(|(_, sample)| sample.failure(now).is_some())
    {
        status = format!(" {} failed ·{}", feed.label, status);
    }
    ribbon::panel_top(
        frame,
        (0, top, width),
        (
            if border_headings {
                " space weather · NOAA SWPC · conditions "
            } else {
                " space weather · NOAA SWPC "
            },
            &status,
        ),
        appearance,
        ribbon,
    );
    border(
        frame,
        (0, bottom, width),
        false,
        "",
        " data: NOAA Space Weather Prediction Center ",
        appearance,
    );
    for y in top + 1..bottom {
        sides(frame, 0, y, width, appearance);
    }

    if border_headings {
        put(
            frame,
            alert_x - 1,
            top,
            8,
            " alerts ",
            appearance.style(Role::Title),
        );
    } else if wide {
        put(
            frame,
            2,
            top + 1,
            left,
            "conditions",
            appearance.style(Role::Title),
        );
    }

    for (index, feed) in measurement_feeds.iter().enumerate() {
        let y = first + (index / columns) as u16;
        if y >= bottom {
            break;
        }
        let column = index % columns;
        // In the two-row stacked layout, give the charts and wind field
        // the widest cells.
        let (offset, room) = if !wide && columns == 3 {
            let extra = usize::from(left) - 97;
            let widths = if index < columns {
                [28, 39 + extra, 28]
            } else {
                [31 + extra, 28, 36]
            };
            (
                widths[..column].iter().sum::<usize>() + column,
                widths[column],
            )
        } else {
            let width = usize::from(left) / columns;
            (column * width, width - usize::from(columns > 1))
        };
        let mut row = Row::new(frame, 2 + offset as u16, y, room as u16);
        let sample = &data.samples[index];
        if let Some((values, _)) = &sample.good {
            measurement(
                &mut row,
                values,
                appearance,
                sample.stale(*feed, now),
                compact,
                utc_now,
            );
        } else {
            row.ui(
                &format!("{} waiting", feed.label),
                appearance.style(Role::Dim),
            );
        }
        show_failure(&mut row, sample, now, appearance);
    }

    if alert_row < bottom {
        let area = Rect::new(alert_x, alert_row, width - 1 - alert_x, bottom - alert_row);
        alerts(
            frame,
            area,
            m,
            now,
            utc_now,
            !border_headings,
            recent.as_deref(),
        );
    }
}

pub(super) fn countdown(wait: Duration) -> String {
    let seconds = wait.as_secs() + u64::from(wait.subsec_nanos() != 0);
    age(Duration::from_secs(seconds))
}

pub(super) fn kp_role(kp: f64) -> Role {
    if kp >= 5.0 {
        Role::Bad
    } else if kp >= 4.0 {
        Role::Warning
    } else {
        Role::Text
    }
}

fn decimal(value: f64) -> String {
    format!("{value:.1}").trim_end_matches(".0").to_owned()
}

pub(super) fn stamp(time: i64, utc_now: i64) -> String {
    let clock = format!("{:02}:{:02} UTC", time / 3600 % 24, time / 60 % 60);
    if time / DAY == utc_now / DAY {
        clock
    } else {
        let (_, month, day) = utc_parts(time as u64);
        format!("{month:02}-{day:02} {clock}")
    }
}

fn measurement(
    row: &mut Row<'_, '_>,
    data: &Data,
    a: Appearance,
    stale: bool,
    compact: bool,
    utc_now: i64,
) {
    let style = |role| a.style(if stale { Role::Dim } else { role });
    let (time, chart) = match data {
        Data::SolarFlux(value) => {
            row.ui(&format!("SFI {}", decimal(value.flux)), style(Role::Text));
            (value.time, Vec::new())
        }
        Data::PlanetaryK(values) => {
            let value = values.last().expect("validated nonempty Kp");
            row.ui("Kp ", style(Role::Text));
            row.ui(&format!("{:.1}", value.kp), style(kp_role(value.kp)));
            row.ui(
                &format!(" A {}", decimal(value.a_running)),
                style(Role::Text),
            );
            let chart = values
                .iter()
                .filter(|sample| sample.time > value.time - DAY)
                .map(|sample| (sample.kp * 100.0).round() as u64)
                .collect();
            (value.time, chart)
        }
        Data::Scales(value) => {
            row.ui(
                &format!("R{} S{} G{}", value.r, value.s, value.g),
                style(Role::Text),
            );
            (value.time, Vec::new())
        }
        Data::Xrays(values) => {
            let value = values.last().expect("validated nonempty X-rays");
            row.ui(
                &format!("X-ray {}", qso_graph_spaceweather::xray_class(value.flux)),
                style(Role::Text),
            );
            let logs: Vec<_> = values
                .iter()
                .filter(|sample| sample.time > value.time - 21_600)
                .map(|sample| sample.flux.log10())
                .collect();
            let min = logs.iter().copied().fold(f64::INFINITY, f64::min);
            let chart = logs
                .into_iter()
                .map(|value| ((value - min) * 1e6) as u64 + 1)
                .collect();
            (value.time, chart)
        }
        Data::WindSpeed(value) => {
            row.ui(
                &format!("Wind {}", decimal(value.proton_speed)),
                style(Role::Text),
            );
            if !compact {
                row.ui(" km/s", style(Role::Text));
            }
            (value.time, Vec::new())
        }
        Data::WindField(value) => {
            let role = if value.bz_gsm <= -10.0 {
                Role::Bad
            } else if value.bz_gsm < 0.0 {
                Role::Warning
            } else {
                Role::Text
            };
            row.ui(&format!("Bt {} Bz ", decimal(value.bt)), style(Role::Text));
            row.ui(&decimal(value.bz_gsm), style(role));
            if !compact {
                row.ui(" nT", style(Role::Text));
            }
            (value.time, Vec::new())
        }
        Data::Alerts { .. } | Data::SolarFluxHistory(_) => return,
    };
    row.ui(&format!(" {}", stamp(time, utc_now)), a.style(Role::Dim));
    if stale {
        row.ui(" stale", a.style(Role::Dim));
    }
    let cells = usize::from(row.end.saturating_sub(row.x))
        .saturating_sub(1)
        .min(chart.len());
    if cells > 0 {
        row.ui(" ", a.style(Role::Dim));
        let samples: Vec<_> = (0..cells)
            .map(|i| {
                let bucket = &chart[i * chart.len() / cells..(i + 1) * chart.len() / cells];
                bucket.iter().copied().max().unwrap_or(0)
            })
            .collect();
        spark(row, &samples, a, stale);
    }
}

fn recent_alerts(sample: &Sample, utc_now: i64) -> Option<Vec<(&Alert, usize)>> {
    let Some((Data::Alerts { values, groups }, _)) = &sample.good else {
        return None;
    };
    let window = utc_now - 5 * DAY..=utc_now;
    let recent = values
        .iter()
        .zip(groups)
        .filter(|(alert, _)| window.contains(&alert.issue_time));
    let mut counts = vec![0; values.len()];
    for (_, &group) in recent.clone() {
        counts[group] += 1;
    }
    // QGDXC-SWX-0006 puts the newest issue first, including within a second.
    Some(
        recent
            .filter_map(|(alert, &group)| {
                let count = std::mem::take(&mut counts[group]);
                (count > 0).then_some((alert, count))
            })
            .collect(),
    )
}

fn alerts(
    frame: &mut Frame<'_>,
    area: Rect,
    m: &Model,
    now: Duration,
    utc_now: i64,
    heading: bool,
    recent: Option<&[(&Alert, usize)]>,
) {
    let feed = FEEDS[ALERTS];
    let sample = &m.weather_data.samples[ALERTS];
    let a = m.appearance;
    let stale = sample.stale(feed, now);
    let role = if stale || recent.is_none() {
        Role::Dim
    } else {
        Role::Text
    };
    let style = a.style(role);
    let mut row = Row::new(frame, area.x, area.y, area.width);
    if heading {
        row.ui(
            "alerts",
            a.style(if stale { Role::Dim } else { Role::Title }),
        );
    }
    let status = match recent {
        None => Some("waiting"),
        Some([]) => Some(NO_ALERTS),
        Some(_) => None,
    };
    if let Some(text) = status {
        if heading {
            row.ui(" ", style);
        }
        row.ui(text, style);
    }
    if stale {
        row.ui(
            if heading || status.is_some() {
                " stale"
            } else {
                "stale"
            },
            style,
        );
    }
    show_failure(&mut row, sample, now, a);
    let start = area.y + u16::from(row.x > area.x);
    for (&(alert, count), y) in recent.into_iter().flatten().zip(start..area.bottom()) {
        let mut row = Row::new(frame, area.x, y, area.width);
        row.safe(
            &alert.product_id,
            alert.product_id.len(),
            false,
            style,
            false,
        );
        if count >= 2 {
            row.ui(&format!(" ×{count}"), style);
        }
        row.ui(&format!(" {}", stamp(alert.issue_time, utc_now)), style);
        if let Some(headline) = &alert.headline {
            row.ui(" ", style);
            row.safe(headline, usize::from(row.end - row.x), false, style, true);
        }
    }
}

fn show_failure(row: &mut Row<'_, '_>, sample: &Sample, now: Duration, a: Appearance) {
    if let Some(why) = sample.failure(now) {
        row.ui(&format!(" · {why}"), a.style(Role::Warning));
    }
}
