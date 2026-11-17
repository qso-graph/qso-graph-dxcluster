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
fuzz smoke, and unchanged lockfiles. Verification sets Cargo offline and freezes
the cargo-vet import store. Install and cache dependencies before verifying:

```sh
cargo fetch --locked
cargo install cargo-deny --version 0.20.2 --locked
cargo install cargo-vet --version 0.10.3 --locked
```

If cargo-fuzz is absent, its row says SKIP and gives the installation command.
All other missing verification tools fail their row. Fuzz targets and their
build instructions are in [fuzz/README.md](fuzz/README.md).

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
