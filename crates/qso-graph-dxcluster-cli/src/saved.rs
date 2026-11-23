// SPDX-License-Identifier: GPL-3.0-or-later
//! The operator's own sources file for `qg-dxc tui` (QGDXC-TUI-0006, QGDXC-TUI-0007,
//! QGDXC-TUI-0009). Nothing here writes a file; the project ships no sources file.
use qso_graph_dxcluster::{Config, Profile, values};
use qso_graph_grid::{Locator, Position, Precision};
use serde_core::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    fmt,
    io::Read,
    net::Ipv6Addr,
    path::{Path, PathBuf},
};

/// Largest sources file read, in bytes (QGDXC-TUI-0006).
pub const MAX_BYTES: usize = 65_536;
/// Most saved sources in one file (QGDXC-TUI-0007).
pub const MAX_SOURCES: usize = 64;
/// Longest source name, in bytes (QGDXC-TUI-0007).
pub const MAX_NAME: usize = 32;

/// Why a `host:port` endpoint was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum NodeError {
    /// The endpoint text itself.
    Endpoint(&'static str),
    /// The library's configuration check, which also covers the call.
    Config(String),
}

/// Parse a `host:port` or `[address]:port` endpoint into a receive-only configuration,
/// under QGDXC-TUI-0001 and QGDXC-IO-0003. Returns the configuration and its host.
pub fn node(endpoint: &str, profile: Profile, call: &str) -> Result<(Config, String), NodeError> {
    let error = NodeError::Endpoint;
    let (host, port) = if let Some(rest) = endpoint.strip_prefix('[') {
        let (host, port) = rest
            .split_once("]:")
            .ok_or(error("expected [address]:port"))?;
        host.parse::<Ipv6Addr>()
            .map_err(|_| error("invalid IPv6 literal"))?;
        (host, port)
    } else {
        if endpoint.matches(':').count() > 1 {
            return Err(error("IPv6 requires brackets"));
        }
        let (host, port) = endpoint
            .split_once(':')
            .ok_or(error("expected host:port"))?;
        if host.contains([':', '[', ']']) {
            return Err(error("IPv6 requires brackets"));
        }
        let name = host.strip_suffix('.').unwrap_or(host);
        if name.is_empty()
            || name.split('.').any(|label| {
                label.is_empty()
                    || label.len() > 63
                    || label.starts_with('-')
                    || label.ends_with('-')
                    || !label
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            })
        {
            return Err(error("invalid host"));
        }
        (host, port)
    };
    if port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) {
        return Err(error("port must be 1 to 65535"));
    }
    let port = port
        .parse::<u16>()
        .ok()
        .filter(|p| *p != 0)
        .ok_or(error("port must be 1 to 65535"))?;
    let mut config = Config::new(host.into(), port, call.into(), profile);
    config.retain_third_party_data = false;
    config
        .validate()
        .map_err(|e| NodeError::Config(e.to_string()))?;
    Ok((config, host.to_owned()))
}

/// A profile name accepted by `--node …/<profile>` and the sources file.
pub fn profile(name: &str) -> Option<Profile> {
    Profile::parse(name).filter(|_| name != "unknown")
}

/// A validated sources file.
#[derive(Clone)]
pub struct Saved {
    /// The file's default callsign for cluster nodes.
    pub call: Option<String>,
    pub entries: Vec<Entry>,
    pub grid: Option<Position>,
}

impl fmt::Debug for Saved {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // QGDXC-TUI-0141: the operator's place is for display only.
        f.debug_struct("Saved")
            .field("call", &self.call)
            .field("entries", &self.entries)
            .finish_non_exhaustive()
    }
}

/// One saved source, validated and ready to run.
#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub node: String,
    pub selected: bool,
    /// The cluster connection settings, absent for a POTA source.
    pub config: Option<Config>,
}

/// The default sources file path (QGDXC-TUI-0006). `var` reads the environment.
pub fn default_path(
    windows: bool,
    var: impl Fn(&str) -> Option<OsString>,
) -> Result<PathBuf, String> {
    let absolute = |name: &str| {
        var(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
    };
    let base = if windows {
        absolute("APPDATA").ok_or("APPDATA must be set to an absolute path")?
    } else if let Some(base) = absolute("XDG_CONFIG_HOME") {
        base
    } else {
        absolute("HOME")
            .ok_or("HOME must be set to an absolute path")?
            .join(".config")
    };
    Ok(base.join("qso-graph").join("dxcluster-sources.json"))
}

/// Read and validate a sources file. The path must name a regular file, and at most
/// `MAX_BYTES` are accepted.
pub fn read(path: &Path) -> Result<Saved, String> {
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() {
        return Err("expected a regular file".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    parse(&bytes)
}

/// Validate sources-file bytes. Any error rejects the whole file.
pub fn parse(bytes: &[u8]) -> Result<Saved, String> {
    if bytes.len() > MAX_BYTES {
        return Err(format!("larger than {MAX_BYTES} bytes"));
    }
    let json: Json = serde_json::from_slice(bytes).map_err(|e| format!("invalid JSON: {e}"))?;
    let top = members(
        &json,
        "the file",
        &["version", "sources"],
        &["call", "grid"],
    )?;
    match top[0] {
        Json::Number(Some(1)) => {}
        _ => return Err("version: must be 1".into()),
    }
    let call = optional_call(top[2], "call")?;
    let grid = match top[3] {
        Json::Missing => None,
        value => Some(grid_position(string(value, "grid")?, "grid")?),
    };
    let Json::Array(list) = top[1] else {
        return Err("sources: must be an array".into());
    };
    if list.is_empty() || list.len() > MAX_SOURCES {
        return Err(format!("sources: must hold 1 to {MAX_SOURCES} sources"));
    }
    let mut names = BTreeSet::new();
    let mut endpoints = BTreeSet::new();
    let mut entries = Vec::with_capacity(list.len());
    let mut pota = false;
    for (i, item) in list.iter().enumerate() {
        let at = |member: &str| format!("sources[{i}].{member}");
        let fields = members(
            item,
            &format!("sources[{i}]"),
            &["name", "node"],
            &["profile", "selected", "call"],
        )?;
        let name = string(fields[0], &at("name"))?;
        if name.is_empty()
            || name.len() > MAX_NAME
            || !name.bytes().all(|b| (0x20..=0x7e).contains(&b))
            || name.starts_with(' ')
            || name.ends_with(' ')
        {
            return Err(format!(
                "{}: must be 1 to {MAX_NAME} printable ASCII characters, without leading or trailing spaces",
                at("name")
            ));
        }
        if !names.insert(name.to_ascii_lowercase()) {
            return Err(format!("{}: duplicate name", at("name")));
        }
        let endpoint = string(fields[1], &at("node"))?;
        if endpoint.contains('/') {
            return Err(format!(
                "{}: give the profile in \"profile\", not after /",
                at("node")
            ));
        }
        let selected = match fields[3] {
            Json::Missing => false,
            Json::Bool(b) => *b,
            _ => return Err(format!("{}: must be true or false", at("selected"))),
        };
        let own = optional_call(fields[4], &at("call"))?;
        let config = if endpoint == "pota" {
            if !matches!(fields[2], Json::Missing) {
                return Err(format!("{}: POTA has no profile", at("profile")));
            }
            if std::mem::replace(&mut pota, true) {
                return Err(format!("{}: duplicate endpoint", at("node")));
            }
            None
        } else {
            let profile = match fields[2] {
                Json::Missing => Profile::Auto,
                value => profile(string(value, &at("profile"))?)
                    .ok_or_else(|| format!("{}: unsupported profile", at("profile")))?,
            };
            let call = own.or(call).ok_or_else(|| {
                format!("sources[{i}]: missing \"call\", and the file has no top-level \"call\"")
            })?;
            let (config, host) = node(endpoint, profile, call).map_err(|e| match e {
                NodeError::Endpoint(why) => format!("{}: {why}", at("node")),
                NodeError::Config(why) => format!("{}: {why}", at("node")),
            })?;
            if !endpoints.insert((host.to_ascii_lowercase(), config.port)) {
                return Err(format!("{}: duplicate endpoint", at("node")));
            }
            Some(config)
        };
        entries.push(Entry {
            name: name.to_owned(),
            node: endpoint.to_owned(),
            selected,
            config,
        });
    }
    Ok(Saved {
        call: call.map(str::to_owned),
        entries,
        grid,
    })
}

/// Validate an operator locator under QGDXC-TUI-0140 without echoing it.
pub fn grid_position(value: &str, at: &str) -> Result<Position, String> {
    Locator::parse(value)
        .ok()
        .filter(|grid| grid.precision() != Precision::Field)
        .map(Locator::centre)
        .ok_or_else(|| format!("{at}: expected a 4, 6 or 8 character Maidenhead locator"))
}

/// An optional callsign member, valid under QGDXC-IO-0003 when present.
fn optional_call<'a>(value: &'a Json, at: &str) -> Result<Option<&'a str>, String> {
    if let Json::Missing = value {
        return Ok(None);
    }
    let call = string(value, at)?;
    if !values::login(call) {
        return Err(format!("{at}: not a valid callsign"));
    }
    Ok(Some(call))
}

fn string<'a>(value: &'a Json, at: &str) -> Result<&'a str, String> {
    match value {
        Json::String(s) => Ok(s),
        _ => Err(format!("{at}: must be a string")),
    }
}

/// An object's members in the order of `required` then `optional`, each `Missing`
/// when absent; any other member is refused.
fn members<'a>(
    value: &'a Json,
    at: &str,
    required: &[&str],
    optional: &[&str],
) -> Result<Vec<&'a Json>, String> {
    let Json::Object(pairs) = value else {
        return Err(format!("{at}: must be an object"));
    };
    if let Some((key, _)) = pairs
        .iter()
        .find(|(k, _)| !required.contains(&k.as_str()) && !optional.contains(&k.as_str()))
    {
        return Err(format!("{at}: unexpected member \"{key}\""));
    }
    let find = |name: &str| pairs.iter().find(|(k, _)| k == name).map(|(_, v)| v);
    let mut out = Vec::with_capacity(required.len() + optional.len());
    for name in required {
        out.push(find(name).ok_or_else(|| format!("{at}: missing \"{name}\""))?);
    }
    for name in optional {
        out.push(find(name).unwrap_or(&Json::Missing));
    }
    Ok(out)
}

/// JSON as read, keeping member order so a duplicate member can be refused. Numbers
/// keep only a nonnegative integer value; no other number is ever valid here.
#[derive(Debug)]
enum Json {
    Missing,
    Null,
    Bool(bool),
    Number(Option<u64>),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(JsonVisitor)
    }
}

struct JsonVisitor;

impl<'de> Visitor<'de> for JsonVisitor {
    type Value = Json;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON value")
    }
    fn visit_unit<E>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }
    fn visit_bool<E>(self, v: bool) -> Result<Json, E> {
        Ok(Json::Bool(v))
    }
    fn visit_u64<E>(self, v: u64) -> Result<Json, E> {
        Ok(Json::Number(Some(v)))
    }
    fn visit_i64<E>(self, v: i64) -> Result<Json, E> {
        Ok(Json::Number(u64::try_from(v).ok()))
    }
    fn visit_f64<E>(self, _: f64) -> Result<Json, E> {
        Ok(Json::Number(None))
    }
    fn visit_str<E>(self, v: &str) -> Result<Json, E> {
        Ok(Json::String(v.to_owned()))
    }
    fn visit_string<E>(self, v: String) -> Result<Json, E> {
        Ok(Json::String(v))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Json, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Json::Array(items))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Json, A::Error> {
        let mut pairs: Vec<(String, Json)> = Vec::new();
        while let Some(key) = map.next_key::<String>()? {
            if pairs.iter().any(|(k, _)| *k == key) {
                return Err(de::Error::custom(format_args!(
                    "duplicate member \"{key}\""
                )));
            }
            let value = map.next_value()?;
            pairs.push((key, value));
        }
        Ok(Json::Object(pairs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = r#"{
      "version": 1,
      "call": "Q1ABC",
      "sources": [
        { "name": "Home node", "node": "dx.example.net:7300", "selected": true },
        { "name": "RBN CW", "node": "rbn.example.net:7000", "profile": "rbn" }
      ]
    }"#;

    fn with_sources(sources: &str) -> String {
        format!(r#"{{"version":1,"call":"Q1ABC","sources":[{sources}]}}"#)
    }

    #[test]
    fn sources_file_reads_the_documented_example_in_order() {
        // QGDXC-TUI-0007
        let saved = parse(EXAMPLE.as_bytes()).unwrap();
        assert_eq!(saved.call.as_deref(), Some("Q1ABC"));
        let names: Vec<_> = saved.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Home node", "RBN CW"]);
        let first = &saved.entries[0];
        assert_eq!(
            (first.config.as_ref().unwrap().profile, first.selected),
            (Profile::Auto, true)
        );
        assert_eq!(
            (
                first.config.as_ref().unwrap().host.as_str(),
                first.config.as_ref().unwrap().port
            ),
            ("dx.example.net", 7300)
        );
        assert_eq!(
            (
                saved.entries[1].config.as_ref().unwrap().profile,
                saved.entries[1].selected
            ),
            (Profile::Rbn, false)
        );
        assert!(
            saved
                .entries
                .iter()
                .all(|e| !e.config.as_ref().unwrap().retain_third_party_data)
        );
        let ipv6 = with_sources(r#"{"name":"v6","node":"[2001:db8::1]:7300"}"#);
        assert_eq!(
            parse(ipv6.as_bytes()).unwrap().entries[0]
                .config
                .as_ref()
                .unwrap()
                .host,
            "2001:db8::1"
        );
    }

    #[test]
    fn sources_file_rejects_the_whole_file_naming_the_offending_value() {
        // QGDXC-TUI-0007
        let source = |member: &str| {
            with_sources(&format!(
                r#"{{"name":"A","node":"dx.example.net:1"{member}}}"#
            ))
        };
        let many = with_sources(
            &(0..=MAX_SOURCES)
                .map(|i| format!(r#"{{"name":"n{i}","node":"dx.example.net:{}"}}"#, i + 1))
                .collect::<Vec<_>>()
                .join(","),
        );
        let cases: Vec<(String, &str)> = vec![
            ("{".into(), "invalid JSON"),
            ("[]".into(), "the file: must be an object"),
            (
                r#"{"version":1,"call":"Q1ABC"}"#.into(),
                "missing \"sources\"",
            ),
            (
                r#"{"version":1,"sources":[{"name":"A","node":"dx.example.net:1"}]}"#.into(),
                "sources[0]: missing \"call\"",
            ),
            (
                source(r#","call":"Q1ABC-0""#),
                "sources[0].call: not a valid callsign",
            ),
            (
                source(r#","call":"Q1ABC\n""#),
                "sources[0].call: not a valid callsign",
            ),
            (
                source(r#","call":null"#),
                "sources[0].call: must be a string",
            ),
            (
                EXAMPLE.replace("\"version\": 1", "\"version\": 2"),
                "version: must be 1",
            ),
            (
                EXAMPLE.replace("\"version\": 1", "\"version\": \"1\""),
                "version: must be 1",
            ),
            (
                EXAMPLE.replace("\"version\": 1", "\"version\": 1.0"),
                "version: must be 1",
            ),
            (
                EXAMPLE.replace("\"version\": 1", "\"version\": -1"),
                "version: must be 1",
            ),
            (
                EXAMPLE.replace("Q1ABC", "Q1ABC-0"),
                "call: not a valid callsign",
            ),
            (EXAMPLE.replace("\"Q1ABC\"", "7"), "call: must be a string"),
            (
                EXAMPLE.replace("\"version\": 1,", "\"version\": 1, \"theme\": \"mono\","),
                "unexpected member \"theme\"",
            ),
            (
                EXAMPLE.replace("\"version\": 1,", "\"version\": 1, \"version\": 1,"),
                "duplicate member \"version\"",
            ),
            (with_sources(""), "sources: must hold 1 to 64 sources"),
            (many, "sources: must hold 1 to 64 sources"),
            (
                r#"{"version":1,"call":"Q1ABC","sources":{}}"#.into(),
                "sources: must be an array",
            ),
            (with_sources("[]"), "sources[0]: must be an object"),
            (
                with_sources(r#"{"node":"dx.example.net:1"}"#),
                "sources[0]: missing \"name\"",
            ),
            (
                source(r#","password":"x""#),
                "sources[0]: unexpected member \"password\"",
            ),
            (source(r#","name":"B""#), "duplicate member \"name\""),
            (
                with_sources(&format!(
                    r#"{{"name":"{}","node":"dx.example.net:1"}}"#,
                    "N".repeat(33)
                )),
                "sources[0].name",
            ),
            (
                with_sources(r#"{"name":"","node":"dx.example.net:1"}"#),
                "sources[0].name",
            ),
            (
                with_sources(r#"{"name":" A","node":"dx.example.net:1"}"#),
                "sources[0].name",
            ),
            (
                with_sources(r#"{"name":"A\tB","node":"dx.example.net:1"}"#),
                "sources[0].name",
            ),
            (
                with_sources(r#"{"name":"Café","node":"dx.example.net:1"}"#),
                "sources[0].name",
            ),
            (
                with_sources(
                    r#"{"name":"A","node":"dx.example.net:1"},{"name":"a","node":"dx.example.net:2"}"#,
                ),
                "sources[1].name: duplicate name",
            ),
            (
                with_sources(
                    r#"{"name":"A","node":"dx.example.net:1"},{"name":"B","node":"DX.example.net:1"}"#,
                ),
                "sources[1].node: duplicate endpoint",
            ),
            (
                with_sources(r#"{"name":"A","node":"dx.example.net:1/rbn"}"#),
                "sources[0].node: give the profile",
            ),
            (
                with_sources(r#"{"name":"A","node":"bad host:1"}"#),
                "sources[0].node: invalid host",
            ),
            (
                with_sources(r#"{"name":"A","node":"dx.example.net:0"}"#),
                "sources[0].node: port must be 1 to 65535",
            ),
            (
                with_sources(r#"{"name":"A","node":"2001:db8::1:7300"}"#),
                "sources[0].node: IPv6 requires brackets",
            ),
            (
                with_sources(r#"{"name":"A","node":7300}"#),
                "sources[0].node: must be a string",
            ),
            (
                source(r#","profile":"unknown""#),
                "sources[0].profile: unsupported profile",
            ),
            (
                source(r#","profile":"ssh""#),
                "sources[0].profile: unsupported profile",
            ),
            (
                source(r#","selected":"yes""#),
                "sources[0].selected: must be true or false",
            ),
            (
                source(r#","selected":null"#),
                "sources[0].selected: must be true or false",
            ),
            (
                format!("{EXAMPLE}{}", " ".repeat(MAX_BYTES)),
                "larger than 65536 bytes",
            ),
        ];
        for (text, expected) in cases {
            let error = parse(text.as_bytes()).unwrap_err();
            assert!(error.contains(expected), "{expected:?} not in {error:?}");
        }
        assert!(parse(b"\xff").unwrap_err().contains("invalid JSON"));
        let deep = format!("{}{}", "[".repeat(200), "]".repeat(200));
        assert!(parse(deep.as_bytes()).is_err());
    }

    #[test]
    fn sources_file_call_per_source_overrides_the_file_call() {
        // QGDXC-TUI-0007
        let text = with_sources(
            r#"{"name":"A","node":"dx.example.net:1"},{"name":"B","node":"dx.example.net:2","call":"Q1ABC-2"}"#,
        );
        let saved = parse(text.as_bytes()).unwrap();
        let calls: Vec<_> = saved
            .entries
            .iter()
            .map(|e| e.config.as_ref().unwrap().call.as_str())
            .collect();
        assert_eq!(calls, ["Q1ABC", "Q1ABC-2"]);
        let text =
            r#"{"version":1,"sources":[{"name":"A","node":"dx.example.net:1","call":"Q9XYZ"}]}"#;
        let saved = parse(text.as_bytes()).unwrap();
        assert_eq!(
            (
                saved.call,
                saved.entries[0].config.as_ref().unwrap().call.as_str()
            ),
            (None, "Q9XYZ")
        );
    }

    #[test]
    fn sources_file_default_path_follows_the_platform() {
        // QGDXC-TUI-0006
        let root = std::env::temp_dir();
        let env = |pairs: Vec<(&'static str, OsString)>| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| v.clone())
            }
        };
        let tail = Path::new("qso-graph").join("dxcluster-sources.json");
        let xdg = root.join("xdg");
        let home = root.join("home");
        assert_eq!(
            default_path(
                false,
                env(vec![
                    ("XDG_CONFIG_HOME", xdg.clone().into()),
                    ("HOME", home.clone().into())
                ])
            )
            .unwrap(),
            xdg.join(&tail)
        );
        for xdg in ["", "relative/dir"] {
            assert_eq!(
                default_path(
                    false,
                    env(vec![
                        ("XDG_CONFIG_HOME", xdg.into()),
                        ("HOME", home.clone().into())
                    ])
                )
                .unwrap(),
                home.join(".config").join(&tail)
            );
        }
        for home in [None, Some(OsString::new()), Some("relative".into())] {
            let pairs = home.map(|h| vec![("HOME", h)]).unwrap_or_default();
            assert!(
                default_path(false, env(pairs))
                    .unwrap_err()
                    .contains("HOME")
            );
        }
        let appdata = root.join("appdata");
        assert_eq!(
            default_path(
                true,
                env(vec![
                    ("APPDATA", appdata.clone().into()),
                    ("HOME", home.clone().into())
                ])
            )
            .unwrap(),
            appdata.join(&tail)
        );
        assert!(
            default_path(true, env(vec![("HOME", home.into())]))
                .unwrap_err()
                .contains("APPDATA")
        );
    }

    #[test]
    fn sources_file_reads_only_a_bounded_regular_file_and_never_writes() {
        // QGDXC-TUI-0006 QGDXC-TUI-0009
        let dir = std::env::temp_dir().join(format!("qg-dxc-sources-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("sources.json");
        assert!(read(&file).is_err(), "missing file accepted");
        assert!(read(&dir).unwrap_err().contains("regular file"));
        // Exactly the limit is accepted; one byte more is refused.
        let mut text = EXAMPLE.to_owned();
        text.push_str(&" ".repeat(MAX_BYTES - text.len()));
        std::fs::write(&file, &text).unwrap();
        let before = std::fs::metadata(&file).unwrap().modified().unwrap();
        assert_eq!(read(&file).unwrap().entries.len(), 2);
        text.push(' ');
        std::fs::write(&file, &text).unwrap();
        assert!(read(&file).unwrap_err().contains("larger than"));
        std::fs::write(&file, EXAMPLE).unwrap();
        let modified = std::fs::metadata(&file).unwrap().modified().unwrap();
        read(&file).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), EXAMPLE);
        assert_eq!(
            std::fs::metadata(&file).unwrap().modified().unwrap(),
            modified
        );
        assert!(modified >= before);
        let listing: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(listing, ["sources.json"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
