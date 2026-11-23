// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use crate::tui::{
    config::{Options, Source},
    model::{Input, Model},
    present,
    sources::Workers,
    web::{
        USER_AGENT,
        test_fetch::{FakeClock, Script},
    },
};
use qso_graph_dxcluster_cli::saved;
use ratatui::{Terminal, backend::TestBackend};
use serde_json::json;
use std::sync::Arc;

fn fixture() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../qso-graph-pota/tests/fixtures/spot-activator.json",
    ))
    .unwrap()
}

struct Run {
    model: Model,
    workers: Workers,
    tx: SyncSender<Envelope>,
    scheduler: Scheduler,
    fetch: Script,
    clock: FakeClock,
}

impl Run {
    fn new() -> Self {
        let mut model = Model::new(vec!["pota".into()], 100, 100);
        model.sources[0].start_pota();
        let mut workers = Workers::new(Arc::new(AtomicBool::new(false)));
        let tx = workers.add_queue();
        Self {
            model,
            workers,
            tx,
            scheduler: Scheduler::default(),
            fetch: Script::default(),
            clock: FakeClock::default(),
        }
    }

    fn tick(&mut self, seconds: u64) -> bool {
        self.clock.at(seconds);
        let Some(poll) = self.scheduler.step(&mut self.fetch, &self.clock) else {
            return false;
        };
        assert!(poll.send(&self.tx));
        self.workers.drain(&mut self.model, Instant::now());
        true
    }

    fn answer(&mut self, seconds: u64, outcome: Outcome) {
        self.fetch.responses.push_back(outcome);
        assert!(self.tick(seconds));
    }

    fn screen(&self, columns: u16, seconds: u64) -> Vec<String> {
        let mut term = Terminal::new(TestBackend::new(columns, 25)).unwrap();
        present(&mut term, &self.model, Duration::from_secs(seconds)).unwrap();
        let buffer = term.backend().buffer();
        (0..buffer.area.height)
            .map(|y| (0..columns).map(|x| buffer[(x, y)].symbol()).collect())
            .collect()
    }
}

#[test]
fn pota_configuration_selects_one_source_and_uses_its_normal_toggle() {
    let parse =
        |args: &[&str]| Options::parse(&args.iter().map(|s| (*s).into()).collect::<Vec<_>>());
    let direct = parse(&["--node", "pota"]).unwrap();
    assert!(matches!(direct.sources.as_slice(), [Source::Pota]));
    assert_eq!(direct.labels, ["pota"]);
    assert!(parse(&["--node", "pota", "--node", "pota"]).is_err());
    assert!(parse(&["--node", "pota/auto"]).is_err());

    let mut args = vec![
        "--node".into(),
        "pota".into(),
        "--call".into(),
        ["Q", "1", "ABC"].concat(),
    ];
    for port in 1..=7 {
        args.extend(["--node".into(), format!("example.invalid:{port}")]);
    }
    assert_eq!(Options::parse(&args).unwrap().sources.len(), 8);
    args.extend(["--node".into(), "example.invalid:8".into()]);
    assert!(Options::parse(&args).err().unwrap().contains("1 to 8"));

    let sources = br#"{"version":1,"sources":[{"name":"Parks", "node":"pota", "selected":true}]}"#;
    let mut options = direct;
    options.saved = Some(saved::parse(sources).unwrap());
    assert!(options.saved.as_ref().unwrap().entries[0].selected);
    options.choose(&[0]);
    assert!(matches!(options.sources.as_slice(), [Source::Pota]));
    assert_eq!(options.labels, ["pota"]);
    let invalid = br#"{"version":1,"sources":[{"name":"Parks","node":"pota","profile":"auto"}]}"#;
    assert!(
        saved::parse(invalid)
            .unwrap_err()
            .contains("POTA has no profile")
    );
    let call = ["Q", "1", "ABC"].concat();
    let colliding = json!({
        "version": 1, "call": call, "sources": [
            {"name": "pota", "node": "example.invalid:1"},
            {"name": "Parks", "node": "pota"}
        ]
    });
    options.saved = Some(saved::parse(colliding.to_string().as_bytes()).unwrap());
    options.choose(&[0, 1]);
    assert_eq!(options.labels, ["pota#2", "pota"]);
    let duplicates = br#"{"version":1,"sources":[{"name":"Parks","node":"pota"},{"name":"Again","node":"pota"}]}"#;
    assert!(
        saved::parse(duplicates)
            .unwrap_err()
            .contains("duplicate endpoint")
    );

    let mut run = Run::new();
    assert_eq!(run.model.sources[0].server, "pota");
    assert_eq!(run.model.sources[0].state, "connecting");
    run.answer(0, Outcome::Bytes(fixture()));
    run.model.key(Input::Char('1'));
    assert!(run.model.matching().is_empty());
    assert_eq!(run.model.sources[0].count, 58);
    run.model.key(Input::Char('1'));
    assert!(!run.model.matching().is_empty());
}

#[test]
fn pota_poll_adds_only_unseen_ids_to_the_source_queue() {
    let bytes = fixture();
    let mut run = Run::new();
    run.answer(0, Outcome::Bytes(bytes.clone()));
    assert_eq!((run.model.total, run.model.spots.len()), (58, 58));
    assert_eq!(run.model.matching().len(), 58);
    assert_eq!(run.model.sources[0].state, "ready");
    assert_eq!(run.model.sources[0].pota_skipped, Some(0));

    run.answer(60, Outcome::Bytes(bytes.clone()));
    assert_eq!((run.model.total, run.model.spots.len()), (58, 58));
    assert_eq!(run.model.matching().len(), 58);

    let mut spots: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let row = &mut spots[0];
    row["spotId"] = json!(1);
    let activator = ["Q", "9", "NEW"].concat();
    row["activator"] = json!(activator);
    run.answer(120, Outcome::Bytes(spots.to_string().into_bytes()));
    assert_eq!((run.model.total, run.model.spots.len()), (59, 59));
    assert_eq!(run.model.matching().len(), 59);
    assert_eq!(run.model.sources[0].count, 59);
}

#[test]
fn pota_mapping_sets_event_values_and_starts_the_comment_with_the_park() {
    let mut spots: serde_json::Value = serde_json::from_slice(&fixture()).unwrap();
    let row = &mut spots[0];
    let address = [192, 0, 2, 17].map(|n| n.to_string()).join(".");
    row["comments"] = json!(format!("Portable {address}\u{001b}"));
    row["grid6"] = json!(["CN94", "xx"].concat());
    let mut run = Run::new();
    let body = json!([row, null]);
    run.answer(0, Outcome::Bytes(body.to_string().into_bytes()));
    let event = &run.model.spots[0].event;
    assert_eq!(event.kind, "spot");
    assert_eq!(event.server_type.name(), "pota");
    assert!(event.raw_line.is_empty());
    assert_eq!(event.get("call"), Some(["Q", "1", "AA"].concat().as_str()));
    assert_eq!(
        event.get("spotter_call"),
        Some(["Q", "1", "BA"].concat().as_str())
    );
    assert_eq!(
        event.fields.get("frequency_hz"),
        Some(&Value::Integer(14_074_000))
    );
    assert_eq!(event.get("mode"), Some("FT8"));
    assert_eq!(event.get("grid"), Some("CN94"));
    assert_eq!(event.get("time_of_day"), Some("00:54"));
    assert_eq!(event.get("pota_reference"), Some("US-9990"));
    assert_eq!(event.get("comment"), Some("Portable <ip>\u{001b}"));
    assert!(event.get("grid6").is_none());
    assert_eq!(run.model.sources[0].pota_skipped, Some(1));

    for columns in [80, 120] {
        let screen = run.screen(columns, 0);
        assert!(screen.iter().any(|row| row.contains("US-9990 Portable")));
        assert!(screen.iter().all(|row| !row.contains('\u{001b}')));
    }
    assert!(
        run.screen(120, 0)
            .iter()
            .any(|row| row.contains("skipped: 1"))
    );
}

#[test]
fn pota_rbn_spots_are_hidden_by_the_human_filter() {
    let mut run = Run::new();
    run.answer(0, Outcome::Bytes(fixture()));
    assert_eq!(
        run.model
            .spots
            .iter()
            .filter(|s| s.event.get("spot_kind") == Some("skimmer"))
            .count(),
        20
    );
    assert!(run.model.spots.iter().any(|s| {
        s.event.get("pota_reference") == Some("AR-0373")
            && s.event.get("spot_kind") == Some("skimmer")
    }));
    run.model.key(Input::Char('k'));
    let human = run.model.matching();
    assert_eq!(human.len(), 38);
    assert!(
        human
            .iter()
            .all(|s| s.event.get("spot_kind") == Some("human"))
    );
    assert!(
        human
            .iter()
            .all(|s| s.event.get("pota_reference") != Some("AR-0373"))
    );
}

#[test]
fn pota_waits_without_requests_and_keeps_rows_on_failure() {
    let bytes = fixture();
    let mut run = Run::new();
    run.answer(0, Outcome::Bytes(bytes.clone()));
    assert!(!run.tick(59));
    assert_eq!(run.fetch.calls.len(), 1);

    run.answer(60, Outcome::Status(429, Some("120".into())));
    assert_eq!(run.model.spots.len(), 58);
    assert_eq!(
        run.model.sources[0].retry_at,
        Some(Duration::from_secs(180))
    );
    let screen = run.screen(120, 61);
    assert!(
        screen
            .iter()
            .any(|row| row.contains("pota") && row.contains("retry"))
    );
    assert!(
        screen
            .iter()
            .any(|row| row.contains("retrying in 119s: HTTP 429"))
    );
    assert!(!run.tick(179));
    assert_eq!(run.fetch.calls, [(URL, 1_048_576); 2]);

    run.answer(180, Outcome::Bytes(b"[".to_vec()));
    assert_eq!(run.model.spots.len(), 58);
    assert_eq!(run.model.sources[0].reason, "invalid JSON");
    assert_eq!(
        run.model.sources[0].retry_at,
        Some(Duration::from_secs(300))
    );
    assert!(!run.tick(299));
    run.answer(300, Outcome::Bytes(bytes));
    assert_eq!(run.model.spots.len(), 58);
    assert_eq!(run.model.sources[0].state, "ready");
    assert_eq!(run.model.sources[0].retry_at, None);
}

#[test]
fn pota_policy_matches_the_annex_f_fixture() {
    let url = "https://api.pota.app/spot/activator";
    assert_eq!(
        policy(),
        Policy {
            floors: [(url.into(), Duration::from_secs(60))].into(),
            wait_scope: WaitScope::Provider,
            retry_after: RetryAfter {
                minimum: Duration::from_secs(60),
                maximum: Duration::from_secs(3_600),
                default: Some(Duration::from_secs(60)),
                server_errors: RetryAfterStatuses::ServiceUnavailable,
            },
            backoff: Some(Backoff {
                first: Duration::from_secs(60),
                maximum: Duration::from_secs(900),
            }),
        }
    );
    let mut run = Run::new();
    run.answer(0, Outcome::Bytes(b"[]".to_vec()));
    assert_eq!(run.fetch.calls, [(url, 1_048_576)]);
    let transport = Https::new(HOST, [URL]);
    assert!(transport.allows_host("api.pota.app"));
    assert!(!transport.allows_host("example.invalid"));
    assert_eq!(USER_AGENT, format!("qg-dxc/{}", env!("CARGO_PKG_VERSION")));
}

#[test]
fn pota_default_retry_after_and_backoff_are_bounded() {
    for (header, seconds) in [
        (None, 60),
        (Some("bad"), 60),
        (Some("2"), 60),
        (Some("99999"), 3_600),
    ] {
        let mut run = Run::new();
        run.answer(0, Outcome::Error("request failed"));
        run.answer(60, Outcome::Status(429, header.map(str::to_owned)));
        assert_eq!(run.scheduler.next(), Duration::from_secs(60 + seconds));
        assert!(!run.tick(60 + seconds - 1));
    }
    let mut run = Run::new();
    let mut now = 0;
    for wait in [60, 120, 240, 480, 900, 900] {
        run.answer(now, Outcome::Status(503, None));
        now += wait;
        assert_eq!(run.scheduler.next(), Duration::from_secs(now));
        assert!(!run.tick(now - 1));
    }
    run.answer(now, Outcome::Bytes(b"[]".to_vec()));
    run.answer(now + 60, Outcome::Error("request failed"));
    assert_eq!(run.scheduler.next(), Duration::from_secs(now + 120));
}

#[test]
fn pota_seen_ids_are_evicted_in_arrival_order() {
    let mut seen = Seen::default();
    for id in 1..=SEEN_LIMIT as u64 {
        assert!(seen.insert(id));
    }
    assert!(!seen.insert(1));
    assert!(seen.insert(SEEN_LIMIT as u64 + 1));
    assert!(!seen.insert(2));
    assert!(seen.insert(1));
    assert_eq!((seen.ids.len(), seen.order.len()), (SEEN_LIMIT, SEEN_LIMIT));
}

#[test]
fn pota_is_refused_with_replay_and_has_no_recording_option() {
    let parse =
        |args: &[&str]| Options::parse(&args.iter().map(|s| (*s).into()).collect::<Vec<_>>());
    let refusal = parse(&["--node", "pota", "--replay", "unused.ndjson"])
        .err()
        .unwrap();
    assert!(refusal.contains("--node cannot be combined with --replay"));
    let refusal = parse(&["--node", "pota", "--record", "unused.ndjson"])
        .err()
        .unwrap();
    assert!(refusal.contains("unexpected argument --record"));
    assert!(Profile::parse("pota").is_none());
}
