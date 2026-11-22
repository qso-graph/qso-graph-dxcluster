// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use crate::tui::{
    config::Options,
    model::{Input, Model},
    present,
    theme::Role,
};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
use std::{cell::Cell, collections::VecDeque};

const FIXTURES: [&str; FEEDS.len()] = [
    "products_summary_10cm-flux.json",
    "products_noaa-planetary-k-index.json",
    "products_noaa-scales.json",
    "json_goes_primary_xrays-6-hour.json",
    "products_summary_solar-wind-speed.json",
    "products_summary_solar-wind-mag-field.json",
    "products_alerts.json",
];

fn fixture(name: &str) -> Vec<u8> {
    let directory = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../qso-graph-spaceweather/tests/fixtures",
    );
    std::fs::read(std::path::Path::new(directory).join(name)).unwrap()
}

#[derive(Default)]
struct FakeClock(Cell<Duration>);

impl FakeClock {
    fn at(&self, seconds: u64) {
        self.0.set(Duration::from_secs(seconds));
    }
}

impl Clock for FakeClock {
    fn elapsed(&self) -> Duration {
        self.0.get()
    }

    fn unix_time(&self) -> Duration {
        Duration::from_secs(1_795_376_460) + self.elapsed()
    }
}

#[derive(Default)]
struct Script {
    responses: VecDeque<Outcome>,
    calls: Vec<(&'static str, usize)>,
}

impl Fetch for Script {
    fn get(&mut self, url: &'static str, limit: usize) -> Outcome {
        self.calls.push((url, limit));
        self.responses.pop_front().expect("unexpected request")
    }
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
    for _ in FEEDS {
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
        FEEDS.map(|feed| (feed.url, feed.product.limit()))
    );
    assert!(cache.samples.iter().all(|sample| sample.good.is_some()));
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
fn https_configuration_and_read_cap() {
    // QGDXC-TUI-0112. Build the agent without sending a request.
    let agent = https();
    let config = agent.config();
    assert!(config.https_only());
    assert_eq!(config.max_redirects(), 0);
    assert!(!config.http_status_as_error());
    assert_eq!(config.timeouts().connect, Some(Duration::from_secs(10)));
    assert_eq!(config.timeouts().global, Some(Duration::from_secs(30)));
    assert!(
        matches!(config.user_agent(), ureq::config::AutoHeaderValue::Provided(value) if value.as_str() == USER_AGENT)
    );
    assert_eq!(config.tls_config().provider(), TlsProvider::Rustls);
    assert!(matches!(
        config.tls_config().root_certs(),
        RootCerts::WebPki
    ));
    assert!(!config.tls_config().disable_verification());
    assert!(
        FEEDS
            .iter()
            .all(|feed| feed.url.starts_with("https://services.swpc.noaa.gov/"))
    );
    for feed in FEEDS {
        let limit = feed.product.limit();
        let mut source = io::Cursor::new(vec![b' '; limit + 42]);
        assert_eq!(bounded_body(&mut source, limit).unwrap().len(), limit + 1);
        assert_eq!(source.position() as usize, limit + 1);
    }
}

#[test]
fn worker_starts_once_on_opt_in_and_keeps_polling() {
    // QGDXC-TUI-0111 and QGDXC-TUI-0116.
    let mut worker = Worker::default();
    let mut cache = Cache::default();
    worker
        .poll(false, &mut cache, || panic!("started before opt-in"))
        .unwrap();
    let (send, updates) = mpsc::sync_channel(1);
    let (stop, _stopped) = mpsc::channel();
    worker
        .poll(true, &mut cache, || {
            Ok(Running {
                _stop: stop,
                updates,
            })
        })
        .unwrap();
    let update = Scheduler::default()
        .step(&mut fixture_script(), &FakeClock::default())
        .unwrap();
    send.send(update).unwrap();
    worker
        .poll(false, &mut cache, || panic!("started a second worker"))
        .unwrap();
    assert!(cache.samples[0].good.is_some());
    worker
        .poll(true, &mut cache, || panic!("started a second worker"))
        .unwrap();
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
    assert!(!m.weather);
    let list = screen(&m, 130, 24, Duration::ZERO);
    assert!(row(&list, 23).contains("W weather"));
    key(&mut m, '/');
    key(&mut m, 'W');
    assert!(!m.weather);
    m.key(Input::Escape);
    key(&mut m, 'W');
    assert!(m.weather && !m.map && !m.beacons);
    let weather = screen(&m, 130, 24, Duration::ZERO);
    for y in 0..8 {
        assert_eq!(row(&list, y), row(&weather, y));
    }
    assert!(row(&weather, 23).contains("W list"));
    assert!(row(&screen(&m, 80, 24, Duration::ZERO), 23).contains(" B W ?"));
    key(&mut m, 'M');
    assert!(m.map && !m.beacons && !m.weather);
    key(&mut m, 'W');
    assert!(m.weather && !m.map && !m.beacons);
    key(&mut m, 'B');
    assert!(m.beacons && !m.map && !m.weather);
    key(&mut m, 'W');
    assert!(m.weather && !m.map && !m.beacons);
    key(&mut m, 'W');
    assert!(!m.weather && !m.map && !m.beacons);
    key(&mut m, '?');
    key(&mut m, 'W');
    assert!(!m.weather);
    assert!(screen_text(&screen(&m, 80, 24, Duration::ZERO)).contains("W: space weather"));
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
    m.weather = true;
    m.utc_epoch = WEATHER_EPOCH;
    m
}

fn install_alerts(m: &mut Model, alerts: &[(&str, &str, &str)]) {
    let items: Vec<_> = alerts
        .iter()
        .map(|(id, time, message)| {
            serde_json::json!({"product_id": id, "issue_datetime": time, "message": message})
        })
        .collect();
    let data = Data::parse(Product::Alerts, &serde_json::to_vec(&items).unwrap()).unwrap();
    m.weather_data.samples.last_mut().unwrap().good = Some((data, Duration::ZERO));
}

#[test]
fn view_shows_waiting_values_alerts_and_refresh_status() {
    // QGDXC-TUI-0113 and QGDXC-TUI-0114.
    let mut m = weather_model();
    let initial = screen_text(&screen(&m, 80, 24, Duration::ZERO));
    assert_eq!(initial.matches("waiting").count(), FEEDS.len());
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
    m.weather = true;
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
    for _ in FEEDS {
        m.weather_data
            .apply(scheduler.step(&mut script, &clock).unwrap());
    }
    let screen_buf = screen(&m, 130, 40, Duration::ZERO);
    let bad = m.appearance.style(Role::Bad).fg.unwrap();
    for field in ["5.0 A 400", "-199.9 nT"] {
        assert_eq!(screen_buf[location(&screen_buf, field)].fg, bad);
    }
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
    let warning = screen(&m, 130, 40, Duration::ZERO);
    assert_eq!(
        warning[location(&warning, "4.0 A 400")].fg,
        m.appearance.style(Role::Warning).fg.unwrap()
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
    m.weather = true;
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
