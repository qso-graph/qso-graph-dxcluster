// SPDX-License-Identifier: GPL-3.0-or-later
//! One table, retained logs, and nonzero exit on every failed row. Adapted from
//! the project's shared workspace verification conventions.
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    time::Instant,
};
fn on_path(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| {
        std::env::split_paths(&p)
            .any(|p| p.join(name).is_file() || p.join(format!("{name}.exe")).is_file())
    })
}
pub fn verify() -> Result<(), String> {
    fs::create_dir_all("target/verify").map_err(|e| e.to_string())?;
    let locks = ["Cargo.lock", "fuzz/Cargo.lock"];
    let before = locks.map(fs::read);
    let steps: [(&str, &[&str]); 11] = [
        ("fmt", &["fmt", "--all", "--check"]),
        (
            "clippy",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--locked",
                "--",
                "-D",
                "warnings",
            ],
        ),
        (
            "tests",
            &["test", "--workspace", "--all-features", "--locked"],
        ),
        ("vectors", &["xtask", "vectors"]),
        ("requirement coverage", &["xtask", "coverage"]),
        ("sanitization", &["xtask", "sanitization"]),
        ("policy", &["xtask", "policy"]),
        (
            "cargo deny",
            &[
                "deny",
                "--locked",
                "--offline",
                "check",
                "bans",
                "licenses",
                "sources",
            ],
        ),
        ("cargo vet", &["xtask", "vet"]),
        ("docs", &["xtask", "docs"]),
        ("rustdoc", &["doc", "--workspace", "--no-deps", "--locked"]),
    ];
    let mut results = vec![];
    for (name, args) in steps {
        let start = Instant::now();
        let path = format!("target/verify/{}.log", name.replace(' ', "-"));
        let file = fs::File::create(&path).map_err(|e| e.to_string())?;
        let error = file.try_clone().map_err(|e| e.to_string())?;
        let result = Command::new("cargo")
            .args(args)
            .env("CARGO_NET_OFFLINE", "true")
            .env("RUSTDOCFLAGS", "-D warnings")
            .stdin(Stdio::null())
            .stdout(file)
            .stderr(error)
            .status();
        let status = if result.is_ok_and(|s| s.success()) {
            "PASS"
        } else {
            "FAIL"
        };
        if status == "FAIL" {
            eprintln!("{name}: see {path}");
            let log = fs::read_to_string(&path).unwrap_or_default();
            let lines: Vec<_> = log.lines().collect();
            eprintln!("{}", lines[lines.len().saturating_sub(20)..].join("\n"));
        }
        results.push((
            name.to_owned(),
            status.to_owned(),
            format!("{:.2}s", start.elapsed().as_secs_f64()),
        ));
    }
    if on_path("cargo-fuzz") {
        for (name, args) in [
            (
                "fuzz fmt",
                vec![
                    "+nightly-2026-10-30",
                    "fmt",
                    "--manifest-path",
                    "fuzz/Cargo.toml",
                    "--check",
                ],
            ),
            (
                "fuzz clippy",
                vec![
                    "+nightly-2026-10-30",
                    "clippy",
                    "--manifest-path",
                    "fuzz/Cargo.toml",
                    "--all-targets",
                    "--locked",
                    "--",
                    "-D",
                    "warnings",
                ],
            ),
            ("fuzz seed", vec!["xtask", "seed-fuzz"]),
        ] {
            let file = fs::File::create(format!("target/verify/{}.log", name.replace(' ', "-")))
                .map_err(|e| e.to_string())?;
            let error = file.try_clone().map_err(|e| e.to_string())?;
            let result = Command::new("cargo")
                .args(args)
                .env("CARGO_NET_OFFLINE", "true")
                .stdout(file)
                .stderr(error)
                .status();
            results.push((
                name.to_owned(),
                if result.is_ok_and(|s| s.success()) {
                    "PASS"
                } else {
                    "FAIL"
                }
                .into(),
                "isolated fuzz workspace".into(),
            ));
        }
        for target in [
            "values", "dx_de", "cc11", "other", "rbn", "framer", "telnet", "session",
        ] {
            let name = format!("fuzz {target}");
            let path = format!("target/verify/fuzz-{target}.log");
            let file = fs::File::create(path).map_err(|e| e.to_string())?;
            let error = file.try_clone().map_err(|e| e.to_string())?;
            let result = Command::new("cargo")
                .args([
                    "+nightly-2026-10-30",
                    "fuzz",
                    "run",
                    target,
                    "--",
                    "-max_total_time=1",
                    "-max_len=70000",
                    "-timeout=10",
                    "-rss_limit_mb=2048",
                ])
                .env_remove("RUSTUP_TOOLCHAIN")
                .env_remove("CARGO")
                .env_remove("RUSTC")
                .env("CARGO_NET_OFFLINE", "true")
                .current_dir("fuzz")
                .stdout(file)
                .stderr(error)
                .status();
            results.push((
                name,
                if result.is_ok_and(|s| s.success()) {
                    "PASS"
                } else {
                    "FAIL"
                }
                .into(),
                "1-second smoke".into(),
            ));
        }
    } else {
        results.push((
            "fuzz smoke".into(),
            "SKIP".into(),
            "blocker: cargo-fuzz absent from PATH; install cargo-fuzz 0.13.2".into(),
        ));
    }
    let unchanged = locks.iter().zip(before.iter()).all(|(path, before)| {
        before.as_ref().ok() == fs::read(path).as_ref().ok() && before.is_ok()
    });
    results.push((
        "lockfiles unchanged".into(),
        if unchanged { "PASS" } else { "FAIL" }.into(),
        locks.join(", "),
    ));
    let mut table = String::from("| Check | Result | Detail |\n|---|---|---|\n");
    for (name, status, detail) in &results {
        table.push_str(&format!("| {name} | {status} | {detail} |\n"));
    }
    print!("{table}");
    fs::write(Path::new("target/verify/table.md"), table).map_err(|e| e.to_string())?;
    if results.iter().any(|(_, s, _)| s == "FAIL") {
        Err("verification failed".into())
    } else {
        Ok(())
    }
}
