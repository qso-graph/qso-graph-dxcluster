// SPDX-License-Identifier: GPL-3.0-or-later
use std::process::{Command, Stdio};
#[test]
fn nonterminal_refusal_opens_no_source_and_emits_no_escape() {
    // QGDXC-TUI-0005 QGDXC-TUI-0052
    for arguments in [
        vec!["tui", "--replay", "missing-file"],
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
