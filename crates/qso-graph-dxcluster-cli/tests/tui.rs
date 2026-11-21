// SPDX-License-Identifier: GPL-3.0-or-later
use std::process::{Command, Stdio};
#[test]
fn retention_usage_errors_name_flag_and_range_before_terminal_checks() {
    // QGDXC-TUI-0012
    for flag in ["--keep-human", "--keep-skimmer"] {
        for value in ["9", "10001"] {
            let output = Command::new(env!("CARGO_BIN_EXE_qg-dxc"))
                .args(["tui", "--replay", "Cargo.toml", flag, value])
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            let error = String::from_utf8(output.stderr).unwrap();
            assert!(
                error.contains(flag) && error.contains("10..=10000"),
                "{error}"
            );
        }
    }
}

#[test]
fn help_describes_retention_options() {
    // QGDXC-TUI-0012
    let output = Command::new(env!("CARGO_BIN_EXE_qg-dxc"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    for value in [
        "--keep-human N",
        "--keep-skimmer N",
        "100 each",
        "10..=10000",
        "l to edit limits",
    ] {
        assert!(help.contains(value), "{help}");
    }
}

#[test]
fn nonterminal_refusal_opens_no_source_and_emits_no_escape() {
    // QGDXC-TUI-0005 QGDXC-TUI-0052
    for arguments in [
        vec!["tui", "--replay", "Cargo.toml"],
        vec!["tui", "--call", "Q1ABC", "--node", "example.invalid:7300"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_qg-dxc"))
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.contains(&27));
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("requires terminal")
        );
    }
}

#[cfg(unix)]
#[test]
fn raw_pasted_keys_keep_ctrl_c_responsive_without_enabling_bracketed_paste() {
    // QGDXC-TUI-0012 QGDXC-TUI-0041 QGDXC-TUI-0051 QGDXC-TUI-0052
    let output = Command::new("python3")
        .arg("-c")
        .arg(
            r#"
import errno, fcntl, json, os, pty, select, struct, subprocess, sys, tempfile, termios, time
with tempfile.TemporaryDirectory(prefix='qg-dxc-input-') as directory:
    recording = os.path.join(directory, 'input.ndjson')
    with open(recording, 'w') as file:
        for record in [
            {'t': 0, 'note': 'qg-dxc configuration: {"call":"Q1ABC","profile":"arcluster"}'},
            {'t': 0, 'conn': 'open'},
            {'t': 3600, 'conn': 'close', 'reason': 'cancelled'},
        ]:
            file.write(json.dumps(record) + '\n')
    for entry in [b'/' + b'Q' * 335, b'/' + b'Q' * 336, b'l' + b'9' * 336 + b'\r']:
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 80, 0, 0))
        before = termios.tcgetattr(slave)
        # macOS revokes descriptors of a controlling terminal when its session
        # leader exits, so read the restored mode through a fresh descriptor.
        slave_name = os.ttyname(slave)
        process = subprocess.Popen([sys.argv[1], 'tui', '--replay', recording],
            stdin=slave, stdout=slave, stderr=slave, start_new_session=True,
            preexec_fn=lambda: fcntl.ioctl(slave, termios.TIOCSCTTY, 0))
        captured = bytearray()
        def read_available(timeout):
            if select.select([master], [], [], timeout)[0]:
                try:
                    captured.extend(os.read(master, 65536))
                except OSError as error:
                    if error.errno != errno.EIO:
                        raise
        try:
            deadline = time.monotonic() + 5
            # Blank cells can be skipped with cursor moves, so wait for fixed text.
            while b' QG DX Cluster ' not in captured and time.monotonic() < deadline:
                read_available(.05)
            assert b' QG DX Cluster ' in captured, bytes(captured)
            # The terminal sends pasted text as raw key bytes under QGDXC-TUI-0041.
            # No synthetic Paste event bypasses the input decoder.
            os.write(master, entry + b'\x03')
            deadline = time.monotonic() + 3
            while process.poll() is None and time.monotonic() < deadline:
                read_available(.02)
            assert process.poll() == 0, ('Ctrl-C stalled or failed', entry, process.poll())
            read_available(0)
            check = os.open(slave_name, os.O_RDWR | os.O_NOCTTY)
            try:
                assert termios.tcgetattr(check) == before, 'input mode was not restored'
            finally:
                os.close(check)
            assert b'\x1b[?1049l' in captured, 'alternate screen was not restored'
            hide, show = captured.find(b'\x1b[?25l'), captured.rfind(b'\x1b[?25h')
            assert hide != -1, 'cursor was not hidden'
            assert show > hide, 'cursor was not shown again on exit'
            for sequence in [b'\x1b[?2004s', b'\x1b[?2004h', b'\x1b[?2004r']:
                assert sequence not in captured, ('bracketed paste changed', sequence)
        finally:
            if process.poll() is None:
                process.kill()
            process.wait()
            os.close(master)
            os.close(slave)
"#,
        )
        .arg(env!("CARGO_BIN_EXE_qg-dxc"))
        .output()
        .expect("python3 is required for the Unix PTY input regression test");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("qg-dxc-it-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
const SAVED: &str = r#"{"version":1,"call":"Q1ABC","sources":[
  {"name":"Home node","node":"dx.example.invalid:7300","selected":true}
]}"#;

#[test]
fn sources_file_errors_come_before_terminal_checks() {
    // QGDXC-TUI-0006 QGDXC-TUI-0007
    let dir = scratch("errors");
    let bad = dir.join("bad.json");
    std::fs::write(&bad, SAVED.replace("7300", "0")).unwrap();
    let default = dir.join("qso-graph").join("dxcluster-sources.json");
    let mut cases = vec![
        (
            vec!["tui".into(), "--sources".into(), bad.display().to_string()],
            vec![],
            format!(
                "sources file {}: sources[0].node: port must be 1 to 65535",
                bad.display()
            ),
        ),
        (
            vec!["tui".into(), "--sources".into(), dir.display().to_string()],
            vec![],
            "expected a regular file".into(),
        ),
        (
            vec!["tui".into()],
            // The default folder is %APPDATA% on Windows, else $XDG_CONFIG_HOME (QGDXC-TUI-0006).
            vec![(
                if cfg!(windows) {
                    "APPDATA"
                } else {
                    "XDG_CONFIG_HOME"
                },
                dir.display().to_string(),
            )],
            format!(
                "qg-dxc: configuration: no sources file at {}\nCreate it as described in https://github.com/qso-graph/qso-graph-dxcluster/blob/main/docs/user/sources.md\nor name nodes directly:\n  qg-dxc tui --call <CALL> --node <host>:<port>[/<profile>] [--node ...]\n",
                default.display()
            ),
        ),
    ];
    if cfg!(unix) {
        cases.push((vec!["tui".into()], vec![], "HOME must be set".into()));
    }
    for (arguments, env, expected) in cases {
        let mut command = Command::new(env!("CARGO_BIN_EXE_qg-dxc"));
        command.args(&arguments).stdin(Stdio::null());
        if cfg!(unix) {
            command.env_remove("XDG_CONFIG_HOME").env_remove("HOME");
        }
        for (key, value) in env {
            command.env(key, value);
        }
        let output = command.output().unwrap();
        assert_eq!(output.status.code(), Some(1), "{arguments:?}");
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.contains(&27));
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains(&expected), "{expected:?} not in {error:?}");
    }
    let help = Command::new(env!("CARGO_BIN_EXE_qg-dxc"))
        .arg("--help")
        .output()
        .unwrap();
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(
        help.contains("qg-dxc tui [--sources <file>]") && help.contains("dxcluster-sources.json"),
        "{help}"
    );
    // A valid file reaches the terminal check, which still refuses before any source opens.
    let good = dir.join("good.json");
    std::fs::write(&good, SAVED).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_qg-dxc"))
        .args(["tui", "--sources", good.to_str().unwrap()])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("requires terminal")
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn sources_screen_quits_without_opening_or_writing() {
    // QGDXC-TUI-0008 QGDXC-TUI-0009 QGDXC-TUI-0052
    let dir = scratch("screen");
    let file = dir.join("sources.json");
    std::fs::write(&file, SAVED).unwrap();
    let output = Command::new("python3")
        .arg("-c")
        .arg(
            r#"
import errno, fcntl, os, pty, select, struct, subprocess, sys, termios, time
for key, code in [(b'q', 0), (b'\x1b', 0), (b'\x03', 0)]:
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 80, 0, 0))
    before = termios.tcgetattr(slave)
    slave_name = os.ttyname(slave)
    process = subprocess.Popen([sys.argv[1], 'tui', '--sources', sys.argv[2]],
        stdin=slave, stdout=slave, stderr=slave, start_new_session=True,
        preexec_fn=lambda: fcntl.ioctl(slave, termios.TIOCSCTTY, 0))
    captured = bytearray()
    def read_available(timeout):
        if select.select([master], [], [], timeout)[0]:
            try:
                captured.extend(os.read(master, 65536))
            except OSError as error:
                if error.errno != errno.EIO:
                    raise
    try:
        deadline = time.monotonic() + 5
        while b'Home node' not in captured and time.monotonic() < deadline:
            read_available(.05)
        assert b'Home node' in captured, bytes(captured)
        assert b'dx.example.invalid:7300' in captured, bytes(captured)
        # Escape alone must be read as Escape, not the start of a sequence.
        os.write(master, key)
        deadline = time.monotonic() + 3
        while process.poll() is None and time.monotonic() < deadline:
            read_available(.02)
        assert process.poll() == code, ('exit', key, process.poll())
        read_available(0)
        check = os.open(slave_name, os.O_RDWR | os.O_NOCTTY)
        try:
            assert termios.tcgetattr(check) == before, 'input mode was not restored'
        finally:
            os.close(check)
        assert b'\x1b[?1049l' in captured, 'alternate screen was not restored'
        assert b'QG DX Cluster' in captured and b'newest first' not in captured, 'spot list opened'
    finally:
        if process.poll() is None:
            process.kill()
        process.wait()
        os.close(master)
        os.close(slave)
"#,
        )
        .arg(env!("CARGO_BIN_EXE_qg-dxc"))
        .arg(&file)
        .output()
        .expect("python3 is required for the Unix PTY Sources screen test");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), SAVED);
    let listing: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(listing, ["sources.json"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn first_run_hint_lines_are_only_the_programs_own() {
    // QGDXC-TUI-0006 QGDXC-TUI-0013: a newline or escape in the configured folder is
    // escaped inside the path line; it never starts a line of its own.
    let output = Command::new(env!("CARGO_BIN_EXE_qg-dxc"))
        .arg("tui")
        .env(
            "XDG_CONFIG_HOME",
            "/nonexistent/qg-dxc\nforged line\u{1b}[2J",
        )
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.contains(&27));
    let error = String::from_utf8(output.stderr).unwrap();
    let lines: Vec<&str> = error.lines().collect();
    assert_eq!(lines.len(), 4, "{error}");
    assert!(lines[0].starts_with(
        "qg-dxc: configuration: no sources file at /nonexistent/qg-dxc\\u{000a}forged line"
    ));
    assert!(lines[1].starts_with("Create it as described in https://"));
    assert_eq!(
        lines[3],
        "  qg-dxc tui --call <CALL> --node <host>:<port>[/<profile>] [--node ...]"
    );
}
