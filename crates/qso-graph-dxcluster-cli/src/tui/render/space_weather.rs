// SPDX-License-Identifier: GPL-3.0-or-later
//! NOAA SWPC space-weather view (QGDXC-TUI-0114).

use super::{Appearance, Frame, Model, Role, Row, age, border, sides, spark, utc};
use crate::tui::weather::{Data, FEEDS, Sample};
use std::time::Duration;

pub(super) fn draw(frame: &mut Frame<'_>, (top, bottom): (u16, u16), m: &Model, now: Duration) {
    let width = frame.area().width;
    let appearance = m.appearance;
    let data = &m.weather_data;
    let mut status = if now < data.pause {
        format!(" Retry-After {} ", countdown(data.pause - now))
    } else {
        format!(
            " next refresh {} ",
            countdown(data.next.saturating_sub(now))
        )
    };
    let rows = (bottom - top - 1) as usize;
    let feeds = FEEDS.len() - 1; // Alerts follow the six measurements.
    let columns = feeds.div_ceil(rows.max(1));
    let compact = columns > 1;
    let alert_row = top + 1 + feeds.div_ceil(columns) as u16;
    if alert_row >= bottom {
        let alerts = &data.samples[feeds];
        let alert_feed = FEEDS[feeds];
        if alerts.good.is_none() {
            status = format!(" Alerts waiting ·{status}");
        } else if alerts.stale(alert_feed, now) {
            status = format!(" Alerts stale ·{status}");
        }
    }
    if let Some((feed, _)) = FEEDS
        .iter()
        .zip(&data.samples)
        .find(|(_, sample)| sample.failure(now).is_some())
    {
        status = format!(" {} failed ·{}", feed.label, status);
    }
    border(
        frame,
        (0, top, width),
        true,
        " space weather · NOAA SWPC ",
        &status,
        appearance,
    );
    border(
        frame,
        (0, bottom, width),
        false,
        " W list ",
        " data: NOAA Space Weather Prediction Center ",
        appearance,
    );
    for y in top + 1..bottom {
        sides(frame, 0, y, width, appearance);
    }

    let available = (width - 3) as usize;
    for (index, feed) in FEEDS[..feeds].iter().enumerate() {
        let y = top + 1 + (index / columns) as u16;
        if y >= bottom {
            break;
        }
        let column = index % columns;
        // At the minimum wide height, give the two charts space alongside
        // the longest formatted values, timestamps and stale markers.
        let (offset, room) = if columns == 3 {
            let extra = available - 97;
            let widths = if index < columns {
                [26, 41 + extra, 28]
            } else {
                [32 + extra, 28, 35]
            };
            (
                widths[..column].iter().sum::<usize>() + column,
                widths[column],
            )
        } else {
            let width = available / columns;
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
        alerts(frame, alert_row..bottom, m, now, width, appearance);
    }
}

fn countdown(wait: Duration) -> String {
    let seconds = wait.as_secs() + u64::from(wait.subsec_nanos() != 0);
    age(Duration::from_secs(seconds))
}

fn decimal(value: f64) -> String {
    format!("{value:.1}").trim_end_matches(".0").to_owned()
}

fn stamp(time: i64, compact: bool) -> String {
    let text = utc(time as u64);
    if compact { text[12..].to_owned() } else { text }
}

fn measurement(row: &mut Row<'_, '_>, data: &Data, a: Appearance, stale: bool, compact: bool) {
    let style = |role| a.style(if stale { Role::Dim } else { role });
    let (time, chart) = match data {
        Data::SolarFlux(value) => {
            row.ui(&format!("SFI {}", decimal(value.flux)), style(Role::Text));
            (value.time, Vec::new())
        }
        Data::PlanetaryK(values) => {
            let value = values.last().expect("validated nonempty Kp");
            let role = if value.kp >= 5.0 {
                Role::Bad
            } else if value.kp >= 4.0 {
                Role::Warning
            } else {
                Role::Text
            };
            row.ui("Kp ", style(Role::Text));
            row.ui(&format!("{:.1}", value.kp), style(role));
            row.ui(
                &format!(" A {}", decimal(value.a_running)),
                style(Role::Text),
            );
            let chart = values
                .iter()
                .filter(|sample| sample.time > value.time - 86_400)
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
        Data::Alerts(_) => return,
    };
    row.ui(&format!(" {}", stamp(time, compact)), a.style(Role::Dim));
    if stale {
        row.ui(" stale", a.style(Role::Dim));
    }
    let cells = usize::from(row.end.saturating_sub(row.x))
        .saturating_sub(1)
        .min(24)
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

fn alerts(
    frame: &mut Frame<'_>,
    mut rows: std::ops::Range<u16>,
    m: &Model,
    now: Duration,
    width: u16,
    a: Appearance,
) {
    let feed = *FEEDS.last().expect("alerts feed");
    let sample = m.weather_data.samples.last().expect("alerts sample");
    let stale = sample.stale(feed, now);
    let role = if stale { Role::Dim } else { Role::Text };
    let y = rows.next().expect("room for alerts heading");
    let mut row = Row::new(frame, 2, y, width - 3);
    let values = match &sample.good {
        Some((Data::Alerts(values), _)) => {
            row.ui(
                if values.is_empty() {
                    "Alerts none"
                } else {
                    "Alerts"
                },
                a.style(role),
            );
            if stale {
                row.ui(" stale", a.style(Role::Dim));
            }
            values.as_slice()
        }
        _ => {
            row.ui("Alerts waiting", a.style(Role::Dim));
            &[]
        }
    };
    show_failure(&mut row, sample, now, a);
    for (alert, y) in values.iter().zip(rows) {
        let mut row = Row::new(frame, 2, y, width - 3);
        let info = format!("{} {}", alert.product_id, stamp(alert.issue_time, false));
        row.safe(&info, info.len(), false, a.style(role), false);
        if let Some(headline) = &alert.headline {
            row.ui(" ", a.style(role));
            row.safe(
                headline,
                usize::from(row.end - row.x),
                false,
                a.style(role),
                true,
            );
        }
    }
}

fn show_failure(row: &mut Row<'_, '_>, sample: &Sample, now: Duration, a: Appearance) {
    if let Some(why) = sample.failure(now) {
        row.ui(&format!(" · {why}"), a.style(Role::Warning));
    }
}
