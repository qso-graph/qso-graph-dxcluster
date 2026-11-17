// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
#[cfg(target_os = "linux")]
#[test]
fn capture_provenance_ignores_notes_under_a_memory_ceiling() {
    // QGDXC-SEC-0001 QGDXC-VEC-0001
    use std::{
        fs,
        io::Write,
        process::{Command, Stdio},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../target/capture-memory-{}", std::process::id()));
    let captures = root.join("captures");
    fs::create_dir_all(&captures).unwrap();
    fs::create_dir_all(root.join("vectors")).unwrap();
    fs::create_dir_all(root.join("spec")).unwrap();
    fs::write(root.join("spec/evidence.md"), "<!-- SPDX-License-Identifier: GPL-3.0-or-later -->\nSynthetic capture `dxspider-synthetic.ndjson`.\n").unwrap();
    let notes = 2_000_000;
    let mut file = std::io::BufWriter::new(
        fs::File::create(captures.join("dxspider-synthetic.ndjson")).unwrap(),
    );
    let record = |b: &[u8]| {
        use qso_graph_dxcluster::model::base64;
        format!("{}\n", serde_json::json!({"dir":"in", "b64":base64(b)}))
    };
    file.write_all(record(b"DX de Q1ABC: 14025 Q2ABC ").as_bytes())
        .unwrap();
    for _ in 0..notes {
        file.write_all(b"{\"note\":\"x\"}\n").unwrap();
    }
    file.write_all(record(b"CQ 1832Z\n").as_bytes()).unwrap();
    file.flush().unwrap();
    let output = Command::new("bash")
        .args([
            "-c",
            "ulimit -c 0; ulimit -v 65536; exec \"$1\" gen-vectors --captures \"$2\"",
            "capture-memory",
        ])
        .arg(env!("CARGO_BIN_EXE_xtask"))
        .arg(&captures)
        .current_dir(&root)
        .stdout(Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "64 MiB capture ceiling: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let vectors = fs::read_to_string(root.join("vectors/events.ndjson")).unwrap();
    let captured: Vec<serde_json::Value> = vectors
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .filter(|v| v["provenance"]["capture"] == "dxspider-synthetic.ndjson")
        .collect();
    assert_eq!(captured.len(), 1);
    assert_eq!(
        captured[0]["provenance"]["records"],
        serde_json::json!([1, notes + 2])
    );
    fs::remove_dir_all(root).unwrap();
}
