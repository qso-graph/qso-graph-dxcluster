// SPDX-License-Identifier: GPL-3.0-or-later
use std::process::{Command, Stdio};
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
    // QGDXC-TUI-0041 QGDXC-TUI-0051 QGDXC-TUI-0052
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
    for count in [335, 336]:
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
            while b'retained:0' not in captured and time.monotonic() < deadline:
                read_available(.05)
            assert b'retained:0' in captured, bytes(captured)
            # The terminal sends pasted text as raw key bytes under QGDXC-TUI-0041.
            # No synthetic Paste event bypasses the input decoder.
            os.write(master, b'/' + b'Q' * count + b'\x03')
            deadline = time.monotonic() + 3
            while process.poll() is None and time.monotonic() < deadline:
                read_available(.02)
            assert process.poll() == 0, ('Ctrl-C stalled or failed', count, process.poll())
            read_available(0)
            check = os.open(slave_name, os.O_RDWR | os.O_NOCTTY)
            try:
                assert termios.tcgetattr(check) == before, 'input mode was not restored'
            finally:
                os.close(check)
            assert b'\x1b[?1049l' in captured, 'alternate screen was not restored'
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
