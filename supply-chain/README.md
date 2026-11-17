<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Dependency review store

The project-owned configuration, empty local audit store, and pinned imported
store retain SPDX comments. Cargo-vet requires canonical generated comments;
`cargo xtask vet` copies these three files to target/verify/vet-store, normalizes
that temporary copy with cargo-vet fmt, and runs cargo vet --locked --frozen
against it. The committed files are unchanged, and verification stays offline.

Imported audits retain upstream attribution. Exact-version exemptions explicitly
mean not audited yet. See [docs/DEPENDENCIES.md](../docs/DEPENDENCIES.md).
