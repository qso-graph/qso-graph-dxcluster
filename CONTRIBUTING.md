<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Contributing

Follow [the repository instructions](AGENTS.md), American English, and the
[Google developer documentation style guide](https://developers.google.com/style).
Protocol changes cite the requirement ID and pinned evidence in [spec/](spec/).

Run every check from the repository root:

```sh
cargo xtask verify
```

The command prints one pass/fail table, retains diagnostics in `target/verify/`,
and returns nonzero if any check fails. It checks formatting, Clippy, workspace
tests, complete vector traces and byte-split invariance, requirement coverage,
published-data sanitization, source policy, cargo-deny, cargo-vet, documentation,
fuzz smoke, and unchanged lockfiles. Its first row fetches both locked dependency
graphs and the cargo-audit advisory database. Every subsequent row runs offline,
including cargo-audit with --no-fetch --deny warnings and the frozen cargo-vet
store. Install the pinned tools before verification:

```sh
cargo install cargo-deny --version 0.20.2 --locked
cargo install cargo-vet --version 0.10.3 --locked
cargo install cargo-audit --version 0.22.2 --locked
```

Linux requires the pinned fuzz tools in [fuzz/README.md](fuzz/README.md); missing
prerequisites fail verification. Other platforms skip with a stated reason.
CI installs those tools, then runs exactly `cargo xtask verify`.

Regenerate evidence with a private capture directory supplied explicitly:

```sh
cargo xtask gen-vectors --captures /path/to/private/captures
cargo xtask vectors
cargo xtask sanitization
```

Generation sorts input files and identity tokens, records source record numbers,
and is deterministic and idempotent. It never writes raw captures or the reverse
identity map to the repository. Expected events come from the independent
[oracle](xtask/src/oracle.rs); session expectations are declarative
[state-table traces](xtask/src/scenarios.rs). Neither imports implementation code.
The runner alone imports the library. Add a small independent oracle for each
protocol regression, and review the expected values before committing it.
See the hand-checked sample in [docs/CONFORMANCE.md](docs/CONFORMANCE.md).

Source policy and privacy checks include planted failures. Add a planted failure
when extending a policy check. Tests and verification use local data only; they
do not contact cluster or RBN nodes. New dependencies are reviewed in
[docs/DEPENDENCIES.md](docs/DEPENDENCIES.md). Cargo-vet exemptions are review
backlog entries, not completed audits. No release workflow is part of 0.1.0.
