// SPDX-License-Identifier: GPL-3.0-or-later
use super::model::{DEFAULT_KEEP, MAX_KEEP, MIN_KEEP, clip, sanitize};
use super::theme::Theme;
use qso_graph_dxcluster::Config;
use qso_graph_dxcluster_cli::saved::{self, NodeError, Saved};
use std::{collections::BTreeSet, path::Path};

#[derive(Debug, Clone)]
pub enum Source {
    Node(Config),
    Replay(String),
}
pub struct Options {
    pub theme: Theme,
    pub keep_human: usize,
    pub keep_skimmer: usize,
    pub sources: Vec<Source>,
    pub labels: Vec<String>,
    /// The operator's sources file, when no source was given (QGDXC-TUI-0006).
    pub saved: Option<Saved>,
}
impl Options {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut theme = None;
        let mut keep_human = None;
        let mut keep_skimmer = None;
        let mut nodes = vec![];
        let mut replays = vec![];
        let mut call = None;
        let mut sources_file = None;
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
                "--sources" if sources_file.is_none() => sources_file = Some(value.clone()),
                "--keep-human" if keep_human.is_none() => {
                    keep_human = Some(keep(value, flag)?);
                }
                "--keep-skimmer" if keep_skimmer.is_none() => {
                    keep_skimmer = Some(keep(value, flag)?);
                }
                _ => return Err(format!("configuration: unexpected argument {flag}")),
            }
            i += 2;
        }
        let saved = if nodes.is_empty() && replays.is_empty() {
            if call.is_some() {
                return Err("configuration: --call requires --node".into());
            }
            Some(read_saved(sources_file.as_deref())?)
        } else if sources_file.is_some() {
            return Err(
                "configuration: --sources cannot be combined with --node or --replay".into(),
            );
        } else {
            None
        };
        if saved.is_none() && nodes.len() + replays.len() > 8 {
            return Err("configuration: --node/--replay requires 1 to 8 sources".into());
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
                let profile =
                    saved::profile(profile).ok_or_else(|| error("unsupported profile"))?;
                let (config, host) =
                    saved::node(endpoint, profile, &call).map_err(|e| match e {
                        NodeError::Endpoint(why) => error(why),
                        NodeError::Config(why) => format!("{}; --call {call}", error(&why)),
                    })?;
                if !endpoints.insert((host.to_ascii_lowercase(), config.port)) {
                    return Err(error("duplicate endpoint"));
                }
                bases.push(host);
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
            keep_human: keep_human.unwrap_or(DEFAULT_KEEP),
            keep_skimmer: keep_skimmer.unwrap_or(DEFAULT_KEEP),
            theme: Theme::initial(
                theme,
                std::env::var_os("NO_COLOR")
                    .map(|s| s.to_string_lossy().into_owned())
                    .as_deref(),
            ),
            sources,
            labels: labels(&bases),
            saved,
        })
    }
}
impl Options {
    /// Run the chosen saved sources, in file order, as the equivalent `--node`
    /// arguments would; each label's base is the source's name (QGDXC-TUI-0008).
    pub fn choose(&mut self, chosen: &[usize]) {
        let Some(saved) = &self.saved else { return };
        let entries: Vec<_> = chosen
            .iter()
            .filter_map(|&i| saved.entries.get(i))
            .collect();
        self.sources = entries
            .iter()
            .map(|e| Source::Node(e.config.clone()))
            .collect();
        self.labels = labels(&entries.iter().map(|e| e.name.clone()).collect::<Vec<_>>());
    }
}
fn read_saved(explicit: Option<&str>) -> Result<Saved, String> {
    let path = match explicit {
        Some(path) => path.into(),
        None => saved::default_path(cfg!(windows), |name| std::env::var_os(name))
            .map_err(|e| format!("configuration: {e}"))?,
    };
    saved::read(&path).map_err(|e| format!("configuration: sources file {}: {e}", path.display()))
}
fn keep(value: &str, flag: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .ok()
        .filter(|n| (MIN_KEEP..=MAX_KEEP).contains(n))
        .ok_or_else(|| {
            format!("configuration: {flag} must be an integer in {MIN_KEEP}..={MAX_KEEP}")
        })
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
