// SPDX-License-Identifier: GPL-3.0-or-later
//! Minute-granularity astronomy for the dashboard and map (QGDXC-TUI-0142–0144).

use qso_graph_astro::{
    AllDay, D_LAYER, HORIZON, MapLines, Moment, Position, Span, moon_phase, moon_position,
    sublunar_point, subsolar_point, sun_crossings, terminator,
};
use std::cell::{Ref, RefCell};

#[derive(Default)]
pub(super) struct Cache {
    place: Option<Position>,
    entry: RefCell<Option<(u64, Option<Figures>)>>,
}

impl Cache {
    pub(super) fn new(place: Option<Position>) -> Self {
        Self {
            place,
            ..Self::default()
        }
    }

    pub(super) fn place(&self) -> Option<Position> {
        self.place
    }

    pub(super) fn at(&self, utc_seconds: u64) -> Ref<'_, Option<Figures>> {
        let minute = utc_seconds / 60;
        if self
            .entry
            .borrow()
            .as_ref()
            .is_none_or(|(cached, _)| *cached != minute)
        {
            let figures = i64::try_from(minute)
                .ok()
                .and_then(|minute| minute.checked_mul(60_000))
                .and_then(|ms| Moment::from_unix_millis(ms).ok())
                .map(|moment| Figures::new(moment, self.place));
            self.entry.replace(Some((minute, figures)));
        }
        Ref::map(self.entry.borrow(), |entry| {
            &entry.as_ref().expect("minute cached").1
        })
    }
}

pub(super) struct Figures {
    pub(super) station: Option<Station>,
    pub(super) horizon: MapLines,
    pub(super) d_layer: MapLines,
    pub(super) sun: Position,
    pub(super) moon: Position,
}

impl Figures {
    fn new(moment: Moment, place: Option<Position>) -> Self {
        Self {
            station: place.and_then(|place| Station::new(moment, place)),
            horizon: terminator(moment, HORIZON, 2.0).expect("fixed terminator parameters"),
            d_layer: terminator(moment, D_LAYER, 2.0).expect("fixed terminator parameters"),
            sun: subsolar_point(moment),
            moon: sublunar_point(moment),
        }
    }
}

pub(super) struct Station {
    pub(super) sun: String,
    pub(super) moon: String,
}

impl Station {
    fn new(moment: Moment, place: Position) -> Option<Self> {
        let span = Span::new(moment, 1440).ok()?;
        let sun = sun_crossings(span, place, HORIZON);
        let sun = match sun.all_day {
            Some(AllDay::Above) => " up all day".into(),
            Some(AllDay::Below) => " down all day".into(),
            None => format!(" ↑{} ↓{}", time(sun.rising), time(sun.setting)),
        };
        let fraction = (moon_phase(moment).fraction * 100.0 + 0.5).floor() as u8;
        let elevation = (moon_position(moment, place).elevation + 0.5).floor() as i16;
        let elevation = if elevation == 0 {
            "0".into()
        } else {
            format!("{elevation:+}")
        };
        Some(Self {
            sun,
            moon: format!(" {fraction}% {elevation}°"),
        })
    }
}

fn time(moment: Option<Moment>) -> String {
    moment.map_or_else(
        || "—".into(),
        |moment| {
            let minute = moment.unix_millis().div_euclid(60_000).rem_euclid(1440);
            format!("{:02}:{:02}", minute / 60, minute % 60)
        },
    )
}
