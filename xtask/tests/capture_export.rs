// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
use qso_graph_dxcluster::model::{base64, unbase64};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

fn export(root: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["gen-vectors", "--captures", "captures"])
        .current_dir(root)
        .output()
        .unwrap()
}

#[test]
fn capture_export_redacts_boundaries_and_validates_before_publication() {
    // QGDXC-SEC-0005 QGDXC-SEC-0006
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../target/capture-export-{}", std::process::id()));
    fs::create_dir_all(root.join("captures")).unwrap();
    fs::create_dir_all(root.join("vectors")).unwrap();
    fs::create_dir_all(root.join("spec")).unwrap();
    fs::write(root.join("spec/evidence.md"), "<!-- SPDX-License-Identifier: GPL-3.0-or-later -->\nSynthetic capture `dxspider-synthetic.ndjson`.\n").unwrap();
    let addresses = [
        [192, 0, 2, 1].map(|n| n.to_string()).join("."),
        [10, 20, 30, 40].map(|n| n.to_string()).join("."),
        ["2001", "db8", "", "1"].join(":"),
        ["fd00", "", "1"].join(":"),
    ];
    let mut source = String::new();
    for address in &addresses {
        for (before, after) in [
            ("", "."),
            ("peer:", ""),
            ("peer:", "Z"),
            ("x", "."),
            ("('", ".'),"),
        ] {
            let line = format!("DX de Q1ABC: 14025 Q2ABC peer {before}{address}{after} 1832Z\n");
            source.push_str(&format!(
                "{}\n",
                json!({"dir":"in", "b64":base64(line.as_bytes())})
            ));
        }
    }
    fs::write(root.join("captures/dxspider-synthetic.ndjson"), source).unwrap();
    let result = export(&root);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let output = fs::read_to_string(root.join("vectors/events.ndjson")).unwrap();
    let exported: Vec<Value> = output
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .filter(|v: &Value| v["provenance"]["capture"] == "dxspider-synthetic.ndjson")
        .collect();
    assert_eq!(exported.len(), addresses.len() * 5);
    for v in exported {
        let input = unbase64(v["steps"][0]["b64"].as_str().unwrap()).unwrap();
        let event = &v["expected_events"][0]["event"];
        let raw = unbase64(event["raw_line"].as_str().unwrap()).unwrap();
        assert_eq!(raw, input.strip_suffix(b"\n").unwrap());
        assert!(
            event["fields"]["comment"]
                .as_str()
                .unwrap()
                .contains("<ip>")
        );
        for address in &addresses {
            assert!(!String::from_utf8_lossy(&input).contains(address));
            assert!(!String::from_utf8_lossy(&raw).contains(address));
            assert!(!event.to_string().contains(address));
        }
    }
    // Unregistered provenance must fail the same gate as public fixtures.
    fs::rename(
        root.join("captures/dxspider-synthetic.ndjson"),
        root.join("captures/dxspider-unregistered.ndjson"),
    )
    .unwrap();
    let paths = ["vectors/events.ndjson", "vectors/sessions.ndjson"];
    let before = paths.map(|p| fs::read(root.join(p)).unwrap());
    for exists in [true, false] {
        let result = export(&root);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("unsanitized generated file"));
        for (path, bytes) in paths.iter().zip(&before) {
            if exists {
                assert_eq!(fs::read(root.join(path)).unwrap(), *bytes);
                fs::remove_file(root.join(path)).unwrap();
            } else {
                assert!(!root.join(path).exists());
            }
        }
    }
    fs::remove_dir_all(root).unwrap();
}
