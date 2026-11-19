// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
//! `qg-dxc --version` must name the version being built, never a stale literal.

#[test]
fn version_matches_the_package_version() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_qg-dxc"))
        .arg("--version")
        .output()
        .expect("qg-dxc runs");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        format!("qg-dxc {}", env!("CARGO_PKG_VERSION"))
    );
}
