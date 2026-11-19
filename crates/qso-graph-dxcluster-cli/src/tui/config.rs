// SPDX-License-Identifier: GPL-3.0-or-later
use super::model::{clip, sanitize};
use super::theme::Theme;
use qso_graph_dxcluster::{Config, Profile};
use std::{collections::BTreeSet, net::Ipv6Addr, path::Path};

#[derive(Debug, Clone)]
pub enum Source {
    Node(Config),
    Replay(String),
}
pub struct Options {
    pub theme: Theme,
    pub sources: Vec<Source>,
    pub labels: Vec<String>,
}
impl Options {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut theme = None;
        let mut nodes = vec![];
        let mut replays = vec![];
        let mut call = None;
        let mut i = 0;
        while i < args.len() {
            let flag = &args[i];
            let value = args
                .get(i + 1)
                .filter(|s| !s.starts_with("--"))
                .ok_or_else(|| format!("configuration: missing value for {flag}"))?;
            match flag.as_str() {
                "--theme" if theme.is_none() => {
                    theme = Some(
                        Theme::parse(value)
                            .ok_or_else(|| format!("configuration: invalid --theme {value}"))?,
                    );
                }
                "--node" => nodes.push(value.clone()),
                "--replay" => replays.push(value.clone()),
                "--call" if call.is_none() => call = Some(value.clone()),
                _ => return Err(format!("configuration: unexpected argument {flag}")),
            }
            i += 2;
        }
        if nodes.len() + replays.len() == 0 || nodes.len() + replays.len() > 8 {
            return Err("configuration: --node/--replay requires 1–8 sources".into());
        }
        if !nodes.is_empty() && !replays.is_empty() {
            return Err("configuration: --node cannot be combined with --replay".into());
        }
        let mut sources = vec![];
        let mut bases = vec![];
        let mut endpoints = BTreeSet::new();
        if !nodes.is_empty() {
            let call = call.ok_or("configuration: --call is required with --node")?;
            for arg in nodes {
                let error = |why: &str| format!("configuration: --node {arg}: {why}");
                let (endpoint, profile) = arg.split_once('/').unwrap_or((&arg, "auto"));
                let profile = Profile::parse(profile)
                    .filter(|_| profile != "unknown")
                    .ok_or_else(|| error("unsupported profile"))?;
                let (host, port) = if let Some(rest) = endpoint.strip_prefix('[') {
                    let (host, port) = rest
                        .split_once("]:")
                        .ok_or_else(|| error("expected [address]:port"))?;
                    host.parse::<Ipv6Addr>()
                        .map_err(|_| error("invalid IPv6 literal"))?;
                    (host, port)
                } else {
                    let (host, port) = endpoint
                        .split_once(':')
                        .ok_or_else(|| error("expected host:port"))?;
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
                    return Err(error("port must be 1–65535"));
                }
                let port = port
                    .parse::<u16>()
                    .ok()
                    .filter(|p| *p != 0)
                    .ok_or_else(|| error("port must be 1–65535"))?;
                let mut config = Config::new(host.into(), port, call.clone(), profile);
                config.retain_third_party_data = false;
                config
                    .validate()
                    .map_err(|e| format!("{}; --call {call}", error(e)))?;
                if !endpoints.insert((host.to_ascii_lowercase(), port)) {
                    return Err(error("duplicate endpoint"));
                }
                bases.push(host.to_owned());
                sources.push(Source::Node(config));
            }
        } else {
            if call.is_some() {
                return Err("configuration: --call requires --node".into());
            }
            for path in replays {
                // QGDXC-TUI-0003: metadata follows symlinks; never open special files.
                let metadata = std::fs::metadata(&path)
                    .map_err(|e| format!("configuration: --replay {path}: {e}"))?;
                if !metadata.is_file() {
                    return Err(format!(
                        "configuration: --replay {path}: expected a regular file"
                    ));
                }
                bases.push(
                    Path::new(&path)
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| "source".into()),
                );
                sources.push(Source::Replay(path));
            }
        }
        Ok(Self {
            theme: Theme::initial(
                theme,
                std::env::var_os("NO_COLOR")
                    .map(|s| s.to_string_lossy().into_owned())
                    .as_deref(),
            ),
            sources,
            labels: labels(&bases),
        })
    }
}
pub fn labels(bases: &[String]) -> Vec<String> {
    let mut used = BTreeSet::new();
    bases
        .iter()
        .map(|base| {
            let base = sanitize(base);
            let mut label = clip(&base, 16, false);
            let mut suffix = 2;
            while used.contains(&label) {
                let tail = format!("#{suffix}");
                label = format!("{}{}", clip(&base, 16 - tail.len(), false), tail);
                suffix += 1;
            }
            used.insert(label.clone());
            label
        })
        .collect()
}
