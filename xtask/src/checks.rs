// SPDX-License-Identifier: GPL-3.0-or-later
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};
pub fn files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = vec![];
    collect_files(root, &mut out, &mut crate::input::Budget::new(), 0)?;
    Ok(out)
}
fn collect_files(
    root: &Path,
    out: &mut Vec<PathBuf>,
    budget: &mut crate::input::Budget,
    depth: usize,
) -> Result<(), String> {
    if depth > 64 {
        return Err("directory nesting exceeds 64 levels".into());
    }
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let e = entry.map_err(|e| e.to_string())?;
        if e.file_type().map_err(|e| e.to_string())?.is_symlink() {
            return Err(format!("symlink not allowed: {}", e.path().display()));
        }
        if e.path().is_dir() {
            collect_files(&e.path(), out, budget, depth + 1)?;
        } else {
            budget.charge(e.path().as_os_str().len() + 256)?;
            out.push(e.path());
        }
    }
    Ok(())
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
            for line in crate::input::text(file)
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
        &crate::input::text(root.join("docs/coverage.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mapped = manifest["requirements"]
        .as_object()
        .ok_or("coverage requirement map")?;
    let mut vectors = BTreeSet::new();
    let mut vector_budget = crate::input::Budget::new();
    for file in files(&root.join("vectors"))? {
        if file.extension().is_some_and(|e| e == "ndjson") {
            for line in crate::input::lines(&file)? {
                let v: Value = serde_json::from_str(&line?).map_err(|e| e.to_string())?;
                let id = v["id"].as_str().ok_or("vector id")?;
                vector_budget.charge(id.len() + 256)?;
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
    let notes = crate::input::text(root.join("docs/CONFORMANCE.md")).map_err(|e| e.to_string())?;
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
            let source = crate::input::text(root.join(file)).map_err(|e| format!("{file}: {e}"))?;
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
    let output = crate::input::output(
        std::process::Command::new("cargo")
            .args([
                "test",
                "--workspace",
                "--all-features",
                "--locked",
                "--",
                "--list",
            ])
            .env("CARGO_NET_OFFLINE", "true"),
    )
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
    let clock_path = compact.contains(concat!(":", ":time:", ":"))
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
                    // Reserve the attribute name conservatively, including
                    // inactive/nested cfg_attr and whitespace/comment variants.
                    "path",
                ]
                .contains(w)
            })
}
// QGDXC-IO-0001: no approved third-party library test dependencies yet.
const LIBRARY_DEV_ALLOWLIST: &[&str] = &[];
fn library_dependencies(metadata: &Value) -> Result<(), String> {
    let packages = metadata["packages"].as_array().ok_or("metadata packages")?;
    let library = packages
        .iter()
        .find(|p| p["name"] == "qso-graph-dxcluster")
        .ok_or("library missing from Cargo metadata")?;
    let dependencies = library["dependencies"]
        .as_array()
        .ok_or("metadata dependencies")?;
    let mut rejected = vec![];
    let mut development = vec![];
    for dependency in dependencies {
        let name = dependency["name"].as_str().ok_or("dependency name")?;
        let alias = dependency["rename"].as_str().unwrap_or(name);
        let target = dependency["target"].as_str().unwrap_or("all targets");
        let description = format!("{name} (as {alias}; {target})");
        if dependency["kind"] == "dev" {
            development.push(description.clone());
            if !LIBRARY_DEV_ALLOWLIST.contains(&name) {
                rejected.push(format!("unapproved dev dependency: {description}"));
            }
        } else {
            rejected.push(format!("normal/build dependency: {description}"));
        }
    }
    println!(
        "library dev-dependencies: {}",
        if development.is_empty() {
            "none".to_owned()
        } else {
            development.join(", ")
        }
    );
    if rejected.is_empty() {
        Ok(())
    } else {
        Err(rejected.join("\n"))
    }
}
fn dependency_policy(root: &Path) -> Result<(), String> {
    // Package declarations include optional, renamed, and target-specific
    // dependencies, even when the current host or features do not enable them.
    let output = crate::input::output(
        std::process::Command::new("cargo")
            .args([
                "metadata",
                "--locked",
                "--offline",
                "--format-version",
                "1",
                "--no-deps",
            ])
            .current_dir(root),
    )
    .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "Cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    library_dependencies(&serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?)
}
// Read rustc's first Make-style rule, whose prerequisites are the complete
// source inputs. Keep escaped spaces and Windows path separators intact.
fn dep_info_paths(source: &str) -> Result<Vec<PathBuf>, String> {
    let source = source.replace("\\\r\n", "").replace("\\\n", "");
    let (_, prerequisites) = source
        .lines()
        .next()
        .and_then(|line| line.split_once(": "))
        .ok_or("missing dep-info prerequisite rule")?;
    let mut paths = vec![];
    let mut word = String::new();
    let mut chars = prerequisites.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\'
            && chars
                .peek()
                .is_some_and(|c| matches!(c, ' ' | '\t' | '#' | '\\'))
        {
            word.push(chars.next().unwrap());
        } else if c.is_whitespace() {
            if !word.is_empty() {
                paths.push(PathBuf::from(std::mem::take(&mut word)));
            }
        } else {
            word.push(c);
        }
    }
    if !word.is_empty() {
        paths.push(PathBuf::from(word));
    }
    if paths.is_empty() {
        return Err("empty dep-info prerequisite rule".into());
    }
    Ok(paths)
}
fn compiled_library_sources(root: &Path) -> Result<BTreeSet<PathBuf>, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let allowed = root.join("crates/qso-graph-dxcluster/src");
    let manifest = root.join("crates/qso-graph-dxcluster/Cargo.toml");
    let output = crate::input::output(
        std::process::Command::new("cargo")
            .args([
                "check",
                "--locked",
                "--offline",
                "-p",
                "qso-graph-dxcluster",
                "--lib",
                "--tests",
                "--all-features",
                "--message-format=json",
                "--target-dir",
            ])
            .arg(root.join("target/policy-sources"))
            .current_dir(&root),
    )
    .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "library source compilation failed: {}\n{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        ));
    }
    let mut sources = BTreeSet::new();
    for line in output
        .stdout
        .split(|b| *b == b'\n')
        .filter(|l| !l.is_empty())
    {
        let artifact: Value = serde_json::from_slice(line).map_err(|e| e.to_string())?;
        if artifact["reason"] != "compiler-artifact"
            || artifact["target"]["kind"] != serde_json::json!(["lib"])
            || artifact["manifest_path"]
                .as_str()
                .and_then(|p| Path::new(p).canonicalize().ok())
                != Some(manifest.clone())
        {
            continue;
        }
        let metadata = artifact["filenames"]
            .as_array()
            .and_then(|names| {
                names
                    .iter()
                    .filter_map(Value::as_str)
                    .find(|p| p.ends_with(".rmeta"))
            })
            .map(Path::new)
            .ok_or("missing library metadata artifact")?;
        let stem = metadata
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.strip_prefix("lib"))
            .ok_or("invalid library metadata artifact")?;
        let dep_info = metadata.with_file_name(format!("{stem}.d"));
        let dependencies = crate::input::text(&dep_info)
            .map_err(|e| format!("cannot read {}: {e}", dep_info.display()))?;
        for path in dep_info_paths(&dependencies)? {
            if !path.extension().is_some_and(|e| e == "rs") {
                continue;
            }
            let path = root.join(path).canonicalize().map_err(|e| e.to_string())?;
            if !path.starts_with(&allowed) {
                return Err(format!(
                    "compiled library source outside src: {}",
                    path.display()
                ));
            }
            sources.insert(path);
        }
    }
    if !sources.contains(&allowed.join("lib.rs")) {
        return Err("library root missing from compiler dep-info".into());
    }
    Ok(sources)
}
pub fn policy() -> Result<(), String> {
    dependency_policy(Path::new("."))?;
    let sources = compiled_library_sources(Path::new("."))?;
    for source in &sources {
        let text = crate::input::text(source).map_err(|e| e.to_string())?;
        if source_violation(&text, true) {
            return Err(format!("compiled source policy: {}", source.display()));
        }
    }
    println!(
        "{} compiled library sources confined to src by compiler dep-info",
        sources.len()
    );
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
        "#[cfg_attr(all(), path = \"../../../shared.rs\")] mod shared;",
        "#[cfg_attr(any(), path = \"../../../shared.rs\")] mod shared;",
    ] {
        if !source_violation(planted, true) {
            return Err("policy planted-case detector failed".into());
        }
    }
    for (root, library) in [
        ("crates/qso-graph-dxcluster", true),
        ("crates/qso-graph-dxcluster-cli", false),
    ] {
        for file in files(Path::new(root))? {
            if file.file_name().is_some_and(|n| n == "build.rs")
                || file
                    .extension()
                    .is_some_and(|e| ["c", "cc", "cpp", "h", "o", "a"].iter().any(|s| e == *s))
            {
                return Err(format!("native source/build script: {}", file.display()));
            }
            if file.extension().is_some_and(|e| e == "rs") {
                let s = crate::input::text(&file).map_err(|e| e.to_string())?;
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
    if crate::input::text("rust-toolchain.toml")
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
    // Rust hexadecimal literals are source syntax, not received identities.
    if s.strip_prefix("0x")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return false;
    }
    let b = s.as_bytes();
    let mut i = usize::from(b[0].is_ascii_digit());
    let letters = i;
    while i < b.len() && b[i].is_ascii_alphabetic() {
        i += 1;
    }
    if i == letters || i - letters > 3 {
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
fn addresses_sanitized(s: &str) -> bool {
    fn permitted_literal(s: &str) -> bool {
        match s.parse::<std::net::IpAddr>() {
            Ok(std::net::IpAddr::V4(ip)) => {
                let o = ip.octets();
                o[..3] == [192, 0, 2] || o[..3] == [198, 51, 100] || o[..3] == [203, 0, 113]
            }
            Ok(std::net::IpAddr::V6(ip)) => ip.segments()[..2] == [0x2001, 0xdb8],
            Err(_) => true,
        }
    }
    // DNS labels and IPv4 have different boundaries from IPv6. Colons,
    // slashes, brackets, and prose punctuation delimit a host component.
    for host in s.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))) {
        // A final dot can be sentence punctuation or the DNS root label.
        let host = host.trim_matches('.');
        if [".local", ".lan", ".internal"].iter().any(|suffix| {
            host.get(host.len().saturating_sub(suffix.len())..)
                .is_some_and(|tail| tail.eq_ignore_ascii_case(suffix))
                && host.len() > suffix.len()
        }) || !permitted_literal(host)
        {
            return false;
        }
    }
    crate::addresses::visit(s, |range, _| permitted_literal(&s[range]))
}
fn text_sanitized(s: &str) -> bool {
    if !addresses_sanitized(s) {
        return false;
    }
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
            .filter_map(|p| crate::input::text(p).ok())
            .flat_map(|s| {
                s.split('`')
                    .filter(|s| s.ends_with(".ndjson") && !s.contains(['/', ' ', '\n']))
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect()
    })
}
fn identity_field(key: &str) -> bool {
    matches!(
        key,
        "identity"
            | "call"
            | "spotter_call"
            | "from_call"
            | "to_call"
            | "via_call"
            | "origin"
            | "slot_08"
            | "slot_09"
    )
}
fn fixture_identity(id: &str, key: &str, value: &str) -> bool {
    if value.starts_with(['Q', 'q']) {
        return true;
    }
    // Opaque optional native identities can be absent on the wire.
    if value.is_empty() && matches!(key, "origin" | "slot_08" | "slot_09") {
        return true;
    }
    // Exact named lexical cases test QGDXC-DX-0002's received-token grammar.
    // Neither an ID prefix nor a general exception to identity checks is used.
    if matches!(key, "call" | "spotter_call")
        && matches!(
            (id, value),
            ("lexical-0", "!") | ("lexical-1", "_") | ("lexical-2", "123") | ("lexical-3", "a")
        )
    {
        return true;
    }
    // These exact documentation-IP roles test QGDXC-SEC-0005 with retention.
    value == "192.0.2.1"
        && matches!(
            (id, key),
            ("privacy-0-true", "spotter_call")
                | ("privacy-1-true", "call")
                | ("privacy-2-true", "to_call")
                | ("privacy-3-true", "from_call")
                | ("privacy-4-true", "from_call")
                | ("cc11-ip-call-true", "call")
        )
}
fn vector_sanitized(v: &Value) -> bool {
    fn visit(v: &Value, id: &str, parent: &str) -> bool {
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
                if identity_field(k)
                    && (matches!(parent, "config" | "fields" | "native") || v.is_string())
                    && !v.as_str().is_some_and(|s| fixture_identity(id, k, s))
                {
                    return false;
                }
                visit(v, id, k)
            }),
            Value::Array(a) => a.iter().all(|v| visit(v, id, parent)),
            Value::String(s) => text_sanitized(s),
            _ => true,
        }
    }
    visit(v, v["id"].as_str().unwrap_or(""), "")
}
fn public_paths() -> Result<Vec<PathBuf>, String> {
    let output = crate::input::output(std::process::Command::new("git").args([
        "ls-files",
        "--cached",
        "--others",
        "--exclude-standard",
        "-z",
    ]))
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
    for key in [
        "identity",
        "call",
        "spotter_call",
        "from_call",
        "to_call",
        "via_call",
        "origin",
        "slot_08",
        "slot_09",
    ] {
        let marker = format!("\"{key}\"");
        for rest in source.split(&marker).skip(1) {
            if let Some(rest) = rest.trim_start().strip_prefix(':')
                && let Some(rest) = rest.trim_start().strip_prefix('"')
                && let Some((value, _)) = rest.split_once('"')
                && !fixture_identity("", key, value)
            {
                return false;
            }
        }
    }
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
// Keep literal and comment text when scanning Rust source for fixture data.
// Paths outside literals are syntax and can resemble short IPv6 addresses.
fn rust_fixture_text(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = vec![b' '; bytes.len()];
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        if bytes[i..].starts_with(b"//") {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if bytes[i..].starts_with(b"/*") {
            i += 2;
            let mut depth = 1;
            while i < bytes.len() && depth > 0 {
                if bytes[i..].starts_with(b"/*") {
                    depth += 1;
                    i += 2;
                } else if bytes[i..].starts_with(b"*/") {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
        } else if bytes[i] == b'\'' && bytes.get(i + 2) == Some(&b'\'') {
            i += 3;
            continue;
        } else if bytes[i..].starts_with(b"'\\") {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\'' {
                i += 1;
            }
            i = bytes.len().min(i + 1);
            continue;
        } else if bytes[i] == b'r' && bytes.get(i + 1).is_some_and(|b| matches!(b, b'#' | b'"')) {
            i += 1;
            while bytes.get(i) == Some(&b'#') {
                i += 1;
            }
            let hashes = i - start - 1;
            if bytes.get(i) != Some(&b'"') {
                continue;
            }
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'"'
                    && bytes
                        .get(i + 1..i + 1 + hashes)
                        .is_some_and(|tail| tail.iter().all(|b| *b == b'#'))
                {
                    i += 1 + hashes;
                    break;
                }
                i += 1;
            }
        } else if bytes[i] == b'"' {
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    i = bytes.len().min(i + 2);
                } else if bytes[i] == b'"' {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
        } else {
            if bytes[i] == b'\n' {
                out[i] = b'\n';
            }
            i += 1;
            continue;
        }
        out[start..i].copy_from_slice(&bytes[start..i]);
    }
    let text = String::from_utf8(out).expect("only complete UTF-8 text is retained");
    // Source policy fixtures and API references can themselves contain Rust
    // paths. Restrict the exception to the API roots used by those fixtures;
    // arbitrary label prefixes still go through literal candidate extraction.
    text.split_inclusive(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | ':')))
        .map(|part| {
            let token = part
                .trim_end_matches(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | ':')));
            let parts: Vec<_> = token.split(concat!(":", ":")).collect();
            if parts.len() > 1
                && matches!(
                    parts[0],
                    "std"
                        | "crate"
                        | "Self"
                        | "Session"
                        | "Parser"
                        | "State"
                        | "Timer"
                        | "Profile"
                        | "Action"
                        | "Value"
                        | "Event"
                        | "Config"
                        | "Batch"
                        | "inside"
                        | "alias"
                        | "clocks"
                        | "time"
                )
                && parts.iter().enumerate().all(|(index, p)| {
                    (p.is_empty() && index + 1 == parts.len())
                        || p.as_bytes()
                            .first()
                            .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_')
                            && p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                            && p.bytes().any(|b| !b.is_ascii_hexdigit())
                })
            {
                format!("{}{}", " ".repeat(token.len()), &part[token.len()..])
            } else {
                part.to_owned()
            }
        })
        .collect()
}
pub(crate) fn public_file_sanitized(file: &Path, source: &str) -> bool {
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
    let fixture_text;
    let source = if file.extension().is_some_and(|e| e == "rs") {
        fixture_text = rust_fixture_text(
            &source
                .replace("matches(\"::\")", "matches(\"colon\")")
                .replace("contains(\"::\")", "contains(\"colon\")")
                .replace("contains(\":::\")", "contains(\"colon\")"),
        );
        &fixture_text
    } else {
        source
    };
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
                .replace("contains(\"::\")", "contains(\"colon\")")
                .replace("contains(\":::\")", "contains(\"colon\")"),
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
            ["4X", "1", "ABC"].concat(),
            ["contact", "example.invalid"].join("@"),
            ["node", "internal"].join("."),
        ] {
            let source = format!("#[test] fn fixture() {{ let data = {planted:?}; }}");
            if public_file_sanitized(Path::new(path), &source) {
                return Err(format!("inline planted fixture accepted: {path}"));
            }
        }
    }
    let planted = ["4X", "1", "ABC"].concat();
    let wire = format!("DX de {planted}: 14025 Q2ABC CQ 1832Z");
    let fixture = crate::generate::vector(
        "planted-digit-initial",
        "dxspider",
        wire.as_bytes(),
        "lf",
        None,
    );
    if vector_sanitized(&fixture) {
        return Err("planted digit-initial wire/structured fixture accepted".into());
    }
    for file in paths {
        if file.extension().is_some_and(|e| e == "ndjson") {
            for (i, line) in crate::input::lines(&file)?.enumerate() {
                if !public_file_sanitized(&file, &line?) {
                    return Err(format!(
                        "unsanitized identity/address in {}:{}",
                        file.display(),
                        i + 1
                    ));
                }
            }
            continue;
        }
        let s = crate::input::text(&file).map_err(|e| e.to_string())?;
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
        if !crate::input::text(path)
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
    fn dep_info_rejects_compiled_external_module_and_symlink() {
        // QGDXC-IO-0001: compile the planted module, without executing it.
        // Check locations independently of the attribute-name source detector.
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../target")
            .join(format!("policy sources-{}", std::process::id()));
        let source = root.join("crates/qso-graph-dxcluster/src");
        fs::create_dir_all(&source).unwrap();
        fs::write(root.join("Cargo.toml"), "# SPDX-License-Identifier: GPL-3.0-or-later\n[workspace]\nmembers = [\"crates/qso-graph-dxcluster\"]\nresolver = \"3\"\n").unwrap();
        fs::write(root.join("Cargo.lock"), "# SPDX-License-Identifier: GPL-3.0-or-later\nversion = 4\n[[package]]\nname = \"qso-graph-dxcluster\"\nversion = \"0.1.0\"\n").unwrap();
        fs::write(source.join("../Cargo.toml"), "# SPDX-License-Identifier: GPL-3.0-or-later\n[package]\nname = \"qso-graph-dxcluster\"\nversion = \"0.1.0\"\nedition = \"2024\"\n").unwrap();
        let inside = "// SPDX-License-Identifier: GPL-3.0-or-later\npub fn probe() {}\n";
        let library = "// SPDX-License-Identifier: GPL-3.0-or-later\n#![forbid(unsafe_code)]\nmod inside;\npub struct Session;\nimpl Session { pub fn new() -> Self { inside::probe(); Self } }\n";
        fs::write(source.join("inside.rs"), inside).unwrap();
        fs::write(source.join("lib.rs"), library).unwrap();
        let baseline = compiled_library_sources(&root).unwrap();
        assert_eq!(baseline.len(), 2);
        assert!(baseline.contains(&source.join("inside.rs").canonicalize().unwrap()));
        fs::write(root.join("shared.rs"), "// SPDX-License-Identifier: GPL-3.0-or-later\npub fn probe() { let _ = std::fs::metadata(\"Cargo.toml\"); }\n").unwrap();
        let redirected = format!(
            "{}\n#[cfg_attr(all(), path = \"../../../shared.rs\")] mod shared;\n",
            library.replace(
                "inside::probe();",
                "inside::probe(); crate::shared::probe();"
            )
        );
        fs::write(source.join("lib.rs"), &redirected).unwrap();
        assert!(source_violation(&redirected, true));
        let error = compiled_library_sources(&root).unwrap_err();
        assert!(
            error.contains("compiled library source outside src:"),
            "{error}"
        );
        assert!(error.contains("shared.rs"), "{error}");
        fs::write(source.join("lib.rs"), library).unwrap();
        assert_eq!(compiled_library_sources(&root).unwrap(), baseline);
        // A conventional module name can also escape through a symlink, with
        // no path attribute at all. Canonical source locations catch this.
        #[cfg(unix)]
        {
            fs::remove_file(source.join("inside.rs")).unwrap();
            std::os::unix::fs::symlink(
                root.join("shared.rs").canonicalize().unwrap(),
                source.join("inside.rs"),
            )
            .unwrap();
            let error = compiled_library_sources(&root).unwrap_err();
            assert!(
                error.contains("compiled library source outside src:"),
                "{error}"
            );
            fs::remove_file(source.join("inside.rs")).unwrap();
            fs::write(source.join("inside.rs"), inside).unwrap();
            assert_eq!(compiled_library_sources(&root).unwrap(), baseline);
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn dep_info_reads_escaped_paths_and_ignores_phony_rules() {
        let source = "artifact.d: src/lib.rs src/nested\\ space.rs \\\n C:\\project\\src\\values.rs\n\nsrc/lib.rs:\n";
        assert_eq!(
            dep_info_paths(source).unwrap(),
            vec![
                PathBuf::from("src/lib.rs"),
                PathBuf::from("src/nested space.rs"),
                PathBuf::from("C:\\project\\src\\values.rs"),
            ]
        );
        assert!(dep_info_paths("").is_err());
        assert!(dep_info_paths("artifact.d: \n").is_err());
    }
    #[test]
    fn source_policy_rejects_conditional_module_paths() {
        // QGDXC-IO-0001: include inactive and nested attribute forms.
        for source in [
            "#[path = \"../../../shared.rs\"] mod shared;",
            "#[cfg_attr(all(), path = \"../../../shared.rs\")] mod shared;",
            "#[cfg_attr(any(), path = \"../../../shared.rs\")] mod shared;",
            "# [cfg_attr(all(), cfg_attr(all(), path /* gap */ = \"../../../shared.rs\"))] mod shared;",
        ] {
            assert!(source_violation(source, true), "accepted {source}");
        }
    }
    #[test]
    fn metadata_rejects_dependency_tables_on_every_target() {
        // QGDXC-IO-0001: Cargo, rather than manifest text matching, identifies
        // each dependency declaration. No network or compilation is needed.
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../target")
            .join(format!("policy-dependencies-{}", std::process::id()));
        fs::create_dir_all(root.join("src")).unwrap();
        let base = "# SPDX-License-Identifier: GPL-3.0-or-later\n[workspace]\n[package]\nname = \"qso-graph-dxcluster\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";
        fs::write(
            root.join("src/lib.rs"),
            "// SPDX-License-Identifier: GPL-3.0-or-later\n#![forbid(unsafe_code)]\n",
        )
        .unwrap();
        fs::write(
            root.join("Cargo.lock"),
            crate::input::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../Cargo.lock"))
                .unwrap(),
        )
        .unwrap();
        for declaration in [
            "",
            "[dependencies.ctrlc]\nversion = \"=3.5.1\"\n",
            "[dependencies]\nsignals = { package = \"ctrlc\", version = \"=3.5.1\", optional = true }\n",
            "[build-dependencies.ctrlc]\nversion = \"=3.5.1\"\n",
            "[target.'cfg(target_os = \"none\")'.dependencies.ctrlc]\nversion = \"=3.5.1\"\n",
            "[target.'cfg(target_os = \"none\")'.build-dependencies]\nsignals = { package = \"ctrlc\", version = \"=3.5.1\" }\n",
            "[dev-dependencies.ctrlc]\nversion = \"=3.5.1\"\n",
            "[target.'cfg(target_os = \"none\")'.dev-dependencies]\nsignals = { package = \"ctrlc\", version = \"=3.5.1\" }\n",
        ] {
            fs::write(root.join("Cargo.toml"), format!("{base}{declaration}")).unwrap();
            let result = dependency_policy(&root);
            if declaration.is_empty() {
                result.unwrap();
            } else {
                let error = result.unwrap_err();
                assert!(
                    error.contains("dependency: ctrlc"),
                    "{declaration}: {error}"
                );
                assert!(!error.contains("Cargo metadata failed"), "{error}");
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn sanitization_rejects_digit_initial_calls_and_identity_field_mutations() {
        // QGDXC-SEC-0006
        let planted = ["4X", "1", "ABC"].concat();
        let line = format!("DX de {planted}: 14025 Q2ABC CQ 1832Z");
        let v = crate::generate::vector(
            "planted-digit-initial",
            "dxspider",
            line.as_bytes(),
            "lf",
            None,
        );
        // The generator independently changes both wire input and expected fields.
        assert_eq!(
            v["expected_events"][0]["event"]["fields"]["spotter_call"],
            planted
        );
        assert!(!vector_sanitized(&v));
        assert!(!text_sanitized(&planted));
        for call in [
            ["3D", "2", "ABC"].concat(),
            ["3DA", "0", "ABC"].concat(),
            planted.to_ascii_lowercase(),
        ] {
            assert!(!text_sanitized(&call));
        }
        let path = Path::new("crates/qso-graph-dxcluster-cli/src/recording.rs");
        for key in [
            "identity",
            "call",
            "spotter_call",
            "from_call",
            "to_call",
            "via_call",
            "origin",
        ] {
            for value in [&planted, "RELAY"] {
                let source = format!("let fixture = json!({{\"{key}\":\"{value}\"}});");
                assert!(!public_file_sanitized(path, &source));
            }
        }

        let base = crate::generate::vector(
            "identity-fields",
            "dxspider",
            b"DX de Q1ABC: 14025 Q2ABC CQ 1832Z",
            "lf",
            None,
        );
        assert!(vector_sanitized(&base));
        for key in [
            "identity",
            "call",
            "spotter_call",
            "from_call",
            "to_call",
            "via_call",
            "origin",
            "slot_08",
            "slot_09",
        ] {
            for bad in [&planted, "RELAY", "123"] {
                let mut v = base.clone();
                v["expected_events"][0]["event"]["fields"][key] = serde_json::json!(bad);
                assert!(!vector_sanitized(&v), "accepted {key}");
            }
        }
        for v in crate::scenarios::vectors() {
            assert!(vector_sanitized(&v), "{}", v["id"]);
        }
        let mut lexical =
            crate::generate::vector("lexical-0", "arcluster", b"DX de !: 1 ! 0000Z", "lf", None);
        assert!(vector_sanitized(&lexical));
        lexical["expected_events"][0]["event"]["fields"]["call"] = serde_json::json!(planted);
        assert!(!vector_sanitized(&lexical));
        lexical["expected_events"][0]["event"]["fields"]["call"] = serde_json::json!("123");
        assert!(!vector_sanitized(&lexical));
    }
    #[test]
    fn sanitization_rejects_labeled_ipv6_in_prose_and_decoded_fixture() {
        // QGDXC-SEC-0006
        let literal = ["fd00", "", "1"].join(":");
        let prose = format!("The peer:{literal} endpoint is private.");
        let prose_rejected = !public_file_sanitized(Path::new("README.md"), &prose);
        let fixture = serde_json::json!({"b64": crate::oracle::b64(prose.as_bytes())});
        let fixture_rejected =
            !public_file_sanitized(Path::new("vectors/planted.ndjson"), &fixture.to_string());
        assert!(
            prose_rejected && fixture_rejected,
            "prose rejected: {prose_rejected}; decoded fixture rejected: {fixture_rejected}"
        );
        for source in [
            format!("let fixture = b\"peer:{literal}\";"),
            format!("let fixture = r#\"peer:{literal}\"#;"),
            format!("/* outer /* inner */ peer:{literal} */"),
            format!("let text = \"first line\npeer:{literal}\";"),
        ] {
            assert!(!public_file_sanitized(
                Path::new("crates/qso-graph-dxcluster-cli/src/main.rs"),
                &source
            ));
        }
        let hex_tail = ["fd00", "", "face"].join(":");
        let source = format!("let text = \"std::{hex_tail}\";");
        assert!(!public_file_sanitized(
            Path::new("xtask/src/checks.rs"),
            &source
        ));
        assert!(public_file_sanitized(
            Path::new("xtask/src/checks.rs"),
            "let source = \"use std::{fs};\"; let quote = b'\"';"
        ));
    }
    #[test]
    fn sanitization_rejects_punctuated_addresses_in_prose_and_decoded_fixtures() {
        // QGDXC-SEC-0006
        let addresses = [
            ["10", "20", "30", "40"].join("."),
            ["fd00", "1234", "", "1"].join(":"),
            ["fd00", "", "1"].join(":"),
            [
                ["", "", "ffff", ""].join(":"),
                ["10", "20", "30", "40"].join("."),
            ]
            .concat(),
            ["node", "internal"].join("."),
            ["node", "LOCAL"].join("."),
            ["node", "lan"].join("."),
        ];
        for address in addresses {
            for (before, after) in [
                ("peer:", ""),
                (concat!("peer:", ":"), ""),
                ("dead:", "."),
                ("peer.", "%zone"),
                ("", "."),
                ("", ","),
                ("(", ")"),
                ("", ":"),
                ("\"", "\""),
                ("[", "]"),
                ("('", ".'),"),
                ("tcp://", ".:7300/path"),
            ] {
                let prose = format!("Connect to {before}{address}{after}");
                assert!(
                    !public_file_sanitized(Path::new("README.md"), &prose),
                    "accepted prose: {prose}"
                );
                let encoded = crate::oracle::b64(prose.as_bytes());
                for key in ["b64", "raw_line"] {
                    let fixture = serde_json::json!({key: encoded});
                    assert!(
                        !public_file_sanitized(
                            Path::new("vectors/planted.ndjson"),
                            &fixture.to_string()
                        ),
                        "accepted decoded {key}: {prose}"
                    );
                    let source = format!("let fixture = json!({fixture});");
                    assert!(!public_file_sanitized(
                        Path::new("xtask/src/runner.rs"),
                        &source
                    ));
                }
            }
        }
        for safe in [
            "Connect to example.invalid.",
            "Connect to (192.0.2.1).",
            "Connect to [2001:db8::1]:7300.",
            "See docs/CONFORMANCE.md.",
        ] {
            assert!(
                public_file_sanitized(Path::new("README.md"), safe),
                "{safe}"
            );
        }
    }
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
