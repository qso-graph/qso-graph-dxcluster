// SPDX-License-Identifier: GPL-3.0-or-later
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};
pub fn files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = vec![];
    if !root.exists() {
        return Ok(out);
    }
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let e = entry.map_err(|e| e.to_string())?;
        if e.file_type().map_err(|e| e.to_string())?.is_symlink() {
            return Err(format!("symlink not allowed: {}", e.path().display()));
        }
        if e.path().is_dir() {
            out.extend(files(&e.path())?);
        } else {
            out.push(e.path());
        }
    }
    Ok(out)
}
fn identifiers(s: &str) -> BTreeSet<String> {
    s.split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .filter(|s| {
            s.starts_with("QGDXC-")
                && s.rsplit('-')
                    .next()
                    .is_some_and(|n| n.len() == 4 && n.bytes().all(|b| b.is_ascii_digit()))
        })
        .map(str::to_owned)
        .collect()
}
/// Test identifiers are taken only from test attributes and function names.
fn named_tests(source: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut pending = false;
    for line in source.lines().map(str::trim) {
        if line == "#[test]" {
            pending = true;
        } else if pending && line.starts_with("fn ") {
            if let Some((name, _)) = line[3..].split_once('(') {
                names.insert(name.to_owned());
            }
            pending = false;
        } else if !line.is_empty() && !line.starts_with("//") && !line.starts_with("#[") {
            pending = false;
        }
    }
    names
}
fn coverage_at(root: &Path, compiled: Option<&BTreeSet<String>>) -> Result<(), String> {
    let mut defined = std::collections::BTreeMap::new();
    for file in files(&root.join("spec"))? {
        if file.extension().is_some_and(|e| e == "md") {
            for line in fs::read_to_string(file)
                .map_err(|e| e.to_string())?
                .lines()
                .filter(|l| l.starts_with("| `QGDXC-"))
            {
                let id = line.split('`').nth(1).ok_or("requirement table")?;
                let method = line
                    .split('|')
                    .rev()
                    .nth(1)
                    .ok_or("verification method")?
                    .trim();
                if defined.insert(id.to_owned(), method.to_owned()).is_some() {
                    return Err(format!("duplicate requirement definition: {id}"));
                }
            }
        }
    }
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(root.join("docs/coverage.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mapped = manifest["requirements"]
        .as_object()
        .ok_or("coverage requirement map")?;
    let mut vectors = BTreeSet::new();
    for file in files(&root.join("vectors"))? {
        if file.extension().is_some_and(|e| e == "ndjson") {
            for line in fs::read_to_string(file).map_err(|e| e.to_string())?.lines() {
                let v: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
                let id = v["id"].as_str().ok_or("vector id")?;
                if v["spec"] != "0.1.0"
                    || !v["steps"].is_array()
                    || !v["expected_events"].is_array()
                    || !vectors.insert(id.to_owned())
                {
                    return Err(format!("invalid/duplicate vector: {id}"));
                }
            }
        }
    }
    let notes = fs::read_to_string(root.join("docs/CONFORMANCE.md")).map_err(|e| e.to_string())?;
    let inspected: BTreeSet<_> = notes
        .lines()
        .filter(|l| l.starts_with("Inspection:"))
        .flat_map(identifiers)
        .collect();
    let mut tested = 0;
    for (id, method) in &defined {
        let evidence = mapped
            .get(id)
            .ok_or_else(|| format!("missing coverage mapping: {id}"))?;
        let tests = evidence["tests"].as_array().ok_or("named test array")?;
        let cases = evidence["vectors"].as_array().ok_or("vector id array")?;
        for test in tests {
            let file = test["file"].as_str().ok_or("test file")?;
            let name = test["name"].as_str().ok_or("test name")?;
            let source = fs::read_to_string(root.join(file)).map_err(|e| format!("{file}: {e}"))?;
            if !named_tests(&source).contains(name)
                || compiled.is_some_and(|names| !names.contains(name))
            {
                return Err(format!("missing named test: {file}#{name}"));
            }
        }
        for case in cases {
            if !case.as_str().is_some_and(|id| vectors.contains(id)) {
                return Err(format!("missing vector for {id}: {case}"));
            }
        }
        let has_test = !tests.is_empty() || !cases.is_empty();
        if method.contains("Test") && !has_test {
            return Err(format!("Test requirement has no named test/vector: {id}"));
        }
        if !has_test && !inspected.contains(id) {
            return Err(format!("missing Inspection note: {id}"));
        }
        if has_test {
            tested += 1;
        }
    }
    if mapped.keys().any(|id| !defined.contains_key(id)) {
        return Err("coverage maps undefined requirements".into());
    }
    // Categories pin their cases independently of incidental requirement mentions.
    for (category, cases) in manifest["scenarios"].as_object().ok_or("scenario map")? {
        let cases = cases
            .as_array()
            .filter(|a| !a.is_empty())
            .ok_or("empty scenario category")?;
        for id in cases {
            if !id.as_str().is_some_and(|id| vectors.contains(id)) {
                return Err(format!("missing scenario {category}: {id}"));
            }
        }
    }
    if vectors.is_empty() {
        return Err("missing vector suite".into());
    }
    println!(
        "{} / {} requirements covered ({} named test/vector, {} inspection-only)",
        defined.len(),
        defined.len(),
        tested,
        defined.len() - tested
    );
    Ok(())
}
pub fn coverage() -> Result<(), String> {
    // Compiler discovery excludes commented-out, inactive, and ordinary functions.
    let output = std::process::Command::new("cargo")
        .args([
            "test",
            "--workspace",
            "--all-features",
            "--locked",
            "--",
            "--list",
        ])
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("cannot discover compiled tests".into());
    }
    let names = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .filter_map(|name| name.rsplit(':').next())
        .map(str::to_owned)
        .collect();
    coverage_at(Path::new("."), Some(&names))
}
fn words(s: &str) -> Vec<&str> {
    s.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .filter(|s| !s.is_empty())
        .collect()
}
pub fn source_violation(s: &str, library: bool) -> bool {
    let w = words(s);
    let forbidden = [
        "unsafe",
        "include",
        "include_str",
        "include_bytes",
        "macro_rules",
        "global_asm",
        "asm",
        "extern",
    ];
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let clock_path = compact.contains("::time::")
        || compact.contains(".elapsed(")
        || compact
            .split(';')
            .any(|part| part.contains("use") && part.contains("time") && part.contains("std"));
    (library && clock_path)
        || w.iter().any(|w| forbidden.contains(w))
        || library
            && w.iter().any(|w| {
                [
                    "io",
                    "print",
                    "println",
                    "eprint",
                    "eprintln",
                    "dbg",
                    "log",
                    "tracing",
                    "net",
                    "fs",
                    "process",
                    "SystemTime",
                    "Instant",
                    "UNIX_EPOCH",
                    "env",
                    "thread",
                    "os",
                    "ffi",
                ]
                .contains(w)
            })
}
pub fn policy() -> Result<(), String> {
    // Planted violations include inactive code, grouped imports, aliases and redirection.
    for planted in [
        "use std::net::TcpStream;",
        "use std::{fs};",
        "use std as alias; alias::process::Command;",
        "#[cfg(any())] fn f(){let _=std::env::vars();}",
        "use std::time::Instant;",
        "let _ = std::time::UNIX_EPOCH.elapsed();",
        "use std::{time as clocks}; clocks::UNIX_EPOCH.elapsed();",
        "use std::time; time::SystemTime::now();",
        "std::io::stdout();",
        r#"println!("text");"#,
        "unsafe {}",
        "include!(\"extra.rs\");",
        "#[path=\"extra.rs\"] mod hidden;",
    ] {
        if !source_violation(planted, true) && !planted.contains("#[path") {
            return Err("policy planted-case detector failed".into());
        }
    }
    for (root, library) in [
        ("crates/qso-graph-dxcluster", true),
        ("crates/qso-graph-dxcluster-cli", false),
    ] {
        let manifest =
            fs::read_to_string(format!("{root}/Cargo.toml")).map_err(|e| e.to_string())?;
        if library && manifest.contains("[dependencies]") {
            return Err("library runtime dependencies require review".into());
        }
        for file in files(Path::new(root))? {
            if file.file_name().is_some_and(|n| n == "build.rs")
                || file
                    .extension()
                    .is_some_and(|e| ["c", "cc", "cpp", "h", "o", "a"].iter().any(|s| e == *s))
            {
                return Err(format!("native source/build script: {}", file.display()));
            }
            if file.extension().is_some_and(|e| e == "rs") {
                let s = fs::read_to_string(&file).map_err(|e| e.to_string())?;
                if source_violation(&s, library) || s.contains("#[path") {
                    return Err(format!("source policy: {}", file.display()));
                }
                if file
                    .file_name()
                    .is_some_and(|n| n == "lib.rs" || n == "main.rs")
                    && !s.contains("#![forbid(unsafe_code)]")
                {
                    return Err("crate root must forbid unsafe code".into());
                }
            }
        }
    }
    if fs::read_to_string("rust-toolchain.toml")
        .map_err(|e| e.to_string())?
        .contains("channel = \"1.99.1\"")
    {
        println!("source I/O and unsafe bans; planted alias/inactive/include cases rejected");
        Ok(())
    } else {
        Err("toolchain pin".into())
    }
}
fn likely_call(s: &str) -> bool {
    let s = s.trim_matches(['/', '-', '#']);
    if s.starts_with(['Q', 'q']) || s.len() < 3 || s.len() > 64 {
        return false;
    }
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && b[i].is_ascii_alphabetic() {
        i += 1;
    }
    if i == 0 || i > 3 {
        return false;
    }
    let start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    i > start
        && i < b.len()
        && b[i..].iter().all(u8::is_ascii_alphanumeric)
        && b[i..].iter().any(u8::is_ascii_alphabetic)
}
fn text_sanitized(s: &str) -> bool {
    for t in s.split(|c: char| {
        !c.is_ascii_alphanumeric()
            && c != ':'
            && c != '.'
            && c != '_'
            && c != '-'
            && c != '/'
            && c != '#'
            && c != '@'
    }) {
        if t.split_once('@')
            .is_some_and(|(a, b)| !a.is_empty() && b.contains('.'))
            || [".local", ".lan", ".internal"].iter().any(|suffix| {
                t.strip_suffix(suffix)
                    .is_some_and(|prefix| !prefix.is_empty())
            })
        {
            return false;
        }
        // References to capture basenames are provenance, never endpoint examples.
        if matches!(t, "set/ve7cc" | "ve7cc") {
            continue;
        }
        if (t.len() == 40 || t.len() == 64) && t.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        for sub in t.split(['/', '-', '_', ':', '.']) {
            if (sub.len() == 40 || sub.len() == 64) && sub.bytes().all(|b| b.is_ascii_hexdigit()) {
                continue;
            }
            if likely_call(sub) {
                return false;
            }
        }
        // Check the complete IPv6 token and endpoint host components.
        for literal in std::iter::once(t).chain(t.split(['/', ':'])) {
            if [".local", ".lan", ".internal"].iter().any(|suffix| {
                literal
                    .strip_suffix(suffix)
                    .is_some_and(|prefix| !prefix.is_empty())
            }) {
                return false;
            }
            if let Ok(ip) = literal.parse::<std::net::IpAddr>() {
                let doc = match ip {
                    std::net::IpAddr::V4(ip) => {
                        let o = ip.octets();
                        o[..3] == [192, 0, 2] || o[..3] == [198, 51, 100] || o[..3] == [203, 0, 113]
                    }
                    std::net::IpAddr::V6(ip) => ip.segments()[..2] == [0x2001, 0xdb8],
                };
                if !doc {
                    return false;
                }
            }
        }
    }
    true
}
fn evidence_captures() -> &'static BTreeSet<String> {
    static CAPTURES: std::sync::OnceLock<BTreeSet<String>> = std::sync::OnceLock::new();
    CAPTURES.get_or_init(|| {
        files(Path::new("spec"))
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "md"))
            .filter_map(|p| fs::read_to_string(p).ok())
            .flat_map(|s| {
                s.split('`')
                    .filter(|s| s.ends_with(".ndjson") && !s.contains(['/', ' ', '\n']))
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect()
    })
}
fn vector_sanitized(v: &Value) -> bool {
    match v {
        Value::Object(o) => o.iter().all(|(k, v)| {
            if k == "capture" {
                return v
                    .as_str()
                    .is_some_and(|s| s == "synthetic" || evidence_captures().contains(s));
            }
            if k == "b64" || k == "raw_line" {
                return v.as_str().is_some_and(|s| {
                    crate::oracle::unb64(s)
                        .is_ok_and(|b| text_sanitized(&String::from_utf8_lossy(&b)))
                });
            }
            vector_sanitized(v)
        }),
        Value::Array(a) => a.iter().all(vector_sanitized),
        Value::String(s) => text_sanitized(s),
        _ => true,
    }
}
fn public_paths() -> Result<Vec<PathBuf>, String> {
    let output = std::process::Command::new("git")
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("cannot enumerate public files".into());
    }
    String::from_utf8(output.stdout)
        .map_err(|e| e.to_string())
        .map(|s| {
            s.split('\0')
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .collect()
        })
}
fn unescape_source(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 'r' | 't') => out.push(' '),
            Some('x') => {
                let hex: String = chars.by_ref().take(2).collect();
                if let Ok(b) = u8::from_str_radix(&hex, 16) {
                    out.push(char::from(b));
                }
            }
            Some('u') if chars.peek() == Some(&'{') => {
                chars.next();
                let hex: String = chars.by_ref().take_while(|c| *c != '}').collect();
                if let Some(c) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(c);
                }
            }
            Some(c) => out.push(c),
            None => out.push('\\'),
        }
    }
    out
}
fn inline_fixture_sanitized(source: &str) -> bool {
    let source = unescape_source(source);
    for key in ["b64", "raw_line"] {
        let marker = format!("\"{key}\"");
        for rest in source.split(&marker).skip(1) {
            if let Some(rest) = rest.trim_start().strip_prefix(':')
                && let Some(rest) = rest.trim_start().strip_prefix('"')
                && let Some((encoded, _)) = rest.split_once('"')
                && let Ok(bytes) = crate::oracle::unb64(encoded)
                && !text_sanitized(&String::from_utf8_lossy(&bytes))
            {
                return false;
            }
        }
    }
    true
}
fn public_file_sanitized(file: &Path, source: &str) -> bool {
    if file
        .extension()
        .is_some_and(|e| e == "ndjson" || e == "json")
    {
        return source
            .lines()
            .filter(|l| !l.is_empty())
            .all(|l| serde_json::from_str::<Value>(l).is_ok_and(|v| vector_sanitized(&v)))
            || serde_json::from_str::<Value>(source).is_ok_and(|v| vector_sanitized(&v));
    }
    if !inline_fixture_sanitized(source) {
        return false;
    }
    source.lines().all(|line| {
        // Upstream audit authors are metadata, never protocol examples.
        if file == Path::new("supply-chain/imports.lock") && line.starts_with("who = ") {
            return true;
        }
        let mut line = line.to_owned();
        if file.starts_with("spec") {
            for capture in evidence_captures() {
                line = line.replace(capture, "capture.ndjson");
            }
        }
        // Only pinned source citations can contain these upstream identities.
        if file == Path::new("spec/annex-b-dxspider.md") {
            line = line
                .replace(
                    &["EA", "3CV/dxspider/blob/"].concat(),
                    "Q1ABC/dxspider/blob/",
                )
                .replace(&["VE", "7CC.pm"].concat(), "Q1ABC.pm")
                .replace(&["ve7cc", ".pl"].concat(), "Q1ABC.pl");
        }
        if file == Path::new("spec/evidence/arcluster-user-guide-2026-11-16.txt")
            && line.starts_with("L26: ")
        {
            line = line.replace(
                &["Server 6.6.", "247.0 Stable"].concat(),
                "Server VERSION Stable",
            );
        }
        text_sanitized(
            &unescape_source(&line)
                .replace("matches(\"::\")", "matches(\"colon\")")
                .replace("contains(\"::\")", "contains(\"colon\")"),
        )
    })
}
pub fn sanitization() -> Result<(), String> {
    let paths = public_paths()?;
    for path in [
        "crates/qso-graph-dxcluster-cli/src/recording.rs",
        "crates/qso-graph-dxcluster/src/privacy.rs",
        "xtask/src/checks.rs",
        "fuzz/fuzz_targets/session.rs",
    ] {
        if !paths.contains(&PathBuf::from(path)) {
            return Err(format!("source omitted from sanitization: {path}"));
        }
        for planted in [
            ["W", "1", "ABC"].concat(),
            ["contact", "example.invalid"].join("@"),
            ["node", "internal"].join("."),
        ] {
            let source = format!("#[test] fn fixture() {{ let data = {planted:?}; }}");
            if public_file_sanitized(Path::new(path), &source) {
                return Err(format!("inline planted fixture accepted: {path}"));
            }
        }
    }
    for file in paths {
        let s = fs::read_to_string(&file).map_err(|e| e.to_string())?;
        if !public_file_sanitized(&file, &s) {
            let line = s
                .lines()
                .position(|line| !public_file_sanitized(&file, line))
                .map(|n| n + 1)
                .unwrap_or(0);
            return Err(format!(
                "unsanitized identity/address in {}:{line}",
                file.display()
            ));
        }
    }
    println!("all public files and decoded vectors sanitized; inline planted cases rejected");
    Ok(())
}
pub fn docs() -> Result<(), String> {
    for path in [
        "README.md",
        "CONTRIBUTING.md",
        "SECURITY.md",
        "CHANGELOG.md",
        "docs/CONFORMANCE.md",
        "docs/DEPENDENCIES.md",
    ] {
        if !fs::read_to_string(path)
            .map_err(|e| e.to_string())?
            .contains("SPDX-License-Identifier: GPL-3.0-or-later")
        {
            return Err(format!("missing SPDX: {path}"));
        }
    }
    Ok(())
}

pub fn vet() -> Result<(), String> {
    let store = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .join("target/verify/vet-store");
    fs::create_dir_all(&store).map_err(|e| e.to_string())?;
    for file in ["config.toml", "audits.toml", "imports.lock"] {
        fs::copy(Path::new("supply-chain").join(file), store.join(file))
            .map_err(|e| e.to_string())?;
    }
    // cargo-vet requires its own canonical generated comments. Normalize only a
    // temporary store; project-owned source files retain their SPDX headers.
    for args in [vec!["fmt"], vec!["--locked", "--frozen"]] {
        let status = std::process::Command::new("cargo")
            .arg("vet")
            .args(args)
            .arg("--store-path")
            .arg(&store)
            .env("CARGO_NET_OFFLINE", "true")
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("cargo-vet failed; committed store is unchanged".into());
        }
    }
    Ok(())
}
pub fn seed_fuzz() -> Result<(), String> {
    for target in [
        "values", "dx_de", "cc11", "other", "rbn", "framer", "telnet", "session",
    ] {
        let dir = Path::new("fuzz/corpus").join(target);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let seed = match target {
            "values" => b"14025.123".as_slice(),
            "cc11" => b"CC11^1^Q2ABC^1-Nov-2026^1832^^Q1ABC^^^^^^^^^^^^^^^^^^".as_slice(),
            "other" => b"To ALL de Q1ABC: Hello\r\n".as_slice(),
            "telnet" => b"\xff\xfa\x01\xff\xff\xff\xf0\r\n".as_slice(),
            "session" => b"\x04\x00\x01login:Q1ABC de RELAY 1-Nov-2026 18:32Z >\x03\x04".as_slice(),
            "rbn" => b"DX de Q9XYZ-#: 14074 Q2ABC FT8 -12 dB AA00 CQ 1832Z\n".as_slice(),
            _ => b"DX de Q1ABC: 14025 Q2ABC CQ 1832Z\r\n".as_slice(),
        };
        fs::write(dir.join("synthetic"), seed).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checks_reject_mutations() {
        // QGDXC-SEC-0004 QGDXC-SEC-0006
        assert!(source_violation("std::{net}", true));
        assert!(source_violation("unsafe{}", false));
        assert!(source_violation(
            "let _ = std::time::UNIX_EPOCH.elapsed();",
            true
        ));
        assert!(source_violation(
            "use std::{time as clocks}; clocks::UNIX_EPOCH.elapsed();",
            true
        ));
        let path = Path::new("crates/qso-graph-dxcluster-cli/src/recording.rs");
        for data in [
            ["person", "example.invalid"].join("@"),
            ["node", "internal"].join("."),
        ] {
            let source = format!("#[test] fn inline_fixture() {{ let data = {data:?}; }}");
            assert!(!public_file_sanitized(path, &source));
            let encoded = crate::oracle::b64(data.as_bytes());
            let fixture = format!("let data = json!({{\"b64\":\"{encoded}\"}});");
            assert!(!public_file_sanitized(path, &fixture));
            let escaped: String = data.bytes().map(|b| format!("\\x{b:02x}")).collect();
            assert!(!public_file_sanitized(
                path,
                &format!("let bytes = b\"{escaped}\";")
            ));
        }
        assert!(!text_sanitized(&["N", "9", "XYZ"].concat()));
        assert!(!text_sanitized(&["private", "internal"].join(".")));
        let ip = ["10", "0", "0", "1"].join(".");
        assert!(!text_sanitized(&ip));
        assert!(!text_sanitized(&format!("tcp://{ip}:7300")));
    }
    #[test]
    fn coverage_rejects_inspection_only_tests_and_deleted_vectors() {
        let root = PathBuf::from(format!("target/coverage-self-test-{}", std::process::id()));
        for dir in ["spec", "docs", "vectors", "tests"] {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        fs::write(root.join("spec/test.md"), "<!-- SPDX-License-Identifier: GPL-3.0-or-later -->\n| `QGDXC-FRM-0001` | rule | Test, Inspection |\n").unwrap();
        fs::write(
            root.join("docs/CONFORMANCE.md"),
            "Inspection: QGDXC-FRM-0001\n",
        )
        .unwrap();
        let mut map = serde_json::json!({"requirements":{"QGDXC-FRM-0001":{"tests":[],"vectors":[]}}, "scenarios":{"framing":["ending-lf"]}});
        let vector =
            serde_json::json!({"id":"ending-lf","spec":"0.1.0","steps":[],"expected_events":[]});
        fs::write(root.join("vectors/test.ndjson"), vector.to_string()).unwrap();
        fs::write(root.join("docs/coverage.json"), map.to_string()).unwrap();
        assert!(
            coverage_at(&root, None)
                .unwrap_err()
                .contains("Test requirement")
        );
        map["requirements"]["QGDXC-FRM-0001"]["tests"] =
            serde_json::json!([{"file":"tests/case.rs","name":"framing"}]);
        fs::write(root.join("docs/coverage.json"), map.to_string()).unwrap();
        fs::write(root.join("tests/case.rs"), "// SPDX-License-Identifier: GPL-3.0-or-later\n#[test]\nfn framing() { assert!(true); }\n").unwrap();
        coverage_at(&root, None).unwrap();
        fs::write(
            root.join("tests/case.rs"),
            "// QGDXC-FRM-0001 is only a comment\n",
        )
        .unwrap();
        assert!(
            coverage_at(&root, None)
                .unwrap_err()
                .contains("missing named test")
        );
        fs::write(
            root.join("tests/case.rs"),
            "#[test]\nfn framing() { assert!(true); }\n",
        )
        .unwrap();
        fs::remove_dir_all(root.join("vectors")).unwrap();
        assert!(
            coverage_at(&root, None)
                .unwrap_err()
                .contains("missing scenario")
        );
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod privacy_complexity_tests {
    #[test]
    fn hostile_ip_runs_have_bounded_processing() {
        let start = std::time::Instant::now();
        for length in [256, 512, 1024, 4096, 65536] {
            for byte in *b":.f" {
                qso_graph_dxcluster::privacy::redact(&vec![byte; length]);
            }
        }
        assert!(start.elapsed() < std::time::Duration::from_secs(2));
    }
}
