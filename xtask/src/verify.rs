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
    let before = locks.map(crate::input::read);
    let steps: [(&str, &[&str]); 14] = [
        ("fetch", &["xtask", "fetch"]),
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
        ("cargo audit", &["xtask", "audit"]),
        ("cargo vet", &["xtask", "vet"]),
        ("docs", &["xtask", "docs"]),
        ("rustdoc", &["doc", "--workspace", "--no-deps", "--locked"]),
        (
            "cargo package",
            &[
                "package",
                "--locked",
                "--allow-dirty",
                "-p",
                "qso-graph-dxcluster",
                "-p",
                "qso-graph-spaceweather",
                "-p",
                "qso-graph-http",
                "-p",
                "qso-graph-pota",
                "-p",
                "qso-graph-grid",
                "-p",
                "qso-graph-astro",
                "-p",
                "qso-graph-dxcluster-cli",
                "--config",
                "patch.crates-io.qso-graph-dxcluster.path=\"crates/qso-graph-dxcluster\"",
                "--config",
                "patch.crates-io.qso-graph-spaceweather.path=\"crates/qso-graph-spaceweather\"",
                "--config",
                "patch.crates-io.qso-graph-http.path=\"crates/qso-graph-http\"",
                "--config",
                "patch.crates-io.qso-graph-pota.path=\"crates/qso-graph-pota\"",
                "--config",
                "patch.crates-io.qso-graph-grid.path=\"crates/qso-graph-grid\"",
                "--config",
                "patch.crates-io.qso-graph-astro.path=\"crates/qso-graph-astro\"",
            ],
        ),
    ];
    let mut results = vec![];
    for (name, args) in steps {
        let start = Instant::now();
        let path = format!("target/verify/{}.log", name.replace(' ', "-"));
        let file = fs::File::create(&path).map_err(|e| e.to_string())?;
        let error = file.try_clone().map_err(|e| e.to_string())?;
        let result = Command::new("cargo")
            .args(args)
            .env(
                "CARGO_NET_OFFLINE",
                if name == "fetch" { "false" } else { "true" },
            )
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
            eprintln!("{}", crate::input::tail(&path).unwrap_or_default());
        }
        results.push((
            name.to_owned(),
            status.to_owned(),
            format!("{:.2}s", start.elapsed().as_secs_f64()),
        ));
    }
    for (name, script, extra) in [
        ("release checks", "scripts/test_release_check.py", &[][..]),
        (
            "package files",
            "scripts/package-files.py",
            &["--check"][..],
        ),
        ("workflow lint", "scripts/lint-workflows.py", &[][..]),
    ] {
        // The release workflow runs these on Linux only, and workflow lint needs Go,
        // which hosted Linux has. The Linux CI leg runs them; elsewhere they show SKIP.
        if name != "package files" && !cfg!(target_os = "linux") {
            results.push((name.to_owned(), "SKIP".into(), "Linux only".into()));
            continue;
        }
        let start = Instant::now();
        let path = format!("target/verify/{}.log", name.replace(' ', "-"));
        let file = fs::File::create(&path).map_err(|e| e.to_string())?;
        let error = file.try_clone().map_err(|e| e.to_string())?;
        let result = Command::new("uv")
            .args(["run", "--no-project", "--python", "3.11", "python", script])
            .args(extra)
            .stdout(file)
            .stderr(error)
            .stdin(Stdio::null())
            .status();
        let status = if result.is_ok_and(|s| s.success()) {
            "PASS"
        } else {
            eprintln!("{name}: see {path}");
            eprintln!("{}", crate::input::tail(&path).unwrap_or_default());
            "FAIL"
        };
        results.push((
            name.to_owned(),
            status.to_owned(),
            format!("{:.2}s", start.elapsed().as_secs_f64()),
        ));
    }
    for (name, program, args) in [
        ("commit messages", "cog", &["check"][..]),
        (
            "changelog script",
            "bash",
            &["scripts/test-changelog-release.sh"][..],
        ),
    ] {
        // Commit history is the same on every platform, so the Linux CI leg checks it once.
        if !cfg!(target_os = "linux") {
            results.push((name.to_owned(), "SKIP".into(), "Linux only".into()));
            continue;
        }
        let start = Instant::now();
        let path = format!("target/verify/{}.log", name.replace(' ', "-"));
        let file = fs::File::create(&path).map_err(|e| e.to_string())?;
        let error = file.try_clone().map_err(|e| e.to_string())?;
        let result = Command::new(program)
            .args(args)
            .stdout(file)
            .stderr(error)
            .stdin(Stdio::null())
            .status();
        let status = if result.is_ok_and(|s| s.success()) {
            "PASS"
        } else {
            eprintln!("{name}: see {path} (needs cog 7.0.0 on PATH)");
            eprintln!("{}", crate::input::tail(&path).unwrap_or_default());
            "FAIL"
        };
        results.push((
            name.to_owned(),
            status.to_owned(),
            format!("{:.2}s", start.elapsed().as_secs_f64()),
        ));
    }
    let nightly_available =
        crate::input::output(Command::new("rustup").args(["toolchain", "list"])).is_ok_and(|o| {
            o.status.success()
                && String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .any(|line| line.starts_with("nightly-2026-10-30-"))
        });
    let supported_fuzz = cfg!(target_os = "linux");
    if supported_fuzz && on_path("cargo-fuzz") && nightly_available && fuzz_version_pinned() {
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
        for (target, max_len) in [
            "values", "dx_de", "cc11", "other", "rbn", "framer", "telnet", "session", "sources",
            "prefixes",
        ]
        .into_iter()
        .map(|target| (target, "70000"))
        .chain(crate::checks::WEATHER_FUZZ.map(|(target, _, limit)| (target, limit)))
        .chain([
            ("http_retry_after", "4096"),
            ("pota_spots", "1048577"),
            ("grid_locator", "32"),
        ]) {
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
                    &format!("-max_len={max_len}"),
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
            if supported_fuzz { "FAIL" } else { "SKIP" }.into(),
            if supported_fuzz {
                "blocker: Linux requires cargo-fuzz 0.13.2 and nightly-2026-10-30 with rustfmt/clippy".into()
            } else {
                "fuzz smokes run on Linux; this platform is unsupported by this gate".into()
            },
        ));
    }
    let unchanged = locks.iter().zip(before.iter()).all(|(path, before)| {
        before.as_ref().ok() == crate::input::read(path).as_ref().ok() && before.is_ok()
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

fn fuzz_version_pinned() -> bool {
    crate::input::output(Command::new("cargo").args(["fuzz", "--version"])).is_ok_and(|o| {
        o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "cargo-fuzz 0.13.2"
    })
}
fn advisory_db() -> Result<std::path::PathBuf, String> {
    std::env::var_os("CARGO_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| std::path::PathBuf::from(p).join(".cargo")))
        .or_else(|| {
            std::env::var_os("USERPROFILE").map(|p| std::path::PathBuf::from(p).join(".cargo"))
        })
        .map(|p| p.join("advisory-db"))
        .ok_or("set CARGO_HOME for the advisory database".into())
}
fn audit_command(file: &Path, offline: bool) -> Result<Command, String> {
    let version = crate::input::output(Command::new("cargo").args(["audit", "--version"]))
        .map_err(|e| e.to_string())?;
    if !version.status.success()
        || !String::from_utf8_lossy(&version.stdout)
            .trim()
            .ends_with(" 0.22.2")
    {
        return Err("install cargo-audit 0.22.2 with --locked".into());
    }
    let mut c = Command::new("cargo");
    c.arg("audit")
        .arg("--db")
        .arg(advisory_db()?)
        .arg("--file")
        .arg(file)
        .args(["--deny", "warnings"]);
    if offline {
        c.arg("--no-fetch").env("CARGO_NET_OFFLINE", "true");
    }
    Ok(c)
}
pub fn fetch() -> Result<(), String> {
    for args in [
        vec!["fetch", "--locked"],
        vec!["fetch", "--locked", "--manifest-path", "fuzz/Cargo.toml"],
    ] {
        if !Command::new("cargo")
            .args(args)
            .env("CARGO_NET_OFFLINE", "false")
            .status()
            .map_err(|e| e.to_string())?
            .success()
        {
            return Err("locked dependency fetch failed".into());
        }
    }
    // cargo-audit fetches its database and registry metadata on this pass.
    if !audit_command(Path::new("Cargo.lock"), false)?
        .env("CARGO_NET_OFFLINE", "false")
        .status()
        .map_err(|e| e.to_string())?
        .success()
    {
        return Err("advisory database preparation failed".into());
    }
    Ok(())
}
pub fn audit() -> Result<(), String> {
    for path in ["Cargo.lock", "fuzz/Cargo.lock"] {
        if !audit_command(Path::new(path), true)?
            .status()
            .map_err(|e| e.to_string())?
            .success()
        {
            return Err(format!("audit failed: {path}"));
        }
    }
    fs::create_dir_all("target/verify").map_err(|e| e.to_string())?;
    let planted = Path::new("target/verify/vulnerable.lock");
    let mut lock = crate::input::text("Cargo.lock").map_err(|e| e.to_string())?;
    lock.push_str("\n[[package]]\nname = \"time\"\nversion = \"0.1.43\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n");
    fs::write(planted, lock).map_err(|e| e.to_string())?;
    let result = crate::input::output(audit_command(planted, true)?.arg("--json"))
        .map_err(|e| e.to_string());
    fs::remove_file(planted).map_err(|e| e.to_string())?;
    let result = result?;
    fs::write("target/verify/audit-planted.json", &result.stdout).map_err(|e| e.to_string())?;
    let report: serde_json::Value =
        serde_json::from_slice(&result.stdout).map_err(|e| e.to_string())?;
    if result.status.success()
        || !report["vulnerabilities"]["list"]
            .as_array()
            .is_some_and(|a| a.iter().any(|v| v["advisory"]["id"] == "RUSTSEC-2020-0071"))
    {
        return Err("audit failed to reject the planted vulnerable time package".into());
    }
    println!("both lockfiles audited offline; planted RUSTSEC-2020-0071 rejected and removed");
    Ok(())
}
