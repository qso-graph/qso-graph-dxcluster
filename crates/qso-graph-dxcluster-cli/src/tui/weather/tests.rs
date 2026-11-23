// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use crate::tui::web::{
    USER_AGENT,
    test_fetch::{FakeClock, Script},
};
use crate::tui::{
    config::Options,
    model::{Input, Model, SpotsView},
    present,
    theme::Role,
};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

const FIXTURES: [&str; FLUX_HISTORY] = [
    "products_summary_10cm-flux.json",
    "products_noaa-planetary-k-index.json",
    "products_noaa-scales.json",
    "json_goes_primary_xrays-6-hour.json",
    "products_summary_solar-wind-speed.json",
    "products_summary_solar-wind-mag-field.json",
    "products_alerts.json",
];
const FLUX_FIXTURE: &str = "json_f107_cm_flux.json";

fn fixture(name: &str) -> Vec<u8> {
    let directory = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../qso-graph-spaceweather/tests/fixtures",
    );
    std::fs::read(std::path::Path::new(directory).join(name)).unwrap()
}

fn fixture_script() -> Script {
    Script {
        responses: FIXTURES.map(|name| Outcome::Bytes(fixture(name))).into(),
        ..Script::default()
    }
}

fn collect_fixtures() -> (Cache, Scheduler, Script, FakeClock) {
    let mut cache = Cache::default();
    let mut scheduler = Scheduler::default();
    let mut fetch = fixture_script();
    let clock = FakeClock::default();
    for _ in FIXTURES {
        cache.apply(scheduler.step(&mut fetch, &clock).unwrap());
    }
    (cache, scheduler, fetch, clock)
}

#[test]
fn floors_and_failures_keep_good_values() {
    // QGDXC-TUI-0111, QGDXC-TUI-0113 and QGDXC-TUI-0116.
    let (mut cache, mut scheduler, mut fetch, clock) = collect_fixtures();
    assert_eq!(
        fetch.calls,
        FEEDS[..FLUX_HISTORY]
            .iter()
            .map(|feed| (feed.url, feed.product.limit()))
            .collect::<Vec<_>>()
    );
    assert!(
        cache.samples[..FLUX_HISTORY]
            .iter()
            .all(|sample| sample.good.is_some())
    );
    assert_eq!(cache.next, FIVE_MINUTES);
    clock.at(299);
    assert!(scheduler.step(&mut fetch, &clock).is_none());
    fetch.responses.extend([
        Outcome::Bytes(vec![0; Product::Xrays.limit() + 1]),
        Outcome::Error("request failed"),
        Outcome::Status(503, None),
        Outcome::Bytes(b"bad JSON".to_vec()),
    ]);
    clock.at(300);
    for _ in 0..4 {
        cache.apply(scheduler.step(&mut fetch, &clock).unwrap());
    }
    assert_eq!(cache.pause, Duration::ZERO);
    for (sample, expected) in cache.samples[3..].iter().zip([
        "byte limit exceeded",
        "request failed",
        "HTTP 503",
        "invalid JSON",
    ]) {
        assert_eq!(sample.failure(clock.elapsed()), Some(expected));
        assert_eq!(sample.good.as_ref().unwrap().1, Duration::ZERO);
        assert!(sample.failure(clock.elapsed() + FAILURE_NOTICE).is_none());
    }
    clock.at(599);
    assert!(scheduler.step(&mut fetch, &clock).is_none());
    clock.at(600);
    fetch.responses.extend(
        FIXTURES[3..]
            .iter()
            .map(|name| Outcome::Bytes(fixture(name))),
    );
    for _ in 0..4 {
        cache.apply(scheduler.step(&mut fetch, &clock).unwrap());
    }
    assert_eq!(cache.samples[3].good.as_ref().unwrap().1, clock.elapsed());
    assert!(cache.samples[3].failure(clock.elapsed()).is_none());
    clock.at(899);
    assert!(scheduler.step(&mut fetch, &clock).is_none());
    clock.at(900);
    fetch
        .responses
        .push_back(Outcome::Bytes(fixture(FIXTURES[0])));
    cache.apply(scheduler.step(&mut fetch, &clock).unwrap());
    assert_eq!(fetch.calls.last().unwrap().0, FEEDS[0].url);
    assert_eq!(cache.samples[0].good.as_ref().unwrap().1, clock.elapsed());
}

#[test]
fn retry_after_pauses_all_feeds_and_preserves_their_floors() {
    // QGDXC-TUI-0113.
    for (status, header, wait) in [
        (429, None, 60),
        (429, Some("5"), 60),
        (429, Some("bad"), 60),
        (503, Some(""), 60),
        (503, Some("200"), 200),
        (503, Some("999999999999999999999"), 3_600),
        (503, Some("Sun, 22 Nov 2026 19:43:00 GMT"), 120),
        (503, Some("Sunday, 22-Nov-26 19:43:00 GMT"), 120),
        (503, Some("Sun Nov 22 19:43:00 2026"), 120),
    ] {
        let clock = FakeClock::default();
        let mut fetch = Script {
            responses: [
                Outcome::Status(status, header.map(str::to_owned)),
                Outcome::Bytes(fixture(FIXTURES[1])),
            ]
            .into(),
            ..Script::default()
        };
        let mut scheduler = Scheduler::default();
        let mut cache = Cache::default();
        cache.apply(scheduler.step(&mut fetch, &clock).unwrap());
        assert_eq!(cache.pause, Duration::from_secs(wait));
        assert_eq!(cache.next, cache.pause);
        clock.at(wait - 1);
        assert!(scheduler.step(&mut fetch, &clock).is_none());
        assert_eq!(fetch.calls.len(), 1);
        clock.at(wait);
        cache.apply(scheduler.step(&mut fetch, &clock).unwrap());
        assert_eq!(fetch.calls[1].0, FEEDS[1].url);
    }
}

#[test]
fn flux_chart_activates_history_once_with_its_own_floor() {
    // QGDXC-TUI-0123. Check all start times against the original per-feed floors.
    let mut scheduler = Scheduler::default();
    let clock = FakeClock::default();
    let mut fetch = Script {
        responses: std::iter::repeat_with(|| Outcome::Error("request failed"))
            .take(200)
            .collect(),
        ..Script::default()
    };
    let mut starts: [Vec<u64>; FEEDS.len()] = Default::default();
    let mut m = weather_model();
    install_layout_alerts(&mut m, 9);
    for second in 0..=7_600 {
        clock.at(second);
        if [399, 400, 401, 3_999].contains(&second)
            && present(
                &mut Terminal::new(TestBackend::new(80, if second == 399 { 24 } else { 25 }))
                    .unwrap(),
                &m,
                clock.elapsed(),
            )
            .unwrap()
        {
            scheduler.activate_history(clock.elapsed());
        }
        while let Some(update) = scheduler.step(&mut fetch, &clock) {
            starts[update.index].push(second);
        }
    }
    for (starts, floor) in starts[..FLUX_HISTORY]
        .iter()
        .zip([900, 900, 900, 300, 300, 300, 300])
    {
        assert_eq!(*starts, (0..=7_600).step_by(floor).collect::<Vec<_>>());
    }
    assert_eq!(starts[FLUX_HISTORY], [400, 4_000, 7_600]);
}

#[test]
fn history_request_obeys_the_shared_retry_after() {
    let mut scheduler = Scheduler::default();
    let clock = FakeClock::default();
    let mut fetch = fixture_script();
    for _ in FIXTURES {
        scheduler.step(&mut fetch, &clock).unwrap();
    }
    fetch
        .responses
        .push_back(Outcome::Status(429, Some("60".to_owned())));
    clock.at(300);
    scheduler.step(&mut fetch, &clock).unwrap();
    clock.at(301);
    scheduler.activate_history(clock.elapsed());
    clock.at(359);
    assert!(scheduler.step(&mut fetch, &clock).is_none());
    fetch.responses.extend(
        FIXTURES[4..]
            .iter()
            .map(|name| Outcome::Bytes(fixture(name))),
    );
    fetch
        .responses
        .push_back(Outcome::Bytes(fixture(FLUX_FIXTURE)));
    clock.at(360);
    for _ in 0..3 {
        assert_ne!(
            scheduler.step(&mut fetch, &clock).unwrap().index,
            FLUX_HISTORY
        );
    }
    let update = scheduler.step(&mut fetch, &clock).unwrap();
    assert_eq!(update.index, FLUX_HISTORY);
    assert!(matches!(update.data(), Some(Data::SolarFluxHistory(_))));
}

#[test]
fn https_uses_only_the_fixed_feed_host() {
    // QGDXC-TUI-0112. The shared crate tests the client and body reader.
    let client = Https::new(NOAA_HOST, FEEDS.map(|feed| feed.url));
    assert!(client.allows_host("services.swpc.noaa.gov"));
    assert!(!client.allows_host("example.invalid"));
    assert!(
        FEEDS
            .iter()
            .all(|feed| feed.url.starts_with("https://services.swpc.noaa.gov/"))
    );
    assert_eq!(USER_AGENT, format!("qg-dxc/{}", env!("CARGO_PKG_VERSION")));
}

#[test]
fn noaa_policy_matches_the_annex_i_fixture() {
    assert_eq!(
        noaa_policy(),
        Policy {
            floors: FEEDS
                .iter()
                .zip([900, 900, 900, 300, 300, 300, 300, 3_600])
                .map(|(feed, seconds)| (feed.url.to_owned(), Duration::from_secs(seconds)))
                .collect(),
            wait_scope: WaitScope::Provider,
            retry_after: RetryAfter {
                minimum: Duration::from_secs(60),
                maximum: Duration::from_secs(3_600),
                default: Some(Duration::from_secs(60)),
                server_errors: RetryAfterStatuses::ServiceUnavailable,
            },
            backoff: None,
        },
    );
}

#[test]
fn worker_starts_once_on_opt_in_and_keeps_polling() {
    // QGDXC-TUI-0111 and QGDXC-TUI-0116.
    let mut worker = Worker::default();
    let mut cache = Cache::default();
    worker
        .poll(
            false,
            false,
            |_| panic!("unexpected update"),
            || panic!("started before opt-in"),
        )
        .unwrap();
    let (send, updates) = mpsc::sync_channel(1);
    let (control, commands) = mpsc::channel();
    worker
        .poll(
            true,
            false,
            |update| cache.apply(update),
            || {
                Ok(Running {
                    control,
                    history_requested: false,
                    updates,
                })
            },
        )
        .unwrap();
    let update = Scheduler::default()
        .step(&mut fixture_script(), &FakeClock::default())
        .unwrap();
    send.send(update).unwrap();
    worker
        .poll(
            false,
            false,
            |update| cache.apply(update),
            || panic!("started a second worker"),
        )
        .unwrap();
    assert!(cache.samples[0].good.is_some());
    assert_eq!(commands.try_recv(), Err(mpsc::TryRecvError::Empty));
}

#[test]
fn worker_requests_flux_history_only_when_the_chart_is_drawn() {
    // QGDXC-TUI-0123.
    let mut m = weather_model();
    install_layout_alerts(&mut m, 9);
    let mut worker = Worker::default();
    let (_send, updates) = mpsc::sync_channel(1);
    let (control, commands) = mpsc::channel();
    let mut running = Some(Running {
        control,
        history_requested: false,
        updates,
    });
    for (height, key, expected_draw, expected_request) in [
        (24, None, false, false),
        (25, Some(Input::Char('?')), false, false),
        (25, Some(Input::Escape), true, true),
        (25, None, true, false),
    ] {
        if let Some(key) = key {
            m.key(key);
        }
        let mut terminal = Terminal::new(TestBackend::new(80, height)).unwrap();
        let drawn = present(&mut terminal, &m, Duration::ZERO).unwrap();
        assert_eq!(drawn, expected_draw);
        assert_eq!(
            screen_text(terminal.backend().buffer()).contains(HISTORY_TITLES[0]),
            expected_draw
        );
        worker
            .poll(
                m.spots_view == SpotsView::Weather,
                drawn,
                |_| panic!("unexpected update"),
                || Ok(running.take().expect("started a second worker")),
            )
            .unwrap();
        assert_eq!(
            commands.try_recv(),
            if expected_request {
                Ok(())
            } else {
                Err(mpsc::TryRecvError::Empty)
            }
        );
    }
}

#[test]
fn space_weather_switches_with_other_views_and_is_listed() {
    // QGDXC-TUI-0110 and QGDXC-TUI-0111.
    let base = ["--call", "Q1ABC", "--node", "source.example.invalid:7300"];
    let args = |opt: bool| {
        base.into_iter()
            .chain(opt.then_some("--space-weather"))
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    assert!(!Options::parse(&args(false)).unwrap().space_weather);
    assert!(Options::parse(&args(true)).unwrap().space_weather);
    let mut m = Model::new(vec!["source".into()], 100, 100);
    let key = |m: &mut Model, ch| m.key(Input::Char(ch));
    assert_eq!(m.spots_view, SpotsView::List);
    let list = screen(&m, 130, 24, Duration::ZERO);
    assert!(row(&list, 23).contains("W weather"));
    key(&mut m, '/');
    key(&mut m, 'W');
    assert_eq!(m.spots_view, SpotsView::List);
    m.key(Input::Escape);
    key(&mut m, 'W');
    assert_eq!(m.spots_view, SpotsView::Weather);
    let weather = screen(&m, 130, 24, Duration::ZERO);
    for y in 0..8 {
        assert_eq!(row(&list, y), row(&weather, y));
    }
    assert!(row(&weather, 23).contains("W list"));
    assert!(row(&screen(&m, 80, 24, Duration::ZERO), 23).contains(" B W ?"));
    key(&mut m, 'W');
    assert_eq!(m.spots_view, SpotsView::List);
    assert_eq!(screen(&m, 130, 24, Duration::ZERO), list);

    for (shortcut, view, title) in [
        ('M', SpotsView::Map, " map "),
        ('B', SpotsView::Beacons, "NCDXF/IARU beacons"),
    ] {
        key(&mut m, 'W');
        key(&mut m, shortcut);
        assert_eq!(m.spots_view, view);
        let other = screen(&m, 130, 24, Duration::ZERO);
        assert!(location(&other, title).1 < 23);
        key(&mut m, 'W');
        assert_eq!(m.spots_view, SpotsView::Weather);
        assert_eq!(screen(&m, 130, 24, Duration::ZERO), weather);
        key(&mut m, 'W');
    }
    m.spots_view = SpotsView::List;
    key(&mut m, '?');
    key(&mut m, 'W');
    assert_eq!(m.spots_view, SpotsView::List);
    assert_eq!(
        row(&screen(&m, 80, 24, Duration::ZERO), 18).trim_end(),
        "W: space weather"
    );
}

fn screen(m: &Model, width: u16, height: u16, now: Duration) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    present(&mut terminal, m, now).unwrap();
    terminal.backend().buffer().clone()
}

fn row(buffer: &Buffer, y: u16) -> String {
    (0..buffer.area.width)
        .map(|x| buffer[(x, y)].symbol())
        .collect()
}

fn screen_text(buffer: &Buffer) -> String {
    (0..buffer.area.height)
        .map(|y| row(buffer, y))
        .collect::<Vec<_>>()
        .join("\n")
}

fn location(buffer: &Buffer, text: &str) -> (u16, u16) {
    (0..buffer.area.height)
        .find_map(|y| {
            let line = row(buffer, y);
            let byte = line.find(text)?;
            Some((line[..byte].chars().count() as u16, y))
        })
        .unwrap_or_else(|| panic!("{text} missing from screen\n{}", screen_text(buffer)))
}

const WEATHER_EPOCH: u64 = 1_795_377_660; // 2026-11-22 20:01:00 UTC

fn weather_model() -> Model {
    let mut m = Model::new(vec!["source".into()], 100, 100);
    m.spots_view = SpotsView::Weather;
    m.utc_epoch = WEATHER_EPOCH;
    m
}

const HISTORY_TITLES: [&str; 4] = [
    "Solar flux (sfu) · 30 days",
    "Kp (index) · 7 days",
    "X-rays (W/m²) · 6 hours",
    "Solar wind (km/s, nT) · since start",
];

fn history_model() -> Model {
    let mut m = weather_model();
    let mut scheduler = Scheduler::default();
    scheduler.activate_history(Duration::ZERO);
    let mut fetch = fixture_script();
    fetch
        .responses
        .push_back(Outcome::Bytes(fixture(FLUX_FIXTURE)));
    let clock = FakeClock::default();
    for _ in FEEDS {
        m.apply_weather(scheduler.step(&mut fetch, &clock).unwrap());
    }
    m
}

#[test]
fn weather_charts_fill_the_rows_below_figures_and_alerts() {
    // QGDXC-TUI-0120 and QGDXC-TUI-0121.
    let mut m = history_model();
    let now = Duration::from_secs(600);
    for (width, height, alerts) in [(120, 40, 2), (160, 50, 8), (100, 40, 2)] {
        install_layout_alerts(&mut m, alerts);
        let grid = screen(&m, width, height, now);
        let [(fx, fy), (kx, ky), (xx, xy), (wx, wy)] =
            HISTORY_TITLES.map(|title| location(&grid, title));
        let figure_y = location(&grid, "Bt 2 Bz -2 nT").1;
        let alert_y = location(&grid, &format!("QX{alerts:02}")).1;
        assert_eq!(fy, figure_y.max(alert_y) + 1, "{}", screen_text(&grid));
        assert_eq!((fx, xx), (3, 3));
        let bottom = height - 2;
        if width >= 120 {
            assert_eq!((kx, wx), (width / 2 + 2, width / 2 + 2));
            assert_eq!(fy, ky);
            assert_eq!(xy, wy);
            assert!(xy - fy >= 4 && bottom - xy >= 4);
        } else {
            assert_eq!((kx, wx), (3, 3));
            assert!(ky - fy >= 4 && xy - ky >= 4 && wy - xy >= 4);
            assert!(bottom - wy >= 4);
        }
        let chart_bottoms = if width >= 120 {
            [xy, xy, bottom, bottom]
        } else {
            [ky, xy, wy, bottom]
        };
        for ((x, left, right), next) in [
            (fx, "10-23", "11-21"),
            (kx, "11-15", "11-22"),
            (xx, "13:40", "19:40"),
            (wx, "20:01", "20:11"),
        ]
        .into_iter()
        .zip(chart_bottoms)
        {
            let room = if width >= 120 {
                width / 2 - 1
            } else {
                width - 2
            };
            let axis: String = row(&grid, next - 1)
                .chars()
                .skip(usize::from(x - 2))
                .take(usize::from(room))
                .collect();
            assert!(
                axis.contains(left) && axis.contains(right) && axis.contains(" UTC "),
                "{axis}"
            );
        }
    }
}

#[test]
fn weather_charts_show_only_what_fits_on_short_screens() {
    // QGDXC-TUI-0121.
    let mut m = history_model();
    for (width, height, alerts, chart_count) in [(80, 24, 2, 2), (80, 24, 9, 0), (120, 24, 8, 2)] {
        install_layout_alerts(&mut m, alerts);
        let grid = screen(&m, width, height, Duration::ZERO);
        let text = screen_text(&grid);
        let figure_y = location(&grid, "Bt 2 Bz -2 nT").1;
        let alert_y = location(&grid, &format!("QX{alerts:02}")).1;
        if width < 120 {
            assert!(alert_y > figure_y);
        }
        for (i, title) in HISTORY_TITLES.into_iter().enumerate() {
            assert_eq!(text.contains(title), i < chart_count, "{text}");
        }
        if chart_count > 0 {
            let (_, flux_y) = location(&grid, HISTORY_TITLES[0]);
            let (_, kp_y) = location(&grid, HISTORY_TITLES[1]);
            assert_eq!(flux_y, figure_y.max(alert_y) + 1);
            if width < 120 {
                assert!(kp_y - flux_y >= 4);
            } else {
                assert_eq!(kp_y, flux_y);
            }
            assert!(height - 2 - kp_y >= 4);
        }
    }
}

#[test]
fn weather_kp_scale_labels_are_left_of_the_bars() {
    // QGDXC-TUI-0122. Check the full scale and minimum-height fallback.
    let mut m = history_model();
    install_layout_alerts(&mut m, 8);
    for (width, height, label_order) in [(120, 40, "9530"), (120, 24, "950")] {
        let grid = screen(&m, width, height, Duration::ZERO);
        let (x, top) = location(&grid, HISTORY_TITLES[1]);
        let (_, bottom) = location(&grid, "11-15");
        let scale: String = (top + 1..=bottom)
            .map(|y| grid[(x - 1, y)].symbol())
            .filter(|symbol| *symbol != " ")
            .collect();
        assert_eq!(scale, label_order, "{}", screen_text(&grid));
        assert!((x..width - 2).any(|x| (top + 1..bottom).any(|y| {
            grid[(x, y)]
                .symbol()
                .starts_with(|ch| ('▁'..='█').contains(&ch))
        })));
    }
}

#[test]
fn history_kp_bars_use_noaa_storm_colours() {
    // QGDXC-TUI-0122. Inspect the bars in the rendered grid.
    let mut m = history_model();
    let (feed, _) = m.weather_data.sample(Product::PlanetaryK);
    let index = FEEDS
        .iter()
        .position(|f| f.product == feed.product)
        .unwrap();
    let Some((Data::PlanetaryK(values), _)) = &mut m.weather_data.samples[index].good else {
        panic!("Kp fixture was missing");
    };
    let time = values.last().unwrap().time;
    let kps = [3.0, 4.67, 6.0, 7.0, 8.67, 9.0];
    *values = kps
        .into_iter()
        .enumerate()
        .map(|(i, kp)| swx::PlanetaryK {
            time: time - (kps.len() as i64 - 1 - i as i64) * 10_800,
            kp,
            a_running: 0.0,
        })
        .collect();
    let grid = screen(&m, 120, 40, Duration::ZERO);
    let (_, title_y) = location(&grid, HISTORY_TITLES[1]);
    let (_, utc_y) = location(&grid, "11-15");
    let bars: Vec<_> = (60..120)
        .filter_map(|x| {
            (title_y + 1..utc_y)
                .map(|y| &grid[(x, y)])
                .find(|cell| cell.symbol() == "█")
                .map(|cell| cell.fg)
        })
        .collect();
    assert_eq!(
        bars,
        [
            Role::KpQuiet,
            Role::G1,
            Role::G2,
            Role::G3,
            Role::G4,
            Role::G5
        ]
        .map(|role| m.appearance.color(role))
    );
}

#[test]
fn kp_levels_follow_noaa_thirds_and_single_hue_themes_bold_strong_storms() {
    // QGDXC-TUI-0122: nearest whole level, but only 9o (9.00) is G5.
    use super::super::render::space_weather::kp_role;
    for (kp, role) in [
        (0.0, Role::KpQuiet),
        (4.33, Role::KpQuiet),
        (4.67, Role::G1),
        (5.33, Role::G1),
        (5.67, Role::G2),
        (6.33, Role::G2),
        (6.67, Role::G3),
        (7.33, Role::G3),
        (7.67, Role::G4),
        (8.67, Role::G4),
        (9.0, Role::G5),
    ] {
        assert_eq!(kp_role(kp), role, "Kp {kp}");
    }
    use super::super::theme::{Appearance, Theme};
    use ratatui::style::Modifier;
    for theme in Theme::ALL {
        let a = Appearance {
            theme,
            ..Appearance::default()
        };
        let single_hue = matches!(theme, Theme::Phosphor | Theme::Amber | Theme::Mono);
        for role in [Role::G3, Role::G4, Role::G5] {
            assert_eq!(
                a.style(role).add_modifier.contains(Modifier::BOLD),
                single_hue,
                "{theme:?}"
            );
        }
        assert!(!a.style(Role::G2).add_modifier.contains(Modifier::BOLD));
    }
    // The colour theme gives each level its own colour.
    let a = Appearance::default();
    let colours: std::collections::BTreeSet<_> = [
        Role::KpQuiet,
        Role::G1,
        Role::G2,
        Role::G3,
        Role::G4,
        Role::G5,
    ]
    .map(|r| format!("{:?}", a.color(r)))
    .into_iter()
    .collect();
    assert_eq!(colours.len(), 6);
}

#[test]
fn history_xray_axis_labels_are_at_each_logarithmic_decade() {
    // QGDXC-TUI-0122. Labels on the y-axis read X through A from top to bottom.
    let mut m = history_model();
    install_layout_alerts(&mut m, 2);
    for (width, height) in [(120, 40), (100, 40)] {
        let grid = screen(&m, width, height, Duration::ZERO);
        let (_, title) = location(&grid, HISTORY_TITLES[2]);
        let bottom = if width == 120 {
            height - 3
        } else {
            location(&grid, HISTORY_TITLES[3]).1 - 1
        };
        for (i, label) in ["A", "B", "C", "M", "X"].into_iter().enumerate() {
            let y = bottom - 1 - i as u16 * (bottom - title - 2) / 4;
            assert_eq!(grid[(2, y)].symbol(), label, "{}", screen_text(&grid));
        }
    }
    install_alerts(&mut m, &[]);
    location(
        &screen(&m, 80, 24, Duration::ZERO),
        "X-rays (W/m²) · 6 hours · A B C M X",
    );
}

#[test]
fn history_waiting_stale_and_mono_are_visible_in_the_grid() {
    // QGDXC-TUI-0124.
    let m = weather_model();
    let grid = screen(&m, 120, 40, Duration::ZERO);
    for title in HISTORY_TITLES {
        let (x, y) = location(&grid, title);
        let interior = row(&grid, y + 1)
            .chars()
            .skip(x as usize)
            .take(56)
            .collect::<String>();
        assert!(interior.contains("waiting"));
    }
    let mut m = history_model();
    let grid = screen(&m, 120, 40, Duration::from_secs(10_800));
    for title in &HISTORY_TITLES[1..] {
        location(&grid, &format!("{title} · stale"));
    }
    assert!(!screen_text(&grid).contains("Solar flux (sfu) · 30 days · stale"));
    let grid = screen(&m, 120, 40, Duration::from_secs(10_801));
    for title in HISTORY_TITLES {
        let (x, y) = location(&grid, &format!("{title} · stale"));
        assert_eq!(grid[(x, y)].fg, m.appearance.color(Role::Dim));
    }
    m.appearance.theme = super::super::theme::Theme::initial(None, Some("1"));
    let fresh = screen(&m, 120, 40, Duration::ZERO);
    let (_, top) = location(&fresh, HISTORY_TITLES[0]);
    let (_, bottom) = location(&fresh, "10-23");
    let mut dim = Vec::new();
    for y in top + 1..bottom {
        for x in 1..59 {
            let cell = &fresh[(x, y)];
            if cell
                .symbol()
                .starts_with(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch))
            {
                dim.push(cell.modifier.contains(ratatui::style::Modifier::DIM));
            }
        }
    }
    assert!(dim.contains(&true) && dim.contains(&false));
    let grid = screen(&m, 120, 40, Duration::from_secs(10_801));
    assert!(
        grid.content
            .iter()
            .all(|cell| cell.fg == ratatui::style::Color::Reset)
    );
    let (x, y) = location(&grid, "Solar flux (sfu) · 30 days · stale");
    assert!(
        grid[(x, y)]
            .modifier
            .contains(ratatui::style::Modifier::DIM)
    );
}

#[test]
fn wind_history_retains_the_latest_288_successes_of_each_series() {
    // QGDXC-TUI-0122. Capture while the spots list is visible.
    let mut m = weather_model();
    m.spots_view = SpotsView::List;
    for i in 0..300 {
        for index in [4, 5] {
            let data = Data::parse(FEEDS[index].product, &fixture(FIXTURES[index])).unwrap();
            m.apply_weather(Update {
                index,
                complete: Duration::from_secs(i * 300),
                next: Duration::ZERO,
                pause: Duration::ZERO,
                result: Ok(data),
            });
        }
    }
    m.apply_weather(Update {
        index: 4,
        complete: Duration::from_secs(300 * 300),
        next: Duration::ZERO,
        pause: Duration::ZERO,
        result: Err("request failed".into()),
    });
    for (series, value) in [(&m.wind_history.speed, 343.0), (&m.wind_history.bz, -2.0)] {
        assert_eq!(series.len(), 288);
        assert_eq!(series.front(), Some(&(WEATHER_EPOCH + 12 * 300, value)));
        assert_eq!(series.back(), Some(&(WEATHER_EPOCH + 299 * 300, value)));
    }
    m.spots_view = SpotsView::Weather;
    let grid = screen(&m, 120, 26, Duration::from_secs(300 * 300));
    let (x, y) = location(&grid, HISTORY_TITLES[3]);
    for (label, y) in [("Speed", y + 1), ("Bz", y + 2)] {
        let series: String = row(&grid, y).chars().skip(usize::from(x - 1)).collect();
        assert!(series.starts_with(label), "{series}");
        assert!(
            series
                .chars()
                .any(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch))
        );
    }
    let axis = row(&grid, 23);
    assert!(axis.chars().skip(60).collect::<String>().contains("11-22"));
    assert!(axis.contains("11-23 ┘"));
}

fn install_alerts(m: &mut Model, alerts: &[(&str, &str, &str)]) {
    let items: Vec<_> = alerts
        .iter()
        .map(|(id, time, message)| {
            serde_json::json!({"product_id": id, "issue_datetime": time, "message": message})
        })
        .collect();
    let data = Data::parse(Product::Alerts, &serde_json::to_vec(&items).unwrap()).unwrap();
    m.weather_data.samples[ALERTS].good = Some((data, Duration::ZERO));
}

fn install_layout_alerts(m: &mut Model, count: usize) {
    let ids: Vec<_> = (1..=count).map(|i| format!("QX{i:02}")).collect();
    let alerts: Vec<_> = ids
        .iter()
        .map(|id| (id.as_str(), "2026-11-22 20:00:00", "SUMMARY: Example"))
        .collect();
    install_alerts(m, &alerts);
}

#[test]
fn ribbon_waits_for_alerts_and_expires_on_the_model_clock() {
    // QGDXC-TUI-0125.
    let mut m = weather_model();
    m.spots_view = SpotsView::List;
    for width in [80, 140] {
        assert!(!screen_text(&screen(&m, width, 24, Duration::ZERO)).contains('▌'));
    }
    install_alerts(&mut m, &[]);
    assert!(!screen_text(&screen(&m, 80, 24, Duration::ZERO)).contains('▌'));
    install_alerts(
        &mut m,
        &[(
            "QX20",
            "2026-11-22 20:02:00",
            "ALERT: Geomagnetic storm\nIssue Time: 2026 Nov 22 2002 UTC",
        )],
    );
    assert!(!screen_text(&screen(&m, 80, 24, Duration::ZERO)).contains('▌'));
    location(&screen(&m, 80, 24, Duration::from_secs(60)), "▌ALERT");
    assert!(!screen_text(&screen(&m, 80, 24, Duration::from_secs(3 * 3600 + 60))).contains('▌'));
}

#[test]
fn ribbon_displays_each_kind_with_its_role_and_utc_end() {
    // QGDXC-TUI-0125.
    let mut m = weather_model();
    m.spots_view = SpotsView::List;
    for (message, expected, role) in [
        (
            "ALERT: Geomagnetic storm\nIssue Time: 2026 Nov 22 1900 UTC",
            "▌ALERT Geomagnetic storm · until 22:00 UTC",
            Role::Bad,
        ),
        (
            "CONTINUED ALERT: Geomagnetic storm\nIssue Time: 2026 Nov 22 2000 UTC",
            "▌ALERT Geomagnetic storm · until 23:00 UTC",
            Role::Bad,
        ),
        (
            concat!(
                "WARNING: Geomagnetic storm\nIssue Time: 2026 Nov 22 1900 UTC\n",
                "Valid From: 2026 Nov 22 1900 UTC\nValid To: 2026 Nov 22 2200 UTC",
            ),
            "▌WARNING Geomagnetic storm · until 22:00 UTC",
            Role::Warning,
        ),
        (
            concat!(
                "EXTENDED WARNING: Geomagnetic storm\nIssue Time: 2026 Nov 22 2000 UTC\n",
                "Now Valid Until: 2026 Nov 23 0200 UTC\nExtension to Serial Number: 1",
            ),
            "▌WARNING Geomagnetic storm · until 11-23 02:00 UTC",
            Role::Warning,
        ),
        (
            concat!(
                "WATCH: Geomagnetic storm\nIssue Time: 2026 Nov 22 1900 UTC\n",
                "Highest Storm Level Predicted by Day:\nNov 23:  G1 (Minor)",
            ),
            "▌WATCH Geomagnetic storm · until 11-23 23:59 UTC",
            Role::Accent,
        ),
    ] {
        install_alerts(&mut m, &[("QX20", "2026-11-22 20:00:00", message)]);
        let grid = screen(&m, 180, 24, Duration::ZERO);
        let (x, y) = location(&grid, expected);
        let fg = m.appearance.style(role).fg.unwrap();
        assert!((x..x + expected.chars().count() as u16).all(|column| grid[(column, y)].fg == fg));
    }
}

#[test]
fn ribbon_uses_the_first_free_bands_row_or_the_top_border() {
    // QGDXC-TUI-0126.
    for sources in [1, 2, 3] {
        let mut m = Model::new(
            (1..=sources).map(|i| format!("source{i}")).collect(),
            100,
            100,
        );
        m.utc_epoch = WEATHER_EPOCH;
        install_alerts(
            &mut m,
            &[(
                "QX20",
                "2026-11-22 20:00:00",
                "ALERT: Geomagnetic storm\nIssue Time: 2026 Nov 22 2000 UTC",
            )],
        );
        let expected = if sources == 3 { (52, 7) } else { (59, 1) };
        for view in [SpotsView::List, SpotsView::Weather] {
            m.spots_view = view;
            let grid = screen(&m, 100, 24, Duration::ZERO);
            assert_eq!(location(&grid, "▌ALERT Geomagnetic"), expected);
            assert_eq!(grid[(99, 1)].symbol(), "╮");
            assert!(row(&grid, 6).chars().skip(50).any(|ch| ch == '0'));
        }
    }
}

#[test]
fn ribbon_is_compact_on_the_top_border_of_every_page() {
    // QGDXC-TUI-0125 and QGDXC-TUI-0126.
    let mut m = weather_model();
    install_alerts(
        &mut m,
        &[
            (
                "QX20",
                "2026-11-22 20:00:00",
                "ALERT: Geomagnetic storm\nIssue Time: 2026 Nov 22 2000 UTC",
            ),
            (
                "QX21",
                "2026-11-22 19:00:00",
                "ALERT: X-ray event\nIssue Time: 2026 Nov 22 1900 UTC",
            ),
        ],
    );
    for (page, title) in [
        (SpotsView::List, "spots newest first"),
        (SpotsView::Map, "map"),
        (SpotsView::Beacons, "NCDXF/IARU beacons"),
        (SpotsView::Weather, "space weather · NOAA SWPC"),
    ] {
        m.spots_view = page;
        let grid = screen(&m, 80, 24, Duration::ZERO);
        let (x, y) = location(&grid, "▌ALERT  +1");
        let (label, label_row) = location(&grid, title);
        assert_eq!((y, label_row), (2, 2));
        assert!(x >= label + title.chars().count() as u16);
        let line = row(&grid, y);
        assert!(
            !line.contains("Geomagnetic") && !line.contains("until"),
            "{line}"
        );

        let grid = screen(&m, 180, 24, Duration::ZERO);
        assert_eq!(
            location(&grid, "▌ALERT Geomagnetic storm · until 23:00 UTC  +1").1,
            1
        );
    }
}

#[test]
fn ribbon_escapes_the_headline_before_drawing() {
    // QGDXC-TUI-0125, QGDXC-TUI-0013.
    let mut m = weather_model();
    install_alerts(
        &mut m,
        &[(
            "QX20",
            "2026-11-22 20:00:00",
            "ALERT: Geomagnetic \u{1b}[31m\nIssue Time: 2026 Nov 22 2000 UTC",
        )],
    );
    let grid = screen(&m, 180, 24, Duration::ZERO);
    location(&grid, r"▌ALERT Geomagnetic \u{001b}[31m · until 23:00 UTC");
    assert!(!screen_text(&grid).contains('\u{1b}'));
}

#[test]
fn view_shows_waiting_values_alerts_and_refresh_status() {
    // QGDXC-TUI-0113 and QGDXC-TUI-0114.
    let mut m = weather_model();
    let initial = screen_text(&screen(&m, 80, 24, Duration::ZERO));
    assert_eq!(initial.matches("waiting").count(), FLUX_HISTORY + 3);
    assert!(initial.contains("space weather · NOAA SWPC"));
    assert!(initial.contains("data: NOAA Space Weather Prediction Center"));
    let (cache, _, _, _) = collect_fixtures();
    m.weather_data = cache;
    let full = screen(&m, 130, 34, Duration::ZERO);
    let text = screen_text(&full);
    for field in [
        "SFI 99",
        "Kp ",
        " A ",
        "R0 S0 G0",
        "X-ray B2.3",
        "Wind 343 km/s",
        "Bt 2 Bz -2 nT",
        "R0 S0 G0 19:41 UTC",
        "alerts",
        "next refresh 5m",
    ] {
        assert!(text.contains(field), "{field}\n{text}");
    }
    assert!(row(&full, location(&full, "X-ray B").1).contains('█'));
    assert!(row(&full, location(&full, "Kp ").1).contains('█'));
    assert_eq!(
        full[location(&full, "-2 nT")].fg,
        m.appearance.style(Role::Warning).fg.unwrap()
    );
    m.weather_data.pause = Duration::from_secs(120);
    let text = screen_text(&screen(&m, 80, 24, Duration::ZERO));
    assert!(text.contains("Retry-After 2m"));
    let stale = screen(&m, 130, 34, Duration::from_secs(2701));
    assert!(row(&stale, location(&stale, "SFI 99").1).contains("stale"));
    assert_eq!(
        stale[location(&stale, "SFI 99")].fg,
        m.appearance.style(Role::Dim).fg.unwrap()
    );
}

#[test]
fn weather_roles_alert_privacy_and_minimum_height() {
    // QGDXC-TUI-0114 and QGDXC-SEC-0005.
    let mut m = Model::new((1..=8).map(|i| format!("source{i}")).collect(), 100, 100);
    m.spots_view = SpotsView::Weather;
    m.utc_epoch = WEATHER_EPOCH;
    let mut script = fixture_script();
    let clock = FakeClock::default();
    let mut scheduler = Scheduler::default();
    let mut speed: serde_json::Value = serde_json::from_slice(&fixture(FIXTURES[4])).unwrap();
    speed[0]["proton_speed"] = serde_json::json!(2999.9);
    script.responses[4] = Outcome::Bytes(speed.to_string().into_bytes());
    script.responses[5] = Outcome::Bytes(
        br#"[{"time_tag":"2026-11-22T19:39:00Z","bt":199.9,"bz_gsm":-199.9}]"#.to_vec(),
    );
    script.responses[1] = Outcome::Bytes(
        br#"[
        {"time_tag":"2026-11-22T18:00:00Z","Kp":5,"a_running":400},
        {"time_tag":"2026-11-22T15:00:00Z","Kp":4,"a_running":14}
    ]"#
        .to_vec(),
    );
    let planted = ["192", "0", "2", "1"].join(".");
    let alerts = serde_json::json!([
        {"product_id":"QX01","issue_datetime":"2026-11-22 19:00:00","message":"ALERT: Older"},
        {"product_id":"QX02","issue_datetime":"2026-11-22 20:00:00","message":format!("WARNING: Newest {planted} \u{1b}[31m {}", "Long ".repeat(40))}
    ]);
    script.responses[6] = Outcome::Bytes(alerts.to_string().into_bytes());
    for _ in FIXTURES {
        m.weather_data
            .apply(scheduler.step(&mut script, &clock).unwrap());
    }
    let screen_buf = screen(&m, 130, 40, Duration::ZERO);
    // Kp 5 is NOAA's G1 colour; Bz at or below -10 nT is bad.
    let g1 = m.appearance.style(Role::G1).fg.unwrap();
    assert_eq!(screen_buf[location(&screen_buf, "5.0 A 400")].fg, g1);
    let bad = m.appearance.style(Role::Bad).fg.unwrap();
    assert_eq!(screen_buf[location(&screen_buf, "-199.9 nT")].fg, bad);
    let first = location(&screen_buf, "QX02");
    assert!(first.1 < location(&screen_buf, "QX01").1);
    let headline = row(&screen_buf, first.1);
    assert!(headline.contains("WARNING: Newest <ip>"));
    assert!(!headline.contains(&planted));
    assert!(!headline.contains('\u{1b}'));
    assert!(headline.contains("~│"));
    let compact = screen_text(&screen(&m, 100, 24, Duration::from_secs(2701)));
    assert_eq!(compact.matches(" stale").count(), 7, "{compact}");
    assert!(compact.contains("Wind 2999.9"));
    assert!(compact.contains("Bt 199.9 Bz -199.9"));
    let Some((Data::PlanetaryK(values), _)) = &mut m.weather_data.samples[1].good else {
        panic!("missing planetary K sample");
    };
    values.last_mut().unwrap().kp = 4.0;
    // Kp 4 is below NOAA's G1, so it reads as quiet.
    let quiet = screen(&m, 130, 40, Duration::ZERO);
    assert_eq!(
        quiet[location(&quiet, "4.0 A 400")].fg,
        m.appearance.style(Role::KpQuiet).fg.unwrap()
    );
}

#[test]
fn weather_times_follow_the_model_utc_date() {
    // QGDXC-TUI-0117.
    let mut m = weather_model();
    m.weather_data = collect_fixtures().0;
    install_alerts(
        &mut m,
        &[
            ("QX01", "2026-11-22 20:00:14", "WATCH: Current date"),
            ("QX02", "2026-11-21 20:00:45", "SUMMARY: Prior date"),
        ],
    );
    let current = screen_text(&screen(&m, 120, 24, Duration::ZERO));
    for text in [
        "R0 S0 G0 19:41 UTC",
        "SFI 99 11-21 20:00 UTC",
        "QX01 20:00 UTC WATCH:",
        "QX02 11-21 20:00 UTC SUMMARY:",
    ] {
        assert!(current.contains(text), "{text}\n{current}");
    }
    let after_midnight = screen_text(&screen(&m, 100, 24, Duration::from_hours(4)));
    for text in ["R0 S0 G0 11-22 19:41 UTC", "QX01 11-22 20:00 UTC"] {
        assert!(after_midnight.contains(text), "{text}\n{after_midnight}");
    }
}

#[test]
fn weather_alerts_include_the_five_day_boundary() {
    // QGDXC-TUI-0118.
    let mut m = weather_model();
    install_alerts(
        &mut m,
        &[
            ("QX01", "2026-11-17 20:01:00", "WATCH: Exact boundary"),
            (
                "QX02",
                "2026-11-17 20:00:59.999999",
                "WATCH: Over five days",
            ),
            ("QX03", "2026-11-22 20:01:00", "WATCH: Issued now"),
            ("QX04", "2026-11-22 20:01:01", "WATCH: Issued later"),
        ],
    );
    let buf = screen(&m, 100, 24, Duration::ZERO);
    let current = screen_text(&buf);
    assert!(current.contains("QX01 11-17 20:01 UTC WATCH: Exact boundary"));
    assert!(location(&buf, "QX03").1 < location(&buf, "QX01").1);
    assert!(!current.contains("QX02"));
    assert!(!current.contains("QX04"));
}

#[test]
fn weather_alerts_group_recent_issues_newest_first() {
    // QGDXC-TUI-0118.
    let mut m = weather_model();
    install_alerts(
        &mut m,
        &[
            ("QX01", "2026-11-22 20:00:00.1", "WATCH: Earlier duplicate"),
            ("QX02", "2026-11-22 20:00:00.2", "WATCH: Separate issue"),
            ("QX01", "2026-11-17 20:00:59", "WATCH: Expired duplicate"),
            ("QX01", "2026-11-22 20:00:00.3", "WATCH: Latest issue"),
        ],
    );
    let buf = screen(&m, 120, 24, Duration::ZERO);
    let text = screen_text(&buf);
    assert_eq!(text.matches("QX01").count(), 1, "{text}");
    assert!(
        text.contains("QX01 ×2 20:00 UTC WATCH: Latest issue"),
        "{text}"
    );
    assert!(
        text.contains("QX02 20:00 UTC WATCH: Separate issue"),
        "{text}"
    );
    assert!(location(&buf, "QX01").1 < location(&buf, "QX02").1);
    assert!(!text.contains("Earlier duplicate"));
    assert!(!text.contains("Expired duplicate"));
}

#[test]
fn weather_alerts_report_an_empty_window() {
    // QGDXC-TUI-0118.
    let mut m = weather_model();
    let expired = [("QX01", "2026-11-16 20:01:00", "WATCH: Expired")];
    for items in [&[][..], &expired[..]] {
        install_alerts(&mut m, items);
        let text = screen_text(&screen(&m, 120, 24, Duration::ZERO));
        assert!(text.contains("no alerts in the last 5 days"), "{text}");
        assert!(!text.contains("QX01"));
    }
}

#[test]
fn weather_layout_splits_at_120_columns() {
    // QGDXC-TUI-0119.
    let mut m = weather_model();
    m.weather_data = collect_fixtures().0;
    install_alerts(&mut m, &[("QX01", "2026-11-22 20:00:00", "WATCH: Example")]);
    for width in [120, 160] {
        let buf = screen(&m, width, 24, Duration::ZERO);
        let (left, heading) = location(&buf, "conditions");
        let (right, alerts_heading) = location(&buf, "alerts");
        assert_eq!(heading, alerts_heading, "{width}");
        assert!(
            (width * 53 / 100..=width * 58 / 100).contains(&right),
            "{width}: {right}"
        );
        assert_eq!(location(&buf, "SFI ").0, left, "{width}");
        assert_eq!(location(&buf, "QX01").0, right, "{width}");
        assert_eq!(
            location(&buf, "QX01").1,
            location(&buf, "SFI ").1,
            "{width}"
        );
    }
    for width in [100, 119] {
        let buf = screen(&m, width, 24, Duration::ZERO);
        assert!(!screen_text(&buf).contains("conditions"), "{width}");
        let (left, last_figure) = location(&buf, "Bt ");
        let (right, alerts_heading) = location(&buf, "alerts");
        assert_eq!(left, right, "{width}");
        assert!(alerts_heading > last_figure, "{width}");
        assert_eq!(
            location(&buf, "QX01"),
            (right, alerts_heading + 1),
            "{width}"
        );
    }
}

#[test]
fn weather_alerts_clip_to_the_short_box() {
    // QGDXC-TUI-0118 and QGDXC-TUI-0119.
    let mut m = Model::new((1..=8).map(|i| format!("source{i}")).collect(), 100, 100);
    m.spots_view = SpotsView::Weather;
    m.utc_epoch = WEATHER_EPOCH;
    m.weather_data = collect_fixtures().0;
    install_alerts(
        &mut m,
        &[
            ("QX01", "2026-11-22 19:59:00", "WATCH: Earlier"),
            ("QX02", "2026-11-22 20:00:00", "WATCH: Latest"),
            ("QX03", "2026-11-22 19:58:00", "WATCH: Clipped"),
        ],
    );
    let buf = screen(&m, 120, 24, Duration::ZERO);
    let (right, header) = location(&buf, "alerts");
    assert_eq!(location(&buf, "conditions").1, header);
    assert_eq!(location(&buf, "QX02"), (right, header + 1));
    assert_eq!(location(&buf, "QX01"), (right, header + 2));
    assert!(!screen_text(&buf).contains("QX03"));
}

#[test]
fn weather_sparklines_grow_to_their_column_width() {
    // QGDXC-TUI-0119.
    let mut m = weather_model();
    m.weather_data = collect_fixtures().0;
    let spark_cells = |buf: &Buffer, name| {
        row(buf, location(buf, name).1)
            .chars()
            .filter(|ch| ('▁'..='█').contains(ch))
            .count()
    };
    let small = screen(&m, 120, 24, Duration::ZERO);
    let large = screen(&m, 160, 24, Duration::ZERO);
    assert_eq!(spark_cells(&small, "Kp "), 8);
    assert_eq!(spark_cells(&large, "Kp "), 8);
    let small_xrays = spark_cells(&small, "X-ray ");
    let large_xrays = spark_cells(&large, "X-ray ");
    assert!(small_xrays > 24 && large_xrays > small_xrays);
    let Some((Data::PlanetaryK(values), _)) = &mut m.weather_data.samples[1].good else {
        panic!("missing planetary K sample");
    };
    let newest = values.last().unwrap().time;
    *values = (0..80)
        .rev()
        .map(|i| swx::PlanetaryK {
            time: newest - i * 900,
            kp: 4.0,
            a_running: 12.0,
        })
        .collect();
    let dense_small = screen(&m, 120, 24, Duration::ZERO);
    let dense_large = screen(&m, 160, 24, Duration::ZERO);
    assert!(spark_cells(&dense_large, "Kp ") > spark_cells(&dense_small, "Kp "));
    for (buf, figure) in [
        (&small, "X-ray "),
        (&large, "X-ray "),
        (&dense_small, "Kp "),
        (&dense_large, "Kp "),
    ] {
        let (right, _) = location(buf, "alerts");
        let (_, y) = location(buf, figure);
        assert!(
            buf[(right - 3, y)]
                .symbol()
                .starts_with(|ch| ('▁'..='█').contains(&ch))
        );
        assert_eq!(buf[(right - 2, y)].symbol(), " ");
    }
}
