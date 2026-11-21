// SPDX-License-Identifier: GPL-3.0-or-later
use super::{
    config::{Options, Source, labels},
    model::*,
    render,
    sources::{CAPACITY, Message, Schedule},
    *,
};
use crate::{
    Delivery, Live,
    recording::{Recorder, ReplayStream},
};
use qso_graph_dxcluster::{Config, Event as ClusterEvent, Profile, Session, State};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect};
use std::{collections::BTreeSet, io::Cursor, sync::mpsc};

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|s| (*s).into()).collect()
}
fn spot(call: &str, hz: i64, comment: &str) -> ClusterEvent {
    let mut event = ClusterEvent::new(Profile::ArCluster, "spot");
    event.set("call", call);
    event.set("spotter_call", "Q9XYZ-#");
    event.set("frequency_hz", qso_graph_dxcluster::Value::Integer(hz));
    event.set("comment", comment);
    event.set("time_of_day", "18:32");
    event.set("spot_kind", "human");
    event
}
fn grid(model: &Model, width: u16, height: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    present(&mut terminal, model, Duration::from_secs(10)).unwrap();
    terminal.backend().buffer().clone()
}
fn text(buffer: &Buffer, y: u16) -> String {
    (0..buffer.area.width)
        .map(|x| buffer[(x, y)].symbol())
        .collect()
}
fn key(model: &mut Model, code: KeyCode) {
    model.key(input(Event::Key(event::KeyEvent::new(code, KeyModifiers::NONE))).unwrap());
}
fn limit_entry(model: &mut Model, value: &str) {
    for _ in 0..5 {
        key(model, KeyCode::Backspace);
    }
    for c in value.chars() {
        key(model, KeyCode::Char(c));
    }
}
#[test]
fn limits_editor_opens_edits_commits_and_cancels() {
    // QGDXC-TUI-0012 QGDXC-TUI-0051 QGDXC-TUI-0060
    let mut m = Model::new(args(&["one"]), 100, 23);
    key(&mut m, KeyCode::Char('l'));
    for width in [80, 120] {
        let screen = grid(&m, width, 24);
        assert!(text(&screen, 0).starts_with("Limits:"));
        assert!(text(&screen, 3).starts_with("> Human: 100"));
        assert!(text(&screen, 4).contains("Skimmer: 23"));
    }
    key(&mut m, KeyCode::Up);
    limit_entry(&mut m, "10");
    for c in ['q', 't', 'l', '-', ' ', 'é', '٢'] {
        key(&mut m, KeyCode::Char(c));
    }
    assert!(!m.quit);
    key(&mut m, KeyCode::Down);
    key(&mut m, KeyCode::Down);
    limit_entry(&mut m, "100009");
    assert!(text(&grid(&m, 80, 24), 4).starts_with("> Skimmer: 10000"));
    key(&mut m, KeyCode::Backspace);
    key(&mut m, KeyCode::Char('0'));
    key(&mut m, KeyCode::Tab);
    assert!(text(&grid(&m, 80, 24), 3).starts_with("> Human: 10"));
    key(&mut m, KeyCode::Tab);
    key(&mut m, KeyCode::Up);
    assert!(text(&grid(&m, 80, 24), 3).starts_with("> Human: 10"));
    key(&mut m, KeyCode::Enter);
    assert!(matches!(m.editor, Editor::Normal));
    key(&mut m, KeyCode::Char('l'));
    assert!(matches!(&m.editor, Editor::Limits { values, .. } if values == &["10", "10000"]));
    limit_entry(&mut m, "999");
    key(&mut m, KeyCode::Tab);
    limit_entry(&mut m, "500");
    key(&mut m, KeyCode::Esc);
    key(&mut m, KeyCode::Char('l'));
    assert!(matches!(&m.editor, Editor::Limits { values, .. } if values == &["10", "10000"]));
}

#[test]
fn limits_invalid_entry_keeps_both_caps_and_shows_error() {
    // QGDXC-TUI-0012 QGDXC-TUI-0051 QGDXC-TUI-0060
    for field in [0, 1] {
        for invalid in ["", "0", "9", "10001", "99999"] {
            let mut m = Model::new(args(&["one"]), 17, 23);
            for _ in 0..17 {
                m.receive(0, spot("Q1HUM", 14_025_000, "CW"), Duration::ZERO);
            }
            key(&mut m, KeyCode::Char('l'));
            limit_entry(&mut m, if field == 0 { invalid } else { "10" });
            key(&mut m, KeyCode::Tab);
            limit_entry(&mut m, if field == 1 { invalid } else { "10" });
            key(&mut m, KeyCode::Enter);
            assert!(matches!(m.editor, Editor::Limits { .. }));
            assert_eq!(m.spots.len(), 17);
            for width in [80, 120] {
                assert!(text(&grid(&m, width, 24), 22).contains("Limits must be 10 to 10,000"));
            }
            key(&mut m, KeyCode::Esc);
            key(&mut m, KeyCode::Char('l'));
            assert!(matches!(&m.editor, Editor::Limits { values, .. } if values == &["17", "23"]));
            key(&mut m, KeyCode::Tab);
            limit_entry(&mut m, "9");
            key(&mut m, KeyCode::Enter);
            m.key(
                input(Event::Key(event::KeyEvent::new(
                    KeyCode::Char('c'),
                    KeyModifiers::CONTROL,
                )))
                .unwrap(),
            );
            assert!(m.quit);
        }
    }
    // A rejected edit can be corrected and committed without reopening.
    let mut m = Model::new(args(&["one"]), 17, 23);
    key(&mut m, KeyCode::Char('l'));
    limit_entry(&mut m, "9");
    key(&mut m, KeyCode::Enter);
    limit_entry(&mut m, "10000");
    key(&mut m, KeyCode::Enter);
    assert!(matches!(m.editor, Editor::Normal));
    key(&mut m, KeyCode::Char('l'));
    assert!(matches!(&m.editor, Editor::Limits { values, .. } if values == &["10000", "23"]));
}

#[test]
fn runtime_limits_evict_oldest_per_kind_and_preserve_activity_and_pause() {
    // QGDXC-TUI-0010 QGDXC-TUI-0012 QGDXC-TUI-0051 QGDXC-TUI-0073
    for paused in [false, true] {
        for human in [false, true] {
            for evicted_anchor in [false, true] {
                let mut m = Model::new(args(&["human", "skimmer"]), 20, 20);
                for i in 0..12 {
                    m.receive(0, spot("Q1HUM", 14_025_000, "CW"), Duration::from_secs(i));
                    let mut event = spot("Q2SKM", 7_025_000, "CW");
                    event.set("spot_kind", "skimmer");
                    m.receive(1, event, Duration::from_secs(i));
                }
                if paused {
                    key(&mut m, KeyCode::Char('p'));
                }
                m.top = Some(if evicted_anchor {
                    if human { 1 } else { 2 }
                } else {
                    24
                });
                key(&mut m, KeyCode::Char('l'));
                // Reception continues in the editor, including unclassified spots.
                for kind in [Some("skimmer"), None] {
                    m.receive(0, spot("Q3NEW", 14_025_000, "CW"), Duration::from_secs(12));
                    let mut event = spot("Q4UNK", 7_025_000, "CW");
                    event.fields.remove("spot_kind");
                    if let Some(kind) = kind {
                        event.set("spot_kind", kind);
                    }
                    m.receive(1, event, Duration::from_secs(12));
                }
                let anchor = m.top;
                let activity: Vec<_> = m
                    .activity
                    .iter()
                    .map(|b| (b.second, b.sources, b.bands))
                    .collect();
                let summary = (
                    m.total,
                    m.sources[0].count,
                    m.sources[1].count,
                    m.paused_new(),
                );
                let before: Vec<_> = m.spots.iter().map(|s| s.id).collect();
                limit_entry(&mut m, if human { "10" } else { "20" });
                key(&mut m, KeyCode::Tab);
                limit_entry(&mut m, if human { "20" } else { "10" });
                key(&mut m, KeyCode::Enter);
                // Plant-and-prove target: trimming the newest must fail this exact order assertion.
                let expected: Vec<_> = before
                    .into_iter()
                    .filter(|id| {
                        let same_kind = (*id % 2 == 1) == human;
                        !same_kind || *id >= 9
                    })
                    .collect();
                assert_eq!(m.spots.iter().map(|s| s.id).collect::<Vec<_>>(), expected);
                assert_eq!(
                    (m.kept_human, m.kept_skimmer),
                    if human { (10, 14) } else { (14, 10) }
                );
                assert_eq!(
                    m.top,
                    if paused && evicted_anchor {
                        expected.last().copied()
                    } else {
                        anchor
                    }
                );
                assert_eq!(m.paused, paused);
                assert_eq!(
                    (
                        m.total,
                        m.sources[0].count,
                        m.sources[1].count,
                        m.paused_new()
                    ),
                    summary
                );
                assert_eq!(
                    m.activity
                        .iter()
                        .map(|b| (b.second, b.sources, b.bands))
                        .collect::<Vec<_>>(),
                    activity
                );
                // Future arrivals use the committed cap, rather than the startup cap.
                let mut event = spot("Q5NEW", 28_025_000, "CW");
                event.set("spot_kind", if human { "human" } else { "skimmer" });
                m.receive(0, event, Duration::from_secs(13));
                assert_eq!(
                    (m.kept_human, m.kept_skimmer),
                    if human { (10, 14) } else { (14, 10) }
                );
                let before: Vec<_> = m.spots.iter().map(|s| s.id).collect();
                key(&mut m, KeyCode::Char('l'));
                limit_entry(&mut m, "40");
                key(&mut m, KeyCode::Tab);
                limit_entry(&mut m, "40");
                key(&mut m, KeyCode::Enter);
                assert_eq!(m.spots.iter().map(|s| s.id).collect::<Vec<_>>(), before);
                m.receive(0, spot("Q6NEW", 14_025_000, "CW"), Duration::from_secs(14));
                let mut event = spot("Q7NEW", 7_025_000, "CW");
                event.set("spot_kind", "skimmer");
                m.receive(1, event, Duration::from_secs(14));
                assert_eq!(
                    (m.kept_human, m.kept_skimmer),
                    if human { (11, 15) } else { (15, 11) }
                );
            }
        }
    }
}

#[test]
fn limits_footer_and_help_fit_compact_and_wide_layouts() {
    // QGDXC-TUI-0012 QGDXC-TUI-0070 QGDXC-TUI-0060
    use ratatui::style::Modifier;
    let mut m = Model::new(args(&["one"]), DEFAULT_KEEP, DEFAULT_KEEP);
    for _ in 0..5 {
        for paused in [false, true] {
            m.paused = paused;
            for width in [80, 120] {
                let screen = grid(&m, width, 24);
                let footer = text(&screen, 23);
                for hint in ["q quit", "b band", "m mode", "/ call", "k kind", "l limits"] {
                    assert!(footer.contains(hint), "{footer}");
                }
                assert!(
                    footer.contains(if paused { "p resume" } else { "p pause" }),
                    "{footer}"
                );
                assert!(
                    footer.contains(if width == 80 { "1-8 src" } else { "1-8 source" }),
                    "{footer}"
                );
                let tail = if width == 80 {
                    "t ?"
                } else {
                    "t theme  ? help"
                };
                assert!(footer.contains(tail), "{footer}");
                let theme = format!("{} ", m.appearance.theme.name());
                assert!(footer.ends_with(&theme), "{footer}");
                assert!(find(&screen, 23, tail) + (tail.len() as u16) < width - theme.len() as u16);
                assert!(
                    screen[(find(&screen, 23, "l limits"), 23)]
                        .modifier
                        .contains(Modifier::BOLD)
                );
            }
        }
        key(&mut m, KeyCode::Char('t'));
    }
    key(&mut m, KeyCode::Char('?'));
    for width in [80, 120] {
        let help = grid(&m, width, 24);
        assert!((0..24).any(|y| text(&help, y).contains("l limits")));
    }
}
#[test]
fn limits_lower_both_caps_even_when_spots_are_hidden_or_filtered() {
    // QGDXC-TUI-0012 QGDXC-TUI-0040 QGDXC-TUI-0042
    let mut m = Model::new(args(&["human", "skimmer"]), 20, 20);
    for _ in 0..12 {
        m.receive(0, spot("Q1HUM", 14_025_000, "CW"), Duration::ZERO);
        let mut event = spot("Q2SKM", 7_025_000, "CW");
        event.set("spot_kind", "skimmer");
        m.receive(1, event, Duration::ZERO);
    }
    m.sources[1].hidden = true;
    m.filters.bands = Some(BTreeSet::new());
    m.clamp(true);
    key(&mut m, KeyCode::Char('p'));
    key(&mut m, KeyCode::Char('l'));
    limit_entry(&mut m, "10");
    key(&mut m, KeyCode::Tab);
    limit_entry(&mut m, "10");
    key(&mut m, KeyCode::Enter);
    assert_eq!((m.kept_human, m.kept_skimmer), (10, 10));
    assert_eq!(
        m.spots.iter().map(|s| s.id).collect::<Vec<_>>(),
        (5..=24).rev().collect::<Vec<_>>()
    );
    assert!(m.matching().is_empty());
    assert!(m.sources[1].hidden);
    assert_eq!(m.top, None);
    key(&mut m, KeyCode::Char('c'));
    assert_eq!(m.matching().len(), 20);
}
#[test]
fn configuration_validates_all_sources_before_io() {
    // QGDXC-TUI-0001 QGDXC-TUI-0014
    let valid = Options::parse(&args(&[
        "--call",
        "Q1ABC",
        "--node",
        "example.invalid:7300",
        "--node",
        "[2001:db8::1]:65535/rbn",
    ]))
    .unwrap();
    assert_eq!(valid.sources.len(), 2);
    for source in valid.sources {
        let Source::Node(c) = source else { panic!() };
        assert!(!c.retain_third_party_data);
    }
    for profile in ["auto", "dxspider", "arcluster", "cccluster", "rbn"] {
        assert!(
            Options::parse(&args(&[
                "--call",
                "Q1ABC",
                "--node",
                &format!("example.invalid:1/{profile}")
            ]))
            .is_ok()
        );
    }
    for endpoint in [
        "bad host:1",
        "example.invalid:0",
        "example.invalid:65536",
        "example.invalid:+1",
        "example.invalid:1/unknown",
        "example.invalid:1/ssh",
        "example.invalid",
        "[]:1",
        "[no]:1",
        "2001:db8::1:7300",
        ":1",
    ] {
        let error = Options::parse(&args(&["--call", "Q1ABC", "--node", endpoint]))
            .err()
            .unwrap();
        assert!(error.contains(endpoint), "{error}");
    }
    for call in ["Q1ABC-0", "Q1ABC-01", "Q1ABC\nq", "QABC"] {
        assert!(
            Options::parse(&args(&["--call", call, "--node", "example.invalid:1"]))
                .err()
                .unwrap()
                .contains(call)
        );
    }
    for values in [
        vec!["--node", "example.invalid:1"],
        vec!["--replay", "one", "--node", "example.invalid:1"],
        vec![
            "--call",
            "Q1ABC",
            "--node",
            "example.invalid:1",
            "--node",
            "EXAMPLE.invalid:1/rbn",
        ],
        vec!["--retain-third-party-data", "yes"],
        vec!["--replay", "one", "--call", "Q1ABC"],
    ] {
        assert!(Options::parse(&args(&values)).is_err());
    }
    let eight: Vec<_> = (0..8)
        .flat_map(|_| ["--replay".into(), "Cargo.toml".to_owned()])
        .collect();
    assert!(Options::parse(&eight).is_ok());
    let nine: Vec<_> = (0..9)
        .flat_map(|_| ["--replay".into(), "Cargo.toml".to_owned()])
        .collect();
    assert!(
        Options::parse(&nine)
            .err()
            .unwrap()
            .contains("--node/--replay")
    );
}
#[test]
fn retention_defaults_and_configured_range() {
    // QGDXC-TUI-0012
    let defaults = Options::parse(&args(&["--replay", "Cargo.toml"])).unwrap();
    assert_eq!((defaults.keep_human, defaults.keep_skimmer), (100, 100));
    let mut m = Model::new(defaults.labels, defaults.keep_human, defaults.keep_skimmer);
    for kind in ["human", "skimmer"] {
        for _ in 0..101 {
            let mut event = spot("Q1ABC", 14_025_000, "CW");
            event.set("spot_kind", kind);
            m.receive(0, event, Duration::ZERO);
        }
    }
    assert_eq!((m.kept_human, m.kept_skimmer), (100, 100));
    assert_eq!(m.spots.len(), 200);
    assert_eq!(m.total, 202);
    for flag in ["--keep-human", "--keep-skimmer"] {
        for value in ["10", "10000"] {
            let options = Options::parse(&args(&["--replay", "Cargo.toml", flag, value])).unwrap();
            assert_eq!(
                if flag == "--keep-human" {
                    options.keep_human
                } else {
                    options.keep_skimmer
                },
                value.parse::<usize>().unwrap()
            );
            assert_eq!(
                if flag == "--keep-human" {
                    options.keep_skimmer
                } else {
                    options.keep_human
                },
                100
            );
        }
        for value in [
            "9",
            "10001",
            "0",
            "-1",
            "abc",
            "10.5",
            "184467440737095516160",
        ] {
            let error = Options::parse(&args(&["--replay", "Cargo.toml", flag, value]))
                .err()
                .unwrap();
            assert!(
                error.contains(flag) && error.contains("10..=10000"),
                "{error}"
            );
        }
    }
    let configured = Options::parse(&args(&[
        "--call",
        "Q1ABC",
        "--node",
        "example.invalid:7300",
        "--keep-human",
        "17",
        "--keep-skimmer",
        "23",
    ]))
    .unwrap();
    assert_eq!((configured.keep_human, configured.keep_skimmer), (17, 23));
}
#[test]
fn per_kind_eviction_preserves_other_kind_and_paused_anchor() {
    // QGDXC-TUI-0010 QGDXC-TUI-0012 QGDXC-TUI-0040 QGDXC-TUI-0051
    let mut m = Model::new(args(&["human", "skimmer"]), 10, 12);
    for _ in 0..10 {
        m.receive(0, spot("Q1HUM", 14_025_000, "CW"), Duration::ZERO);
    }
    key(&mut m, KeyCode::Char('p'));
    for _ in 0..200 {
        let mut event = spot("Q2SKM", 7_025_000, "CW");
        event.set("spot_kind", "skimmer");
        m.receive(1, event, Duration::from_secs(1));
        // Plant-and-prove target: a shared FIFO cap must evict these humans.
        assert_eq!(
            m.spots
                .iter()
                .filter(|s| s.event.get("spot_kind") == Some("human"))
                .count(),
            10
        );
        assert_eq!(m.top, Some(10));
        assert!(m.spots.len() <= 22);
    }
    assert_eq!((m.kept_human, m.kept_skimmer), (10, 12));
    assert_eq!(
        m.spots.iter().map(|s| s.id).collect::<Vec<_>>(),
        (199..=210).rev().chain((1..=10).rev()).collect::<Vec<_>>()
    );
    assert_eq!(m.paused_new(), 200);
    key(&mut m, KeyCode::Down);
    assert_eq!(m.top, Some(9));
    key(&mut m, KeyCode::Up);
    assert_eq!(m.top, Some(10));
    for _ in 0..200 {
        m.receive(0, spot("Q3HUM", 14_025_000, "CW"), Duration::from_secs(2));
    }
    assert_eq!((m.kept_human, m.kept_skimmer), (10, 12));
    assert_eq!(
        m.spots.iter().map(|s| s.id).collect::<Vec<_>>(),
        (401..=410)
            .rev()
            .chain((199..=210).rev())
            .collect::<Vec<_>>()
    );
    assert_eq!(m.top, Some(199));
    assert_eq!(m.paused_new(), 400);
    key(&mut m, KeyCode::Char('k'));
    assert_eq!(m.matching().len(), 10);
    assert_eq!(m.top, Some(410));
    key(&mut m, KeyCode::Char('k'));
    assert_eq!(m.matching().len(), 12);
    assert_eq!(m.top, Some(210));
    key(&mut m, KeyCode::Char('c'));
    key(&mut m, KeyCode::Char('p'));
    assert_eq!(m.top, Some(410));
}
#[test]
fn unclassified_spots_share_skimmer_retention() {
    // QGDXC-TUI-0012
    let mut m = Model::new(args(&["one"]), 10, 10);
    m.receive(0, spot("Q1HUM", 14_025_000, "CW"), Duration::ZERO);
    for i in 0..11 {
        let mut event = spot("Q2UNK", 7_025_000, "CW");
        if i % 2 == 0 {
            event.fields.remove("spot_kind");
        } else {
            event.set("spot_kind", "unknown");
        }
        m.receive(0, event, Duration::ZERO);
    }
    assert_eq!((m.kept_human, m.kept_skimmer), (1, 10));
    assert_eq!(
        m.spots.iter().map(|s| s.id).collect::<Vec<_>>(),
        (3..=12).rev().chain([1]).collect::<Vec<_>>()
    );
    let mut event = spot("Q3SKM", 7_025_000, "CW");
    event.set("spot_kind", "skimmer");
    m.receive(0, event, Duration::ZERO);
    assert_eq!((m.kept_human, m.kept_skimmer), (1, 10));
    assert!(!m.spots.iter().any(|s| s.id == 3));
    assert_eq!(m.spots.back().unwrap().id, 1);
    let wide = grid(&m, 120, 36);
    assert!(text(&wide, 34).contains("1 human · 10 skimmer"));
}
#[test]
fn activity_charts_and_sparklines_are_independent_of_retention_caps() {
    // QGDXC-TUI-0073 QGDXC-TUI-0012 QGDXC-TUI-0070
    let mut small = Model::new(args(&["human", "skimmer"]), 10, 12);
    let mut large = Model::new(args(&["human", "skimmer"]), 10000, 10000);
    for m in [&mut small, &mut large] {
        key(m, KeyCode::Char('p'));
        m.filters.bands = Some(BTreeSet::new());
        m.sources[1].hidden = true;
        for (second, count) in [(1, 12), (61, 24), (121, 48)] {
            for source in [0, 1] {
                for _ in 0..count {
                    let mut event = spot(
                        "Q1ABC",
                        if source == 0 { 14_025_000 } else { 7_025_000 },
                        "CW",
                    );
                    event.set("spot_kind", if source == 0 { "human" } else { "skimmer" });
                    m.receive(source, event, Duration::from_secs(second));
                }
            }
        }
        assert_eq!(m.total, 168);
        assert_eq!(m.paused_new(), 168);
        assert_eq!(
            m.sources.iter().map(|s| s.count).collect::<Vec<_>>(),
            [84, 84]
        );
    }
    assert_eq!(small.spots.len(), 22);
    assert_eq!(large.spots.len(), 168);
    assert_eq!(small.activity.len(), large.activity.len());
    for (a, b) in small.activity.iter().zip(&large.activity) {
        assert_eq!(
            (a.second, a.sources, a.bands),
            (b.second, b.sources, b.bands)
        );
    }
    let buffers: Vec<_> = [&small, &large]
        .into_iter()
        .map(|m| {
            let mut terminal = Terminal::new(TestBackend::new(120, 36)).unwrap();
            present(&mut terminal, m, Duration::from_secs(130)).unwrap();
            terminal.backend().buffer().clone()
        })
        .collect();
    for buffer in &buffers {
        assert!(text(buffer, 0).contains("96/min"));
        let spark: String = text(buffer, 0)
            .chars()
            .filter(|c| "▁▂▃▄▅▆▇█".contains(*c))
            .collect();
        assert_eq!(spark, format!("{}▂▄█", "▁".repeat(9)));
    }
    // Compare the complete source and bands panels, with both source sparklines.
    for y in 1..8 {
        for x in 0..120 {
            assert_eq!(buffers[0][(x, y)], buffers[1][(x, y)]);
        }
    }
}
#[test]
fn labels_resolve_rendered_collisions_in_argument_order() {
    // QGDXC-TUI-0004 QGDXC-TUI-0013
    assert_eq!(
        labels(&args(&[
            "abcdefghijklmnopq",
            "abcdefghijklmnop#2",
            "abcdefghijklmnopq",
            "abcdefghijklmn#2",
            "\u{202e}"
        ])),
        args(&[
            "abcdefghijklmnop",
            "abcdefghijklmn#2",
            "abcdefghijklmn#3",
            "abcdefghijklmn#4",
            "\\u{202e}"
        ])
    );
    // Replay arguments now name regular files (QGDXC-TUI-0003).
    assert!(Options::parse(&args(&["--replay", "/"])).is_err());
    let parsed = Options::parse(&args(&[
        "--replay",
        "Cargo.toml",
        "--replay",
        "./Cargo.toml",
    ]))
    .unwrap();
    assert_eq!(parsed.labels, args(&["Cargo", "Cargo#2"]));
    assert_eq!(
        labels(&args(&["source", "one", "one"])),
        args(&["source", "one", "one#2"])
    );
}
#[test]
fn every_band_edge_and_original_mode_precedence() {
    // QGDXC-TUI-0020 QGDXC-TUI-0030
    for &(name, low, high) in BANDS {
        assert_eq!(band(low), name);
        assert_eq!(band(high), name);
        assert_ne!(band(low - 1), name);
        assert_ne!(band(high + 1), name);
    }
    assert_eq!(band(1), "other");
    assert_eq!(band(54_000_000), "6m");
    assert_eq!(band(54_000_001), "5m");
    assert_eq!(band(7_500_000_000_001), "other");
    for (comment, expected) in [
        ("CQ usb CW", "SSB"),
        ("cw FT8", "CW"),
        ("(CW)\tFT8", "unknown"),
        ("PSK63 CQ", "PSK63"),
        ("", "unknown"),
    ] {
        let mut e = spot("Q1ABC", 14_074_000, comment);
        assert_eq!(mode(&e), expected);
        e.set("mode", "LSB");
        assert_eq!(mode(&e), "SSB");
    }
    for reported in [
        "CW", "SSB", "USB", "LSB", "AM", "FM", "FT8", "FT4", "RTTY", "PSK31", "PSK63", "JT65",
        "JS8", "MSK144", "Q65", "SSTV", "JTTY",
    ] {
        let event = spot("Q1ABC", 1, &reported.to_lowercase());
        assert_eq!(
            mode(&event),
            if matches!(reported, "USB" | "LSB") {
                "SSB"
            } else {
                reported
            }
        );
    }
    assert_eq!(frequency_text(14_349_949), "14349.9");
    assert_eq!(frequency_text(14_349_950), "14350.0");
    let e = spot("Q1ABC", 14_350_001, "40m FT8");
    assert_eq!(frequency_text(frequency(&e)), "14350.0");
    assert_eq!(band(frequency(&e)), "other");
}
#[test]
fn retention_filters_pause_scroll_and_eviction() {
    // QGDXC-TUI-0010 QGDXC-TUI-0012 QGDXC-TUI-0040 QGDXC-TUI-0042 QGDXC-TUI-0051
    let mut m = Model::new(args(&["one", "two"]), DEFAULT_KEEP, DEFAULT_KEEP);
    for source in [0, 1] {
        m.receive(
            source,
            spot("Q1ABC", 14_074_000, "FT8"),
            Duration::from_secs(0),
        );
    }
    assert_eq!(
        m.matching().iter().map(|s| s.source).collect::<Vec<_>>(),
        [1, 0]
    );
    key(&mut m, KeyCode::Char('p'));
    assert!(m.summary().contains("PAUSED +0"));
    let top = m.top;
    for _ in 0..10 {
        m.receive(1, spot("Q2ABC", 7_025_000, "CW"), Duration::from_secs(1));
    }
    assert_eq!(m.top, top);
    assert!(m.summary().contains("PAUSED +10"));
    key(&mut m, KeyCode::Down);
    assert_eq!(m.top, Some(1));
    key(&mut m, KeyCode::Home);
    assert_eq!(m.top, Some(12));
    key(&mut m, KeyCode::PageDown);
    assert_eq!(m.top, Some(2));
    key(&mut m, KeyCode::PageUp);
    assert_eq!(m.top, Some(12));
    key(&mut m, KeyCode::Down);
    key(&mut m, KeyCode::Up);
    assert_eq!(m.top, Some(12));
    // All four filters intersect, including an active empty set.
    m.filters.bands = Some(["40m".into()].into());
    m.filters.modes = Some(["CW".into()].into());
    m.filters.calls = patterns("q2*").unwrap();
    m.filters.kind = 1;
    assert_eq!(m.matching().len(), 10);
    m.filters.modes = Some(BTreeSet::new());
    assert_eq!(m.matching().len(), 0);
    key(&mut m, KeyCode::Char('c'));
    assert_eq!(m.matching().len(), 12);
    key(&mut m, KeyCode::Char('2'));
    assert_eq!(m.matching().len(), 1);
    assert_eq!(m.top, Some(1));
    for _ in 0..DEFAULT_KEEP + 1 {
        m.receive(0, spot("Q1ABC", 1, ""), Duration::from_secs(2));
    }
    assert_eq!(m.spots.len(), DEFAULT_KEEP);
    assert_eq!(m.top, m.matching().last().map(|s| s.id));
    assert_eq!(m.sources[0].count, 102);
    assert_eq!(m.sources[1].count, 11);
    assert!(m.summary().contains("retained:100 received:113"));
    key(&mut m, KeyCode::Char('p'));
    assert_eq!(m.top, Some(113));
    key(&mut m, KeyCode::Char('c'));
    assert!(m.sources.iter().all(|s| !s.hidden));
    for kind in [1, 2, 0] {
        key(&mut m, KeyCode::Char('k'));
        assert_eq!(m.filters.kind, kind);
    }
}
#[test]
fn synthetic_keys_edit_transactionally_and_quit_every_mode() {
    // QGDXC-TUI-0041 QGDXC-TUI-0043 QGDXC-TUI-0051 QGDXC-TUI-0060
    let mut m = Model::new(args(&["one"]), DEFAULT_KEEP, DEFAULT_KEEP);
    key(&mut m, KeyCode::Char('/'));
    for c in "q1*,q2?".chars() {
        key(&mut m, KeyCode::Char(c));
    }
    key(&mut m, KeyCode::Backspace);
    key(&mut m, KeyCode::Char('?'));
    key(&mut m, KeyCode::Enter);
    assert_eq!(m.filters.calls, args(&["Q1*", "Q2?"]));
    assert!(text(&grid(&m, 80, 24), 2).contains("call:2"));
    key(&mut m, KeyCode::Char('/'));
    key(&mut m, KeyCode::Char('q'));
    assert!(!m.quit);
    key(&mut m, KeyCode::Char('['));
    key(&mut m, KeyCode::Enter);
    assert!(matches!(m.editor, Editor::Calls { .. }));
    key(&mut m, KeyCode::Esc);
    assert_eq!(m.filters.calls.len(), 2);
    key(&mut m, KeyCode::Char('b'));
    key(&mut m, KeyCode::Down);
    key(&mut m, KeyCode::Char(' '));
    key(&mut m, KeyCode::Enter);
    assert_eq!(m.filters.bands, Some(["630m".into()].into()));
    key(&mut m, KeyCode::Char('m'));
    key(&mut m, KeyCode::Char(' '));
    key(&mut m, KeyCode::Esc);
    assert!(m.filters.modes.is_none());
    key(&mut m, KeyCode::Char('m'));
    key(&mut m, KeyCode::Down);
    key(&mut m, KeyCode::Up);
    key(&mut m, KeyCode::Char(' '));
    key(&mut m, KeyCode::Enter);
    assert_eq!(m.filters.modes, Some(["CW".into()].into()));
    assert!(text(&grid(&m, 80, 24), 2).contains("band:1 mode:1 call:2"));
    for entry in [
        "Q1*,,Q2*",
        ",",
        "Q1[A]",
        "Q1_",
        "Q1é",
        &"Q".repeat(21),
        &vec!["Q"; 17].join(","),
    ] {
        assert!(patterns(entry).is_err(), "{entry}");
    }
    let maximum = vec!["Q".repeat(20); 16].join(",");
    assert_eq!(maximum.len(), 335);
    assert_eq!(patterns(&maximum).unwrap().len(), 16);
    key(&mut m, KeyCode::Char('c'));
    key(&mut m, KeyCode::Char('/'));
    for c in maximum.chars().chain("extra".chars()) {
        key(&mut m, KeyCode::Char(c));
    }
    m.key(Input::Char('X'));
    let Editor::Calls {
        text: entry, error, ..
    } = &m.editor
    else {
        panic!()
    };
    assert_eq!(entry, &maximum);
    assert!(!error.is_empty());
    let editor_grid = grid(&m, 80, 24);
    let displayed: String = (2..7)
        .map(|y| text(&editor_grid, y).trim_end().to_owned())
        .collect();
    assert_eq!(displayed, maximum);
    key(&mut m, KeyCode::Esc);
    assert!(m.filters.calls.is_empty());
    key(&mut m, KeyCode::Char('/'));
    for c in format!("{maximum}x").chars() {
        key(&mut m, KeyCode::Char(c));
    }
    key(&mut m, KeyCode::Enter);
    assert!(m.filters.calls.is_empty());
    for open in ['/', 'b', 'm', 'l', '?', 'x'] {
        let mut m = Model::new(args(&["one"]), DEFAULT_KEEP, DEFAULT_KEEP);
        key(&mut m, KeyCode::Char(open));
        let event = Event::Key(event::KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        ));
        m.key(input(event).unwrap());
        assert!(m.quit);
    }
    key(&mut m, KeyCode::Esc);
    key(&mut m, KeyCode::Char('?'));
    assert!(text(&grid(&m, 80, 24), 0).contains("receive-only"));
}
#[test]
fn wildcard_pathological_processing_growth_is_bounded() {
    // QGDXC-TUI-0041: count DP time steps, rather than impose a wall-clock limit.
    let mut previous = 0;
    for length in [64, 128, 256, 512, 1024, 2048] {
        let pattern = "*Q".repeat(9) + "*Z";
        let (matched, work) = wildcard_work(&pattern, &"Q".repeat(length));
        assert!(!matched);
        assert_eq!(work, pattern.len() * length);
        if previous > 0 {
            assert_eq!(work, previous * 2);
        }
        previous = work;
    }
    // Also measure relative runtime growth on the adversarial failing suffix.
    // Median batches reduce scheduler noise; the limit is a growth ratio,
    // independent of absolute CPU speed. DP step counts above pin the bound.
    let pattern = "*Q".repeat(9) + "*Z";
    let measure = |length| {
        let call = "Q".repeat(length);
        let mut samples = vec![];
        for _ in 0..7 {
            let start = Instant::now();
            for _ in 0..32 {
                std::hint::black_box(wildcard_work(
                    std::hint::black_box(&pattern),
                    std::hint::black_box(&call),
                ));
            }
            samples.push(start.elapsed());
        }
        samples.sort();
        samples[3]
    };
    let small = measure(256);
    let large = measure(2048);
    assert!(
        large.as_nanos() <= small.as_nanos() * 16,
        "superlinear timing growth: {small:?} -> {large:?}"
    );
    for (p, c, yes) in [
        ("q1*", "Q1ABC", true),
        ("*/P", "Q1ABC/P", true),
        ("Q?ABC", "Q1ABC", true),
        ("Q1", "Q1ABC", false),
        ("*", "", true),
        ("?", "", false),
        ("Q*Q*Z", "QQQQZ", true),
    ] {
        assert_eq!(wildcard_work(p, c).0, yes);
    }
}
#[test]
fn hostile_scalars_cannot_write_outside_field_cells() {
    // QGDXC-TUI-0011 QGDXC-TUI-0013 QGDXC-TUI-0060
    let hostile = "\x1b[2J\x07\u{009b}\u{202e}\u{200b}\u{0301}界\x7f\n";
    let escaped =
        "\\u{001b}[2J\\u{0007}\\u{009b}\\u{202e}\\u{200b}\\u{0301}\\u{754c}\\u{007f}\\u{000a}";
    assert_eq!(sanitize(hostile), escaped);
    for width in 0..=80 {
        let mut terminal = Terminal::new(TestBackend::new(86, 3)).unwrap();
        terminal
            .draw(|frame| {
                for cell in &mut frame.buffer_mut().content {
                    cell.set_symbol("X");
                }
                render::field(frame, Rect::new(3, 1, width, 1), hostile, true);
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        for y in 0..3 {
            for x in 0..86 {
                if y != 1 || x < 3 || x >= 3 + width {
                    assert_eq!(buffer[(x, y)].symbol(), "X");
                }
            }
        }
        let expected = clip(escaped, width as usize, true);
        for (i, c) in expected.chars().enumerate() {
            assert_eq!(buffer[(3 + i as u16, 1)].symbol(), c.to_string());
        }
    }
    let mut m = Model::new(args(&["source-1234567890"]), DEFAULT_KEEP, DEFAULT_KEEP);
    m.receive(
        0,
        spot("Q12345678901234567890", 14_349_950, "\x1b[2J FT8 界"),
        Duration::from_secs(0),
    );
    let buffer = grid(&m, 80, 24);
    assert_eq!(
        text(&buffer, 4).chars().skip(2).take(4).collect::<String>(),
        "1832"
    );
    assert!(text(&buffer, 4).contains("14350.0"));
    assert!(text(&buffer, 4).contains("Q12345678~"));
    key(&mut m, KeyCode::Char('?'));
    assert!(text(&grid(&m, 80, 24), 10).contains("source-123456789"));
    assert!(buffer.content.iter().all(|c| {
        c.symbol()
            .chars()
            .all(|ch| (' '..='~').contains(&ch) || "╭╮╰╯─│▎●◐○¹²³⁴⁵⁶⁷⁸·▁▂▃▄▅▆▇█".contains(ch))
    }));
    assert!(!text(&buffer, 3).contains("SNR"));
    assert!(!text(&buffer, 3).contains("Src"));
}
#[test]
fn status_layout_small_resize_and_redraw_clock() {
    // QGDXC-TUI-0050 QGDXC-TUI-0053 QGDXC-TUI-0054 QGDXC-TUI-0060
    let mut m = Model::new(args(&["one", "two"]), DEFAULT_KEEP, DEFAULT_KEEP);
    assert_eq!(
        m.sources[0].line(Duration::from_secs(10)),
        "one   unknown spots:0 "
    );
    let mut event = ClusterEvent::new(Profile::Rbn, "connection_state");
    event.set("state", "blocked");
    event.set("reason", "retry_exhausted");
    m.receive(0, event, Duration::from_secs(0));
    m.sources[0].hidden = true;
    m.receive(0, spot("Q1ABC", 1, ""), Duration::from_secs(3));
    assert_eq!(m.sources[0].count, 1);
    assert!(m.sources[0].line(Duration::from_secs(10)).contains("age:7"));
    let normal = grid(&m, 80, 24);
    key(&mut m, KeyCode::Char('?'));
    assert!(text(&grid(&m, 80, 24), 10).contains("blocked retry_exhausted"));
    key(&mut m, KeyCode::Esc);
    assert!(text(&normal, 1).contains("off"));
    for (width, height) in [(79, 24), (80, 23), (1, 1), (0, 0)] {
        let small = grid(&m, width, height);
        if height > 0 && width > 0 {
            assert!(text(&small, 0).starts_with('E'));
        }
        assert!(small.content.iter().all(|c| c.symbol().is_ascii()));
    }
    let mut terminal = Terminal::new(TestBackend::new(79, 23)).unwrap();
    present(&mut terminal, &m, Duration::from_secs(10)).unwrap();
    terminal.backend_mut().resize(80, 24);
    present(&mut terminal, &m, Duration::from_secs(10)).unwrap();
    assert!(text(terminal.backend().buffer(), 0).starts_with(" QG DX Cluster "));
    m.receive(0, spot("Q1ABC", 1, ""), Duration::from_millis(3900));
    assert!(
        m.sources[0]
            .line(Duration::from_millis(4100))
            .contains("age:0")
    );
    assert!(
        m.sources[0]
            .line(Duration::from_millis(4900))
            .contains("age:1")
    );
    for open in ['/', 'b', 'm', '?'] {
        let mut small = Model::new(args(&["one"]), DEFAULT_KEEP, DEFAULT_KEEP);
        key(&mut small, KeyCode::Char(open));
        small.key_at_size(Input::Char('q'), 79, 23);
        assert!(small.quit);
    }
    let mut redraw = Redraw::default();
    let mut draws = vec![];
    for ms in 0..=3000 {
        if redraw.due(Duration::from_millis(ms)) {
            draws.push(ms);
        }
    }
    assert!(
        draws
            .windows(2)
            .all(|w| w[1] - w[0] >= 50 && w[1] - w[0] <= 1000)
    );
    assert_eq!(draws.len(), 61);
}
fn recording(at: f64, call: &str, count: usize) -> ReplayStream<Cursor<Vec<u8>>> {
    let mut r = Recorder::new(vec![], true).unwrap();
    r.note(
        at,
        "qg-dxc configuration: {\"call\":\"Q1ABC\",\"profile\":\"arcluster\"}",
    )
    .unwrap();
    r.open(at).unwrap();
    let lines = format!("DX de Q9XYZ: 14074.0 {call} FT8 1832Z\r\n").repeat(count);
    r.wire(at, "in", lines.as_bytes()).unwrap();
    r.close(at, "cancelled").unwrap();
    ReplayStream::new(Cursor::new(r.into_inner()), false).unwrap()
}
#[test]
fn replay_pacing_ties_continuations_completion_and_isolated_errors() {
    // QGDXC-TUI-0002 QGDXC-TUI-0003 QGDXC-TUI-0010 QGDXC-TUI-0014 QGDXC-TUI-0060
    let mut scheduler = Schedule::new(vec![
        Ok(recording(2.0, "Q1ABC", 20)),
        Ok(recording(1.0, "Q2ABC", 1)),
        Ok(recording(2.0, "Q3ABC", 1)),
        Err("broken\u{202e}".into()),
    ]);
    let mut m = Model::new(
        args(&["one", "two", "three", "four"]),
        DEFAULT_KEEP,
        DEFAULT_KEEP,
    );
    assert!(matches!(scheduler.poll(0.0), Some((3, Message::Error(_)))));
    assert!(scheduler.poll(0.999).is_none());
    let mut events = vec![];
    let mut complete = vec![];
    for now in [1.0, 1.999, 2.0, 100.0] {
        while let Some((source, message)) = scheduler.poll(now) {
            match message {
                Message::Event(e) => {
                    if e.kind == "spot" {
                        events.push(source);
                    }
                    m.receive(source, e, Duration::from_secs_f64(now));
                }
                Message::Complete => {
                    complete.push(source);
                    m.sources[source].complete = true;
                }
                _ => {}
            }
        }
        if now < 2.0 {
            assert_eq!(m.total, 1);
        }
    }
    assert_eq!(events, [vec![1], vec![0; 20], vec![2]].concat());
    assert_eq!(complete, [1, 0, 2]);
    assert!(scheduler.done());
    assert!(scheduler.poll(1000.0).is_none());
    assert!((0..24).any(|y| text(&grid(&m, 100, 24), y).contains("replay finished")));
    assert_eq!(m.sources[0].state, "disconnected");
    let mut invalid = Schedule::new(vec![Ok(ReplayStream::new(Cursor::new(b"{\"t\":0,\"note\":\"qg-dxc configuration: {\\\"call\\\":\\\"Q1ABC\\\",\\\"profile\\\":\\\"arcluster\\\"}\"}\ninvalid\n".to_vec()),false).unwrap()),Ok(recording(0.0,"Q4ABC",1))]);
    let mut errors = 0;
    let mut spots = 0;
    while let Some((_, message)) = invalid.poll(100.0) {
        match message {
            Message::Error(_) => errors += 1,
            Message::Event(e) if e.kind == "spot" => spots += 1,
            _ => {}
        }
    }
    assert_eq!((errors, spots), (1, 1));
}
#[test]
fn bounded_source_send_blocks_and_unrelated_session_progresses() {
    // QGDXC-TUI-0002 QGDXC-TUI-0052 QGDXC-TUI-0053
    let (tx, rx) = mpsc::sync_channel(CAPACITY);
    let mut delivery = Delivery::Tui(tx);
    for _ in 0..CAPACITY {
        delivery
            .enqueue(spot("Q1ABC", 1, ""), false, false)
            .unwrap();
    }
    let (ready, ready_rx) = mpsc::channel();
    let (done, done_rx) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        ready.send(()).unwrap();
        let result = delivery.enqueue(spot("Q2ABC", 1, ""), false, false);
        done.send(result).unwrap();
    });
    ready_rx.recv().unwrap();
    assert!(done_rx.recv_timeout(Duration::from_millis(20)).is_err());
    let (other_tx, other_rx) = mpsc::sync_channel(CAPACITY);
    let mut session = Session::new(Config::new(
        "example.invalid".into(),
        7300,
        "Q1ABC".into(),
        Profile::ArCluster,
    ))
    .unwrap();
    session.start(0).unwrap();
    session.connected(0, 1).unwrap();
    let mut live = Live {
        session,
        socket: None,
        recorder: None,
        clock: Instant::now(),
        json: false,
        output: Delivery::Tui(other_tx),
        receiver: None,
        closing: None,
        deferred_connect: false,
        stop: Arc::new(AtomicBool::new(false)),
    };
    live.input(b"DX de Q9XYZ: 14074 Q3ABC CW 1832Z\r\n")
        .unwrap();
    assert!(matches!(other_rx.try_recv().unwrap().message,Message::Event(e) if e.kind=="spot"));
    live.input(b"Password:").unwrap();
    assert_eq!(live.session.state, State::Blocked);
    rx.recv().unwrap();
    done_rx
        .recv_timeout(Duration::from_secs(1))
        .unwrap()
        .unwrap();
    thread.join().unwrap();
    let remaining: Vec<_> = rx.try_iter().collect();
    assert_eq!(remaining.len(), CAPACITY);
    assert!(
        matches!(&remaining.last().unwrap().message,Message::Event(e) if e.get("call")==Some("Q2ABC"))
    );
    let cancel = Arc::new(AtomicBool::new(false));
    let cancelled = cancel.clone();
    let (closed, closed_rx) = mpsc::channel();
    let (_connection_tx, connection_rx) = mpsc::channel();
    let connector = std::thread::spawn(move || {
        while !cancelled.load(Ordering::Relaxed) {
            std::thread::yield_now();
        }
        closed.send(()).unwrap();
    });
    live.receiver = Some(crate::Connector {
        rx: connection_rx,
        cancel,
        worker: Some(connector),
    });
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _live = live;
        panic!("synthetic unwind");
    }));
    closed_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(result.is_err());
}

#[test]
fn terminal_state_restores_on_return_error_partial_entry_and_unwind() {
    // QGDXC-TUI-0052: fake terminal operations, with both initial raw states.
    use std::sync::Mutex;
    struct Fake {
        log: Arc<Mutex<Vec<&'static str>>>,
        raw: bool,
        fail: &'static str,
    }
    impl Fake {
        fn record(&mut self, op: &'static str) -> io::Result<()> {
            self.log.lock().unwrap().push(op);
            if op == self.fail {
                Err(io::Error::other("synthetic terminal failure"))
            } else {
                Ok(())
            }
        }
    }
    impl ScreenIo for Fake {
        fn raw(&mut self) -> io::Result<bool> {
            self.record("query")?;
            Ok(self.raw)
        }
        fn set_raw(&mut self, enabled: bool) -> io::Result<()> {
            self.record(if enabled { "raw" } else { "cooked" })
        }
        fn enter_screen(&mut self) -> io::Result<()> {
            self.record("enter")
        }
        fn leave_screen(&mut self) -> io::Result<()> {
            self.record("leave")
        }
        fn flush(&mut self) -> io::Result<()> {
            self.record("flush")
        }
    }
    for raw in [false, true] {
        for outcome in ["return", "error", "panic"] {
            let log = Arc::new(Mutex::new(vec![]));
            let operations = log.clone();
            let _ = std::panic::catch_unwind(move || -> io::Result<()> {
                let _screen = Screen::enter(Fake {
                    log: operations,
                    raw,
                    fail: "",
                })?;
                match outcome {
                    "panic" => panic!("synthetic interface panic"),
                    "error" => Err(io::Error::other("synthetic draw failure")),
                    _ => Ok(()),
                }
            });
            let actual = log.lock().unwrap().clone();
            let expected = if raw {
                vec!["query", "enter", "flush", "leave", "flush"]
            } else {
                vec!["query", "raw", "enter", "flush", "leave", "cooked", "flush"]
            };
            assert_eq!(actual, expected);
        }
    }
    for (fail, expected) in [
        ("query", vec!["query", "flush"]),
        ("raw", vec!["query", "raw", "cooked", "flush"]),
        (
            "enter",
            vec!["query", "raw", "enter", "leave", "cooked", "flush"],
        ),
        (
            "flush",
            vec!["query", "raw", "enter", "flush", "leave", "cooked", "flush"],
        ),
    ] {
        let log = Arc::new(Mutex::new(vec![]));
        assert!(
            Screen::enter(Fake {
                log: log.clone(),
                raw: false,
                fail
            })
            .is_err()
        );
        assert_eq!(*log.lock().unwrap(), expected);
    }
    for visible in [false, true] {
        let mut backend = TestBackend::new(80, 24);
        if visible {
            backend.show_cursor().unwrap();
        } else {
            backend.hide_cursor().unwrap();
        }
        let mut terminal = Terminal::new(backend).unwrap();
        present(
            &mut terminal,
            &Model::new(args(&["one"]), DEFAULT_KEEP, DEFAULT_KEEP),
            Duration::from_secs(0),
        )
        .unwrap();
        assert_eq!(terminal.backend().cursor_visible(), visible);
    }
}

#[test]
fn callsign_key_limit_rejects_whole_edit_and_keeps_ctrl_c_responsive() {
    // QGDXC-TUI-0041 QGDXC-TUI-0051
    let mut m = Model::new(args(&["one"]), DEFAULT_KEEP, DEFAULT_KEEP);
    m.filters.calls = patterns("Q1*").unwrap();
    key(&mut m, KeyCode::Char('/'));
    for _ in 0..3 {
        key(&mut m, KeyCode::Backspace);
    }
    let maximum = vec!["Q".repeat(20); 16].join(",");
    for c in maximum.chars() {
        key(&mut m, KeyCode::Char(c));
    }
    let Editor::Calls { text, .. } = &m.editor else {
        panic!()
    };
    assert_eq!(text.len(), 335);
    key(&mut m, KeyCode::Char('X'));
    // Enter cannot commit the valid but truncated prefix, even after Backspace.
    key(&mut m, KeyCode::Enter);
    assert_eq!(m.filters.calls, args(&["Q1*"]));
    key(&mut m, KeyCode::Backspace);
    key(&mut m, KeyCode::Enter);
    assert_eq!(m.filters.calls, args(&["Q1*"]));
    let Editor::Calls { text, .. } = &m.editor else {
        panic!()
    };
    assert!(text.len() <= 335);
    m.key(
        input(Event::Key(event::KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        )))
        .unwrap(),
    );
    assert!(m.quit);
}

#[test]
fn replay_configuration_rejects_directory_before_open() {
    // QGDXC-TUI-0003
    let error = Options::parse(&args(&["--replay", "."]))
        .err()
        .expect("directory accepted");
    assert!(error.contains("configuration: --replay ."), "{error}");
    assert!(error.contains("regular file"), "{error}");
}

#[cfg(unix)]
#[test]
fn replay_configuration_rejects_stalled_fifo_before_open() {
    // QGDXC-TUI-0003 QGDXC-TUI-0052
    let path = std::env::temp_dir().join(format!("qg-dxc-fifo-{}", std::process::id()));
    let status = match std::process::Command::new("mkfifo").arg(&path).status() {
        Ok(status) => status,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            eprintln!("skipping FIFO test: mkfifo is unavailable");
            return;
        }
        Err(e) => panic!("mkfifo: {e}"),
    };
    assert!(status.success());
    let argument = path.to_str().unwrap().to_owned();
    let (tx, rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let result = Options::parse(&args(&["--replay", &argument])).err();
        tx.send(result).unwrap();
    });
    let result = rx.recv_timeout(Duration::from_secs(1));
    std::fs::remove_file(&path).unwrap();
    let error = result
        .expect("FIFO validation stalled")
        .expect("FIFO accepted");
    worker.join().unwrap();
    assert!(
        error.contains(&format!("--replay {}", path.display())),
        "{error}"
    );
    assert!(error.contains("regular file"), "{error}");
    // metadata follows symlinks: a regular target is supported, a FIFO is not.
    let link = path.with_extension("link");
    std::os::unix::fs::symlink(std::env::current_dir().unwrap().join("Cargo.toml"), &link).unwrap();
    assert!(Options::parse(&args(&["--replay", link.to_str().unwrap()])).is_ok());
    std::fs::remove_file(link).unwrap();
}

#[test]
fn filters_accept_exact_kind_and_listed_other_unknown_mode_rows() {
    // QGDXC-TUI-0040: assert row identities, including rejected rows.
    let mut m = Model::new(args(&["one"]), DEFAULT_KEEP, DEFAULT_KEEP);
    let modes = [
        "CW", "SSB", "AM", "FM", "FT8", "FT4", "RTTY", "JTTY", "PSK31", "",
    ];
    for (kind, comment) in ["human", "skimmer"]
        .into_iter()
        .flat_map(|k| modes.map(|m| (k, m)))
    {
        let mut e = spot("Q1ABC", 14_074_000, comment);
        e.set("spot_kind", kind);
        m.receive(0, e, Duration::ZERO);
    }
    let ids = |m: &Model| m.matching().iter().map(|s| s.id).collect::<BTreeSet<_>>();
    assert_eq!(ids(&m), (1..=20).collect()); // inactive mode, kind all
    for (kind, offset) in [(1, 0), (2, 10), (0, 0)] {
        m.filters.kind = kind;
        let all: BTreeSet<_> = match kind {
            1 => (1..=10).collect(),
            2 => (11..=20).collect(),
            _ => (1..=20).collect(),
        };
        assert_eq!(ids(&m), all);
        for (i, group) in MODES.iter().enumerate() {
            m.filters.modes = Some([(*group).to_owned()].into());
            let expected = if kind == 0 {
                [i as u64 + 1, i as u64 + 11].into()
            } else {
                [i as u64 + 1 + offset].into()
            };
            assert_eq!(ids(&m), expected, "kind {kind}, mode {group}");
        }
        m.filters.modes = Some(BTreeSet::new());
        assert!(ids(&m).is_empty());
        m.filters.modes = None;
        assert_eq!(ids(&m), all);
    }
    m.filters.bands = Some(BTreeSet::new());
    assert!(ids(&m).is_empty());
    m.filters.bands = None;
    assert_eq!(ids(&m).len(), 20);
}

#[test]
fn replay_error_aborts_and_finalizes_current_connection_once() {
    // QGDXC-TUI-0003 QGDXC-TUI-0052
    for suffix in [&b"\r"[..], &b""[..], &b"\xff\xfa\x01"[..]] {
        let mut r = Recorder::new(vec![], true).unwrap();
        r.note(
            0.0,
            "qg-dxc configuration: {\"call\":\"Q1ABC\",\"profile\":\"arcluster\"}",
        )
        .unwrap();
        r.open(0.0).unwrap();
        r.wire(
            0.0,
            "in",
            &[b"DX de Q9XYZ: 14074 Q2ABC CW 1832Z".as_slice(), suffix].concat(),
        )
        .unwrap();
        let mut bytes = r.into_inner();
        bytes.extend_from_slice(b"invalid JSON\n");
        let mut schedule = Schedule::new(vec![Ok(
            ReplayStream::new(Cursor::new(bytes), false).unwrap()
        )]);
        let (mut errors, mut finalized, mut content) = (0, 0, 0);
        while let Some((_, message)) = schedule.poll(0.0) {
            match message {
                Message::Error(_) => errors += 1,
                Message::Event(e) if e.get("reason") == Some("cancelled") => finalized += 1,
                Message::Event(e) if e.kind == "spot" || e.kind == "unrecognized_line" => {
                    content += 1
                }
                _ => {}
            }
        }
        schedule.cancel();
        assert_eq!((errors, finalized, content), (1, 1, 1), "{suffix:?}");
        assert!(schedule.poll(10000.0).is_none());
    }
}

fn find(buffer: &Buffer, y: u16, value: &str) -> u16 {
    let line = text(buffer, y);
    line[..line
        .find(value)
        .unwrap_or_else(|| panic!("missing {value:?} in {line:?}"))]
        .chars()
        .count() as u16
}
#[test]
fn approved_wide_and_compact_content_and_styles() {
    // QGDXC-TUI-0070 QGDXC-TUI-0073 QGDXC-TUI-0060
    use ratatui::style::{Color, Modifier};
    let mut m = Model::new(
        args(&["dx.example.org", "rbn.example.org"]),
        DEFAULT_KEEP,
        DEFAULT_KEEP,
    );
    m.utc_epoch = 1_790_000_000;
    let mut ready = ClusterEvent::new(Profile::Rbn, "connection_state");
    ready.set("state", "ready");
    m.receive(1, ready, Duration::ZERO);
    let mut skimmer = spot("Q2SKM", 7_025_000, "CW CQ");
    skimmer.set("spot_kind", "skimmer");
    skimmer.set("snr_db", qso_graph_dxcluster::Value::Integer(27));
    skimmer.set("speed", qso_graph_dxcluster::Value::Integer(24));
    skimmer.set("speed_unit", "wpm");
    m.receive(1, skimmer, Duration::from_secs(9));
    m.receive(
        0,
        spot("Q1HUM", 14_074_000, "FT8 up 2"),
        Duration::from_secs(9),
    );
    let wide = grid(&m, 120, 36);
    assert!(text(&wide, 0).contains("1 source live  ·  2 spots  ·  2/min"));
    assert!(text(&wide, 0).ends_with("Z "));
    assert!(text(&wide, 1).contains("sources"));
    assert!(text(&wide, 1).contains("bands"));
    assert!(text(&wide, 1).contains("last 15 min"));
    assert!(text(&wide, 3).contains("² ● rbn.example.org"));
    assert!(text(&wide, 5).contains("160"));
    assert!(text(&wide, 8).contains("spots newest first"));
    assert!(text(&wide, 9).contains("SNR"));
    let human_y = 10;
    assert_eq!(wide[(1, human_y)].symbol(), "▎");
    assert_eq!(wide[(1, human_y)].fg, Color::Rgb(255, 180, 84));
    let band_x = find(&wide, human_y, "20m");
    // Plant-and-prove target: a regression to the default foreground must fail.
    assert_eq!(wide[(band_x, human_y)].fg, Color::Rgb(145, 215, 106));
    assert_eq!(wide[(band_x, human_y)].bg, Color::Rgb(30, 51, 67));
    let dx_x = find(&wide, human_y, "Q1HUM");
    assert!(wide[(dx_x, human_y)].modifier.contains(Modifier::BOLD));
    assert_eq!(wide[(dx_x, human_y)].fg, Color::Rgb(255, 255, 255));
    assert_eq!(wide[(0, human_y)].bg, Color::Reset);
    let mode_x = find(&wide, human_y, "FT8");
    assert_eq!(wide[(mode_x, human_y)].fg, Color::Rgb(212, 166, 255));
    assert_eq!(wide[(88, 4)].fg, Color::Rgb(145, 215, 106));
    assert_eq!(wide[(88, 4)].symbol(), "█");
    assert_eq!(wide[(89, 6)].symbol(), "1");
    assert_eq!(wide[(50, 33)].bg, Color::Reset);
    let skimmer_y = 11;
    assert_eq!(wide[(1, skimmer_y)].symbol(), " ");
    let snr_x = find(&wide, skimmer_y, "27dB");
    assert_eq!(wide[(snr_x, skimmer_y)].fg, Color::Rgb(127, 217, 98));
    assert!(text(&wide, skimmer_y).contains("24  rbn"));
    assert!(
        !wide[(find(&wide, skimmer_y, "Q2SKM"), skimmer_y)]
            .modifier
            .contains(Modifier::BOLD)
    );
    assert!(text(&wide, 34).contains("1 human · 1 skimmer"));
    assert!(text(&wide, 34).contains("2 received"));
    assert!(text(&wide, 35).contains("t theme"));
    assert!(text(&wide, 35).ends_with("color "));
    let compact = grid(&m, 80, 24);
    assert!(text(&compact, 22).contains("1 human · 1 skimmer"));
    assert!(text(&compact, 22).contains("2 received"));
    assert!(text(&compact, 1).contains("¹○ dx"));
    assert!(text(&compact, 2).contains("spots newest first"));
    assert!(text(&compact, 3).contains("Comment"));
    for removed in ["SNR", "WPM", "Src"] {
        assert!(!text(&compact, 3).contains(removed));
    }
    assert!(text(&compact, 4).contains("Q1HUM"));
    assert_eq!(
        compact[(find(&compact, 4, "20m"), 4)].fg,
        Color::Rgb(145, 215, 106)
    );
    key(&mut m, KeyCode::Char('p'));
    m.receive(0, spot("Q3NEW", 28_050_000, "CW"), Duration::from_secs(10));
    for (w, h, title) in [(120, 36, 8), (80, 24, 2)] {
        let paused = grid(&m, w, h);
        let x = find(&paused, title, " PAUSED · 1 new ");
        assert_eq!(paused[(x, title)].bg, Color::Rgb(255, 180, 84));
        assert_eq!(paused[(x, title)].fg, Color::Rgb(15, 19, 22));
        assert!(text(&paused, title + 2).contains("Q1HUM"));
    }
    m.filters.bands = Some(["20m".into()].into());
    m.filters.modes = Some(["FT8".into()].into());
    m.filters.calls = patterns("Q1*").unwrap();
    m.filters.kind = 1;
    m.sources[1].hidden = true;
    for (w, h, title) in [(120, 36, 8), (80, 24, 2)] {
        let paused = grid(&m, w, h);
        assert!(text(&paused, title).contains("PAUSED"));
        assert!(text(&paused, h - 3).contains("band:1 mode:1 call:1 kind:human hidden:1"));
        assert!(text(&paused, h - 2).contains("2 human · 1 skimmer"));
    }
}
#[test]
fn no_color_override_theme_cycle_and_depth() {
    // QGDXC-TUI-0071 QGDXC-TUI-0072 QGDXC-TUI-0060
    use super::theme::{Depth, Theme};
    use ratatui::style::Color;
    assert_eq!(Theme::initial(None, Some("")), Theme::Color);
    assert_eq!(Theme::initial(None, None), Theme::Color);
    assert_eq!(
        Theme::initial(Some(Theme::Daylight), Some("1")),
        Theme::Daylight
    );
    let mut m = Model::new(args(&["dx.example.org"]), DEFAULT_KEEP, DEFAULT_KEEP);
    m.appearance.theme = Theme::initial(None, Some("1"));
    m.receive(0, spot("Q1ABC", 14_074_000, "FT8"), Duration::from_secs(10));
    for (w, h) in [(120, 36), (80, 24)] {
        let mono = grid(&m, w, h);
        assert!(
            mono.content
                .iter()
                .all(|c| c.fg == Color::Reset && c.bg == Color::Reset)
        );
    }
    for expected in [
        Theme::Color,
        Theme::Daylight,
        Theme::Phosphor,
        Theme::Amber,
        Theme::Mono,
    ] {
        key(&mut m, KeyCode::Char('t'));
        assert_eq!(m.appearance.theme, expected);
        let expected_band = match expected {
            Theme::Color => Color::Rgb(145, 215, 106),
            Theme::Daylight => Color::Rgb(47, 138, 47),
            Theme::Phosphor => Color::Rgb(63, 227, 108),
            Theme::Amber => Color::Rgb(255, 176, 0),
            Theme::Mono => Color::Reset,
        };
        let themed = grid(&m, 120, 36);
        assert_eq!(themed[(find(&themed, 10, "20m"), 10)].fg, expected_band);
        assert!(text(&grid(&m, 120, 36), 35).ends_with(&format!("{} ", expected.name())));
    }
    m.appearance.theme = Theme::Color;
    key(&mut m, KeyCode::Char('/'));
    key(&mut m, KeyCode::Char('t'));
    assert_eq!(m.appearance.theme, Theme::Color);
    assert!(matches!(&m.editor, Editor::Calls{text,..} if text == "t"));
    key(&mut m, KeyCode::Esc);
    for (ct, term, expected) in [
        ("truecolor", "xterm", Depth::TrueColor),
        ("24bit", "xterm-256color", Depth::TrueColor),
        ("", "xterm-256color", Depth::Indexed),
        ("", "vt100", Depth::Ansi),
    ] {
        m.appearance.depth = Depth::detect(ct, term);
        assert_eq!(m.appearance.depth, expected);
        let buffer = grid(&m, 120, 36);
        assert!(buffer.content.iter().all(|c| match expected {
            Depth::TrueColor => !matches!(c.fg, Color::Indexed(_)),
            Depth::Indexed => !matches!(c.fg, Color::Rgb(..)),
            Depth::Ansi => !matches!(c.fg, Color::Rgb(..) | Color::Indexed(_)),
        }));
    }
    let valid = Options::parse(&args(&["--replay", "Cargo.toml", "--theme", "amber"])).unwrap();
    assert_eq!(valid.theme, Theme::Amber);
    for value in ["bogus", "COLOR", ""] {
        assert!(Options::parse(&args(&["--replay", "Cargo.toml", "--theme", value])).is_err());
    }
}
#[test]
fn activity_retry_highlight_and_optional_fields_use_typed_data() {
    // QGDXC-TUI-0073 QGDXC-TUI-0070
    use ratatui::style::Color;
    let mut m = Model::new(args(&["dx.example.org"]), DEFAULT_KEEP, DEFAULT_KEEP);
    let mut retry = ClusterEvent::new(Profile::Rbn, "connection_state");
    retry.set("state", "backoff");
    retry.set("retry_in_ms", qso_graph_dxcluster::Value::Integer(12500));
    m.receive(0, retry, Duration::ZERO);
    assert!(text(&grid(&m, 120, 36), 2).contains("retry 3s"));
    // Text that resembles metrics does not populate the typed columns.
    m.receive(
        0,
        spot("Q1ABC", 14_074_000, "CW 27 dB 24 WPM"),
        Duration::from_secs(9),
    );
    let b = grid(&m, 120, 36);
    let snr_x = find(&b, 9, "SNR") - 2;
    assert!((snr_x..snr_x + 10).all(|x| b[(x, 10)].symbol() == " "));
    let mut terminal = Terminal::new(TestBackend::new(120, 36)).unwrap();
    present(&mut terminal, &m, Duration::from_secs(12)).unwrap();
    assert_eq!(terminal.backend().buffer()[(20, 10)].bg, Color::Reset);
    // Counters survive retention eviction and filtering, with a bounded 15-minute history.
    for second in 10..=920 {
        m.receive(
            0,
            spot("Q1ABC", 14_074_000, "CW"),
            Duration::from_secs(second),
        );
    }
    assert_eq!(m.activity.len(), 900);
    m.sources[0].hidden = true;
    assert!(text(&grid(&m, 120, 36), 2).contains("off"));
    let eight = Model::new(
        (1..=8).map(|n| format!("source{n}.example.org")).collect(),
        DEFAULT_KEEP,
        DEFAULT_KEEP,
    );
    for (w, h) in [(100, 24), (120, 36), (80, 24)] {
        let b = grid(&eight, w, h);
        assert!(text(&b, h - 2).contains("0 human · 0 skimmer"));
        assert!(render::list_rows(w, h, 8) > 0);
    }
}

#[test]
fn source_details_are_plain_english_and_only_for_nonready_sources() {
    // QGDXC-TUI-0050 QGDXC-TUI-0070 QGDXC-TUI-0060
    let mut m = Model::new(args(&["one", "two"]), DEFAULT_KEEP, DEFAULT_KEEP);
    for source in &mut m.sources {
        source.state = "ready".into();
        source.reason = "setup_complete".into();
    }
    let ready = grid(&m, 120, 36);
    for y in 4..7 {
        assert!(
            text(&ready, y)
                .chars()
                .take(60)
                .all(|c| c == ' ' || c == '│')
        );
    }
    assert!(!(0..36).any(|y| text(&ready, y).contains("age:")));
    for (state, reason, retry, complete, diagnostic, expected) in [
        ("connecting", "", None, false, "", "² connecting"),
        (
            "backoff",
            "connection_refused",
            Some(Duration::from_millis(21001)),
            false,
            "",
            "² retrying in 12s: connection refused",
        ),
        (
            "blocked",
            "retry_exhausted",
            None,
            false,
            "",
            "² connection failed: retries exhausted",
        ),
        (
            "ready",
            "setup_complete",
            None,
            true,
            "",
            "² replay finished",
        ),
        (
            "ready",
            "setup_complete",
            None,
            false,
            "invalid recording",
            "² error: invalid recording",
        ),
        (
            "await_login",
            "connected",
            None,
            false,
            "",
            "² waiting for login",
        ),
    ] {
        let source = &mut m.sources[1];
        source.state = state.into();
        source.reason = reason.into();
        source.retry_at = retry;
        source.complete = complete;
        source.diagnostic = diagnostic.into();
        let buffer = grid(&m, 120, 36);
        assert!(text(&buffer, 4).contains(expected), "{}", text(&buffer, 4));
        assert!(!text(&buffer, 4).contains("age:"));
        assert!(!text(&buffer, 5).contains("¹"));
    }
    m.sources[1].retry_at = Some(Duration::from_secs(1));
    m.sources[1].diagnostic.clear();
    assert!(text(&grid(&m, 120, 36), 4).contains("retrying in 0s"));
    key(&mut m, KeyCode::Char('?'));
    assert!(!text(&grid(&m, 120, 36), 10).contains("age:"));
}

#[test]
fn zero_spark_cells_keep_full_extent_and_dim_style() {
    // QGDXC-TUI-0070 QGDXC-TUI-0073 QGDXC-TUI-0060
    use theme::Role;
    let mut m = Model::new(args(&["one", "two"]), DEFAULT_KEEP, DEFAULT_KEEP);
    for (w, h) in [(120, 36), (80, 24)] {
        let buffer = grid(&m, w, h);
        let start = find(&buffer, 0, "▁▁▁▁▁▁▁▁▁▁▁▁");
        for x in start..start + 12 {
            assert_eq!(buffer[(x, 0)].fg, m.appearance.style(Role::Dim).fg.unwrap());
        }
    }
    let empty = grid(&m, 120, 36);
    for y in [2, 3] {
        assert!(text(&empty, y).contains("▁▁▁▁▁▁▁▁"));
    }
    m.receive(1, spot("Q1ABC", 14_025_000, "CW"), Duration::from_secs(9));
    let buffer = grid(&m, 120, 36);
    for (y, trend) in [(0, "▁▁▁▁▁▁▁▁▁▁▁█"), (3, "▁▁▁▁▁▁▁█")]
    {
        let start = find(&buffer, y, trend);
        let last = start + trend.chars().count() as u16 - 1;
        for x in start..last {
            assert_eq!(buffer[(x, y)].fg, m.appearance.style(Role::Dim).fg.unwrap());
        }
        assert_eq!(
            buffer[(last, y)].fg,
            m.appearance.style(Role::Spark).fg.unwrap()
        );
    }
}

#[test]
fn comments_remove_only_exact_visible_typed_prefixes_without_mutation() {
    // QGDXC-TUI-0073 QGDXC-TUI-0060
    use qso_graph_dxcluster::Value;
    for (mode, snr, speed, unit, comment, expected) in [
        ("CW", 19, Some(17), "wpm", "CW    19 dB  17 WPM  CQ", "CQ"),
        ("CW", -2, Some(17), "wpm", "CW -2 dB 17 WPM DX", "DX"),
        (
            "CW",
            19,
            Some(17),
            "wpm",
            "CW +19 dB 17 WPM BEACON",
            "BEACON",
        ),
        (
            "CW",
            19,
            Some(17),
            "wpm",
            "CW 19 dB 17 WPM NCDXF B",
            "NCDXF B",
        ),
        ("FT8", 19, None, "", "FT8 19 dB CQ", "CQ"),
        ("FT8", 19, None, "", "FT8 19 dB slot CQ", "slot CQ"),
        (
            "RTTY",
            19,
            Some(45),
            "bps",
            "RTTY 19 dB 45 BPS CQ",
            "45 BPS CQ",
        ),
        (
            "CW",
            18,
            Some(17),
            "wpm",
            "CW 19 dB 17 WPM CQ",
            "CW 19 dB 17 WPM CQ",
        ),
        (
            "CW",
            19,
            Some(18),
            "wpm",
            "CW 19 dB 17 WPM CQ",
            "CW 19 dB 17 WPM CQ",
        ),
        (
            "CW",
            19,
            Some(17),
            "wpm",
            "CW 19 dB 17 BPS CQ",
            "CW 19 dB 17 BPS CQ",
        ),
        ("CW", 19, Some(17), "wpm", "CW 19 dB CQ", "CW 19 dB CQ"),
        ("CW", 19, Some(17), "wpm", "FT8 19 dB CQ", "FT8 19 dB CQ"),
        ("CW", 19, Some(17), "wpm", "CW CQ", "CW CQ"),
        (
            "CW",
            1000,
            Some(17),
            "wpm",
            "CW 1000 dB 17 WPM CQ",
            "CW 1000 dB 17 WPM CQ",
        ),
        (
            "CW",
            19,
            Some(1000),
            "wpm",
            "CW 19 dB 1000 WPM CQ",
            "CW 19 dB 1000 WPM CQ",
        ),
    ] {
        let mut m = Model::new(args(&["one"]), DEFAULT_KEEP, DEFAULT_KEEP);
        let mut event = spot("Q1ABC", 14_025_000, comment);
        event.set("mode", mode);
        event.set("snr_db", Value::Integer(snr));
        if let Some(speed) = speed {
            event.set("speed", Value::Integer(speed));
            event.set("speed_unit", unit);
        }
        let original = event.clone();
        m.receive(0, event, Duration::ZERO);
        let wide = grid(&m, 120, 36);
        let x = find(&wide, 9, "Comment") as usize;
        assert_eq!(
            text(&wide, 10)
                .chars()
                .skip(x)
                .take(119 - x)
                .collect::<String>()
                .trim_end(),
            expected
        );
        let compact = grid(&m, 80, 24);
        let x = find(&compact, 3, "Comment") as usize;
        let actual: String = text(&compact, 4).chars().skip(x).take(79 - x).collect();
        assert_eq!(actual.trim_end(), clip(comment, 79 - x, false).trim_end());
        assert_eq!(m.spots[0].event, original);
    }
    let mut m = Model::new(args(&["one"]), DEFAULT_KEEP, DEFAULT_KEEP);
    m.receive(
        0,
        spot("Q1ABC", 14_025_000, "CW 19 dB 17 WPM CQ"),
        Duration::ZERO,
    );
    assert!(text(&grid(&m, 120, 36), 10).contains("CW 19 dB 17 WPM CQ"));
}

#[test]
fn compact_source_labels_and_header_grammar() {
    // QGDXC-TUI-0070 QGDXC-TUI-0060
    let mut m = Model::new(args(&["q1abc", "q2xyz"]), DEFAULT_KEEP, DEFAULT_KEEP);
    for (count, grammar) in [
        (0, "0 sources live"),
        (1, "1 source live"),
        (2, "2 sources live"),
    ] {
        for (i, source) in m.sources.iter_mut().enumerate() {
            source.state = if i < count { "ready" } else { "connecting" }.into();
        }
        for (w, h) in [(120, 36), (80, 24)] {
            let buffer = grid(&m, w, h);
            assert!(text(&buffer, 0).contains(grammar));
        }
    }
    let compact = grid(&m, 80, 24);
    assert!(text(&compact, 1).contains("¹● q1abc 0"));
    assert!(text(&compact, 1).contains("²● q2xyz 0"));
}
#[test]
fn adif_modes_full_source_labels_and_blank_unknown_mode() {
    // QGDXC-TUI-0070
    assert_eq!(mode(&spot("Q1ABC", 475_000, "MFSK")), "MFSK");
    assert_eq!(mode(&spot("Q1ABC", 7_073_000, "mfsk16 cq")), "MFSK16");
    assert_eq!(mode(&spot("Q1ABC", 7_073_000, "olivia cq")), "OLIVIA");
    assert_eq!(
        mode(&spot("Q1ABC", 7_153_000, "FB SIGNAL UP UP")),
        "unknown"
    );
    let mut m = Model::new(
        args(&["dxnode.example.org", "rbn.example.org"]),
        DEFAULT_KEEP,
        DEFAULT_KEEP,
    );
    m.receive(
        0,
        spot("Q1HUM", 7_153_000, "FB SIGNAL UP UP"),
        Duration::ZERO,
    );
    m.receive(0, spot("Q2PSK", 14_070_000, "PSK125 cq"), Duration::ZERO);
    let wide = grid(&m, 120, 36);
    let row = |call: &str| {
        (10..34)
            .map(|y| text(&wide, y))
            .find(|t| t.contains(call))
            .unwrap()
    };
    let unknown = row("Q1HUM");
    assert!(!unknown.contains("unk"), "{unknown}");
    assert!(unknown.contains("  dxnode "), "{unknown}");
    assert!(!unknown.contains('~'), "{unknown}");
    assert!(row("Q2PSK").contains("PSK125 "), "{}", row("Q2PSK"));
}
#[test]
fn scrolling_down_holds_the_view_and_home_resumes() {
    // QGDXC-TUI-0051
    let mut m = Model::new(args(&["one"]), 100, 100);
    for (i, call) in ["Q1AAA", "Q1BBB", "Q1CCC"].into_iter().enumerate() {
        m.receive(
            0,
            spot(call, 14_074_000, "FT8"),
            Duration::from_secs(i as u64),
        );
    }
    key(&mut m, KeyCode::Down);
    assert!(m.paused, "scrolling down must pause");
    let held = m.top;
    m.receive(0, spot("Q1NEW", 14_074_000, "FT8"), Duration::from_secs(9));
    assert_eq!(m.top, held, "a new spot must not move a scrolled view");
    assert_eq!(m.paused_new(), 1);
    key(&mut m, KeyCode::Home);
    assert!(!m.paused, "Home must resume");
    let newest = m.matching().first().map(|s| s.id);
    assert_eq!(m.top, newest);
    // At the newest spot, Up neither pauses nor moves.
    key(&mut m, KeyCode::Up);
    assert!(!m.paused);
}

fn sources_file(name: &str, text: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("qg-dxc-tui-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("sources.json");
    std::fs::write(&path, text).unwrap();
    path
}
const SAVED: &str = r#"{"version":1,"call":"Q1ABC","sources":[
  {"name":"Home node","node":"dx.example.invalid:7300","selected":true},
  {"name":"Home node and more","node":"dx2.example.invalid:7300"},
  {"name":"RBN CW","node":"rbn.example.invalid:7000","profile":"rbn","selected":true}
]}"#;
#[test]
fn sources_option_rules_and_chosen_sources_run_as_nodes() {
    // QGDXC-TUI-0006 QGDXC-TUI-0008
    let path = sources_file("options", SAVED);
    let file = path.to_str().unwrap();
    let mut options = Options::parse(&args(&["--sources", file, "--keep-human", "50"])).unwrap();
    assert!(options.sources.is_empty() && options.labels.is_empty());
    assert_eq!(options.keep_human, 50);
    assert_eq!(options.saved.as_ref().unwrap().entries.len(), 3);
    options.choose(&[0, 1, 2]);
    assert_eq!(options.labels, ["Home node", "Home node and mo", "RBN CW"]);
    let hosts: Vec<_> = options
        .sources
        .iter()
        .map(|s| match s {
            Source::Node(c) => (
                c.host.clone(),
                c.call.clone(),
                c.profile,
                c.retain_third_party_data,
            ),
            Source::Replay(_) => panic!("replay"),
        })
        .collect();
    assert_eq!(
        hosts[2],
        (
            "rbn.example.invalid".into(),
            "Q1ABC".into(),
            Profile::Rbn,
            false
        )
    );
    assert_eq!(hosts[0].2, Profile::Auto);
    options.choose(&[2]);
    assert_eq!(options.labels, ["RBN CW"]);
    for (values, expected) in [
        (
            vec!["--sources", file, "--node", "example.invalid:1"],
            "--sources cannot be combined",
        ),
        (
            vec!["--sources", file, "--replay", "Cargo.toml"],
            "--sources cannot be combined",
        ),
        (
            vec!["--sources", file, "--call", "Q1ABC"],
            "--call requires --node",
        ),
        (
            vec!["--sources", file, "--sources", file],
            "unexpected argument --sources",
        ),
        (
            vec!["--sources", "/nonexistent/qg-dxc/sources.json"],
            "sources file /nonexistent/qg-dxc/sources.json",
        ),
    ] {
        let error = Options::parse(&args(&values)).err().unwrap();
        assert!(error.contains(expected), "{expected:?} not in {error:?}");
    }
    let bad = sources_file(
        "bad",
        &SAVED.replace("rbn.example.invalid:7000", "rbn.example.invalid:0"),
    );
    let error = Options::parse(&args(&["--sources", bad.to_str().unwrap()]))
        .err()
        .unwrap();
    assert!(
        error.contains("sources[2].node: port must be 1 to 65535"),
        "{error}"
    );
}
fn picker_for(count: usize, selected: &[usize]) -> picker::Picker {
    let sources: Vec<_> = (0..count)
        .map(|i| {
            format!(
                r#"{{"name":"Node {i}","node":"n{i}.example.invalid:7300","selected":{}}}"#,
                selected.contains(&i)
            )
        })
        .collect();
    let text = format!(
        r#"{{"version":1,"call":"Q1ABC","sources":[{}]}}"#,
        sources.join(",")
    );
    picker::Picker::new(&qso_graph_dxcluster_cli::saved::parse(text.as_bytes()).unwrap())
}
#[test]
fn sources_screen_chooses_starts_and_quits() {
    // QGDXC-TUI-0008
    use picker::Outcome;
    let mut p = picker_for(10, &[3, 1]);
    assert_eq!(p.key(Input::Enter), Outcome::Start(vec![1, 3]));
    for quit in [Input::Char('q'), Input::Escape, Input::CtrlC] {
        assert_eq!(picker_for(2, &[0]).key(quit), Outcome::Quit);
    }
    let mut p = picker_for(10, &[]);
    assert_eq!(p.key(Input::Enter), Outcome::Stay);
    assert_eq!(p.message, Some("Choose 1 to 8 sources"));
    p.key(Input::Up);
    assert_eq!(p.cursor, 0);
    assert_eq!(p.message, None);
    for _ in 0..9 {
        p.key(Input::Char(' '));
        p.key(Input::Down);
    }
    assert_eq!(p.cursor, 9);
    p.key(Input::Down);
    assert_eq!(p.cursor, 9);
    // Nine chosen: refused, nothing starts.
    assert_eq!(p.key(Input::Enter), Outcome::Stay);
    assert_eq!(p.message, Some("Choose 1 to 8 sources"));
    p.key(Input::Up);
    p.key(Input::Char(' '));
    assert_eq!(
        p.key(Input::Enter),
        Outcome::Start(vec![0, 1, 2, 3, 4, 5, 6, 7])
    );
    // Other keys change nothing.
    let mut p = picker_for(2, &[]);
    for other in [Input::Char('x'), Input::Tab, Input::PageDown, Input::Home] {
        assert_eq!(p.key(other), Outcome::Stay);
    }
    assert_eq!(p.chosen, [false, false]);
}
fn picker_grid(p: &picker::Picker, width: u16, height: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let a = super::theme::Appearance {
        theme: super::theme::Theme::Mono,
        depth: super::theme::Depth::detect("", ""),
    };
    present_picker(&mut terminal, p, a).unwrap();
    terminal.backend().buffer().clone()
}
#[test]
fn sources_screen_lists_every_source_and_fits_small_windows() {
    // QGDXC-TUI-0008 QGDXC-TUI-0054
    let mut p = picker_for(30, &[0]);
    let grid = picker_grid(&p, 80, 24);
    assert!(
        text(&grid, 0).contains(" QG DX Cluster  Sources"),
        "{:?}",
        text(&grid, 0)
    );
    assert!(text(&grid, 1).contains("1 of 30 chosen; up to 8 run at once"));
    assert!(text(&grid, 4).starts_with("> [x] Node 0"));
    assert!(
        text(&grid, 4).contains("n0.example.invalid:7300") && text(&grid, 4).contains("auto"),
        "{:?}",
        text(&grid, 4)
    );
    assert!(text(&grid, 5).starts_with("  [ ] Node 1"));
    assert!(text(&grid, 23).contains("Space choose  Enter start  q quit"));
    // The list scrolls to keep the cursor in view.
    for _ in 0..29 {
        p.key(Input::Down);
    }
    let grid = picker_grid(&p, 80, 24);
    assert!((4..21).any(|y| text(&grid, y).starts_with("> [ ] Node 29")));
    p.key(Input::Enter);
    p.key(Input::Enter);
    let grid = picker_grid(&picker_for(9, &[0, 1, 2, 3, 4, 5, 6, 7, 8]), 80, 24);
    assert!(!text(&grid, 21).contains("Choose"));
    let mut nine = picker_for(9, &[0, 1, 2, 3, 4, 5, 6, 7, 8]);
    nine.key(Input::Enter);
    assert!(text(&picker_grid(&nine, 80, 24), 21).contains("Choose 1 to 8 sources"));
    for (w, h) in [(79, 24), (80, 23)] {
        let grid = picker_grid(&p, w, h);
        assert!(text(&grid, 0).starts_with("Enlarge window to at least 80 columns and 24 rows."));
        assert!((1..h).all(|y| text(&grid, y).trim().is_empty()));
    }
}
#[test]
fn only_new_human_spots_are_highlighted() {
    // QGDXC-TUI-0073
    use ratatui::style::{Color, Modifier};
    for theme in [super::theme::Theme::Color, super::theme::Theme::Mono] {
        let mut m = Model::new(args(&["one"]), DEFAULT_KEEP, DEFAULT_KEEP);
        m.appearance.theme = theme;
        m.receive(0, spot("Q1ABC", 14_074_000, "CW"), Duration::from_secs(10));
        for kind in [Some("skimmer"), None, Some("beacon")] {
            let mut e = spot("Q2ABC", 14_025_000, "CW");
            match kind {
                Some(kind) => e.set("spot_kind", kind),
                None => {
                    e.fields.remove("spot_kind");
                }
            }
            m.receive(0, e, Duration::from_secs(10));
        }
        let mut terminal = Terminal::new(TestBackend::new(120, 36)).unwrap();
        present(&mut terminal, &m, Duration::from_millis(10_500)).unwrap();
        let b = terminal.backend().buffer().clone();
        let row = |call: &str| {
            (0..36)
                .filter(|&y| text(&b, y).contains(call))
                .collect::<Vec<_>>()
        };
        let lit = |y: u16| {
            let cell = &b[(60, y)];
            cell.bg != Color::Reset || cell.modifier.contains(Modifier::REVERSED)
        };
        assert_eq!(row("Q1ABC").len(), 1, "{theme:?}");
        assert!(
            lit(row("Q1ABC")[0]),
            "human spot not highlighted, {theme:?}"
        );
        let others = row("Q2ABC");
        assert_eq!(others.len(), 3, "{theme:?}");
        for y in others {
            assert!(
                !lit(y),
                "skimmer or unclassified spot highlighted, {theme:?}"
            );
        }
    }
}
