// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
mod checks;
mod generate;
mod oracle;
mod runner;
mod scenarios;
mod verify;
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("gen-vectors") if args.iter().any(|a| a == "--captures") => generate::generate(
            args.iter()
                .position(|a| a == "--captures")
                .and_then(|i| args.get(i + 1))
                .map(std::path::Path::new),
        ),
        Some("vectors") => runner::all(),
        Some("coverage") => checks::coverage(),
        Some("sanitization") => checks::sanitization(),
        Some("policy") => checks::policy(),
        Some("docs") => checks::docs(),
        Some("fetch") => verify::fetch(),
        Some("audit") => verify::audit(),
        Some("verify") => verify::verify(),
        Some("vet") => checks::vet(),
        Some("seed-fuzz") => checks::seed_fuzz(),
        _ => Err("usage: cargo xtask gen-vectors --captures <dir> | vectors".into()),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
