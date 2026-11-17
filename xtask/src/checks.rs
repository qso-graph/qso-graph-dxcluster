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
pub fn coverage() -> Result<(), String> {
    let mut defined = BTreeSet::new();
    for file in files(Path::new("spec"))? {
        if file.extension().is_some_and(|e| e == "md") {
            for line in fs::read_to_string(file)
                .map_err(|e| e.to_string())?
                .lines()
                .filter(|l| l.starts_with("| `QGDXC-"))
            {
                let id = line.split('`').nth(1).ok_or("requirement table")?;
                if !defined.insert(id.to_owned()) {
                    return Err(format!("duplicate requirement definition: {id}"));
                }
            }
        }
    }
    let mut tested = BTreeSet::new();
    for root in ["crates", "xtask/src"] {
        for file in files(Path::new(root))? {
            if file.extension().is_some_and(|e| e == "rs") {
                let s = fs::read_to_string(&file).map_err(|e| e.to_string())?;
                if file.components().any(|c| c.as_os_str() == "tests") {
                    tested.extend(identifiers(&s));
                } else if let Some((_, tests)) = s.split_once("#[cfg(test)]") {
                    tested.extend(identifiers(tests));
                }
            }
        }
    }
    for file in files(Path::new("vectors"))? {
        tested.extend(identifiers(
            &fs::read_to_string(file).map_err(|e| e.to_string())?,
        ));
    }
    let notes = fs::read_to_string("docs/CONFORMANCE.md").map_err(|e| e.to_string())?;
    let mut inspected = BTreeSet::new();
    for line in notes.lines().filter(|l| l.contains("Inspection:")) {
        inspected.extend(identifiers(line));
    }
    let covered: BTreeSet<_> = tested.union(&inspected).cloned().collect();
    let missing: Vec<_> = defined.difference(&covered).collect();
    if !missing.is_empty() {
        return Err(format!("uncovered requirements: {missing:?}"));
    }
    println!(
        "{} / {} requirements covered ({} tested/vector, {} inspection-only)",
        defined.len(),
        defined.len(),
        defined.intersection(&tested).count(),
        defined.difference(&tested).count()
    );
    Ok(())
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
    w.iter().any(|w| forbidden.contains(w))
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
    if s.contains('@') {
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
    }) {
        if t.contains('@')
            || t.ends_with(".local")
            || t.ends_with(".lan")
            || t.ends_with(".internal")
        {
            return false;
        }
        // References to capture basenames are provenance, never endpoint examples.
        if t.ends_with(".ndjson") || matches!(t, "set/ve7cc" | "ve7cc") {
            continue;
        }
        for sub in t.split(['/', '-', '_', ':', '.']) {
            if likely_call(sub) {
                return false;
            }
        }
        let literal = t.trim_matches(['[', ']', ',']);
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
    true
}
fn vector_sanitized(v: &Value) -> bool {
    match v {
        Value::Object(o) => o.iter().all(|(k, v)| {
            if k == "capture" {
                return true;
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
pub fn sanitization() -> Result<(), String> {
    let planted = ["W", "1", "ABC"].concat();
    if text_sanitized(&planted) {
        return Err("sanitization planted callsign escaped detection".into());
    }
    if !text_sanitized("Q1ABC Q9XYZ-2-# example.invalid") {
        return Err("sanitizer rejects invented identities".into());
    }
    // Actually plant a public fixture, scan it, and remove it before continuing.
    let path = Path::new("vectors/.sanitization-planted.txt");
    fs::write(path, &planted).map_err(|e| e.to_string())?;
    let rejected = !text_sanitized(&fs::read_to_string(path).map_err(|e| e.to_string())?);
    fs::remove_file(path).map_err(|e| e.to_string())?;
    if !rejected {
        return Err("planted fixture accepted".into());
    }
    let mut paths = vec![PathBuf::from("README.md")];
    for dir in [
        "vectors",
        "docs",
        "crates/qso-graph-dxcluster/tests",
        "fixtures",
    ] {
        paths.extend(files(Path::new(dir))?);
    }
    for file in paths {
        let s = fs::read_to_string(&file).map_err(|e| e.to_string())?;
        let good = if file.extension().is_some_and(|e| e == "ndjson") {
            s.lines()
                .all(|l| serde_json::from_str::<Value>(l).is_ok_and(|v| vector_sanitized(&v)))
        } else {
            text_sanitized(&s.replace("\\x", ""))
        };
        if !good {
            return Err(format!(
                "unsanitized identity/address in {}",
                file.display()
            ));
        }
    }
    println!("published evidence sanitized; planted real-looking call rejected and removed");
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
        assert!(!text_sanitized(&["N", "9", "XYZ"].concat()));
        assert!(!text_sanitized("private.internal"));
        assert!(!text_sanitized(&["10", "0", "0", "1"].join(".")));
    }
}
