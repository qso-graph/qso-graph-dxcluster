// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
use qso_graph_dxcluster::model::base64;
use serde_json::json;
use std::{fs, process::Command};

#[test]
fn replay_redacts_notes_and_events_in_both_output_formats() {
    // QGDXC-SEC-0003 QGDXC-SEC-0005 QGDXC-REC-0002
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/rendering-{}.ndjson",
        std::process::id()
    ));
    let addresses = "third-party spotter 192.0.2.1; relay [2001:db8::1]";
    let controls = "\u{0000}\u{0007}\u{001b}\r\n\u{007f}\u{0085}\u{009b}";
    let note = format!("{addresses}{controls}");
    let wire = format!("{addresses}{controls}\n");
    let mut slots = vec![""; 25];
    slots[0] = "CC11";
    slots[22] = "+12.3456/-45.6789";
    slots[23] = "192.0.2.1";
    let cc11 = slots.join("^");
    let records = [
        json!({"t":0,"note":note}),
        json!({"t":0,"note":cc11}),
        json!({"t":0,"note":format!("observer: {cc11}")}),
        json!({"t":0,"conn":"open"}),
        json!({"t":0,"dir":"in","b64":base64(wire.as_bytes())}),
        json!({"t":0,"conn":"close","reason":"eof"}),
    ];
    fs::write(
        &path,
        records.iter().map(|v| format!("{v}\n")).collect::<String>(),
    )
    .unwrap();
    for json in [false, true] {
        for retain in [false, true] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_qg-dxc"));
            command.arg("replay").arg(&path);
            if json {
                command.arg("--json");
            }
            if retain {
                command.arg("--retain-third-party-data");
            }
            let output = command.output().unwrap();
            let stderr = String::from_utf8(output.stderr).unwrap();
            let stdout = String::from_utf8(output.stdout).unwrap();
            assert!(output.status.success(), "{stderr}");
            for address in ["192.0.2.1", "2001:db8::1", "+12.3456/-45.6789"] {
                assert_eq!(stderr.contains(address), retain, "{stderr}");
            }
            assert!(stderr.contains("note 1: third-party spotter"), "{stderr}");
            assert_eq!(stderr.contains("<ip>"), !retain, "{stderr}");
            for control in controls.chars() {
                assert!(stderr.contains(&format!("\\u{{{:04x}}}", control as u32)));
            }
            for stream in [&stderr, &stdout] {
                assert!(!stream.chars().any(|c| c.is_control() && c != '\n'));
            }
            if json {
                let mut evidence = String::new();
                for line in stdout.lines() {
                    let event: serde_json::Value = serde_json::from_str(line).unwrap();
                    let raw =
                        qso_graph_dxcluster::model::unbase64(event["raw_line"].as_str().unwrap())
                            .unwrap();
                    evidence.push_str(&String::from_utf8(raw).unwrap());
                }
                for address in ["192.0.2.1", "2001:db8::1"] {
                    assert_eq!(evidence.contains(address), retain, "{evidence}");
                }
            } else {
                for address in ["192.0.2.1", "2001:db8::1"] {
                    assert_eq!(stdout.contains(address), retain, "{stdout}");
                }
            }
        }
    }
    fs::remove_file(path).unwrap();
}
