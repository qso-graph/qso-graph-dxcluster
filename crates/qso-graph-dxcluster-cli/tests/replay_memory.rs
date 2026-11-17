// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
#[cfg(target_os = "linux")]
#[test]
fn replay_memory_is_independent_of_recording_length() {
    // QGDXC-SEC-0001 QGDXC-REC-0004
    use std::{
        fs,
        io::Write,
        process::{Command, Stdio},
    };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/replay-memory-{}.ndjson",
        std::process::id()
    ));
    let mut file = std::io::BufWriter::new(fs::File::create(&path).unwrap());
    writeln!(file, "{{\"t\":0,\"note\":\"qg-dxc configuration: {{\\\"call\\\":\\\"Q1ABC\\\",\\\"profile\\\":\\\"rbn\\\"}}\"}}").unwrap();
    for _ in 0..500_000 {
        file.write_all(
            b"{\"t\":0,\"conn\":\"open\"}\n{\"t\":0,\"conn\":\"close\",\"reason\":\"cancelled\"}\n",
        )
        .unwrap();
    }
    file.flush().unwrap();
    let output = Command::new("bash")
        .args([
            "-c",
            "ulimit -c 0; ulimit -v 65536; exec \"$1\" replay \"$2\"",
            "replay-memory",
        ])
        .arg(env!("CARGO_BIN_EXE_qg-dxc"))
        .arg(&path)
        .stdout(Stdio::null())
        .env("MALLOC_ARENA_MAX", "1") // see test_support.rs in the CLI crate
        .output()
        .unwrap();
    fs::remove_file(path).unwrap();
    assert!(
        output.status.success(),
        "64 MiB replay ceiling: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
