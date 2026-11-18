<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Release QG DX Cluster

KI7MT runs `.github/workflows/release.yml` manually from `main`. The workflow
checks the source, builds and signs the downloads, publishes the library and CLI
in that order, tags only after crates.io serves both packages with matching
checksums, and creates the GitHub Release last. Merging publishes nothing.

## First release setup

1. Protect `main` with pull requests and the required `ci-all-green` check.
   Create the `macos-signing`, `windows-signing`, and `crates-io` GitHub
   Environments in `qso-graph/qso-graph-dxcluster`. Restrict each to `main`.
2. Give this repository access to the six Apple organization secrets:
   `CSC_LINK`, `CSC_KEY_PASSWORD`, `APPLE_API_KEY_P8`, `APPLE_API_KEY_ID`,
   `APPLE_API_ISSUER`, and `APPLE_TEAM_ID`. Alternatively, store them in
   `macos-signing`. The certificate must be the Developer ID Application
   certificate for Greg Beam; the notarization key must belong to its team.
3. Store `AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, and `AZURE_SUBSCRIPTION_ID` in
   `windows-signing`. Configure the signing application's Azure federated
   credential with issuer `https://token.actions.githubusercontent.com`,
   audience `api://AzureADTokenExchange`, and this exact subject:

   ```text
   repo:qso-graph@265651030/qso-graph-dxcluster@1460849467:environment:windows-signing
   ```

   The repository uses GitHub's [immutable OIDC subject format](https://docs.github.com/en/actions/reference/security/oidc#immutable-subject-claims).
   Its owner ID, repository ID, and subject prefix were confirmed through the
   repository and Actions OIDC configuration APIs. The pinned shared workflow
   uses signing account `qsographsigning`, certificate profile `qso-graph`, and
   endpoint `https://wus2.codesigning.azure.net`. The Azure application needs
   permission to sign with that profile. All nine signing secret names pass
   explicitly through the release and binaries workflows to the appropriate
   shared signing workflow. Publication credentials never reach build jobs.
4. In the `crates-io` Environment, set `CRATES_IO_FIRST_PUBLISH` to a short-lived
   crates.io API token with permission to create `qso-graph-dxcluster` and
   `qso-graph-dxcluster-cli`. The workflow uses it only for crates that do not yet
   exist. Once created, configure [Trusted Publishing](https://crates.io/docs/trusted-publishing)
   on **each** crate for owner `qso-graph`, repository `qso-graph-dxcluster`,
   workflow `release.yml`, and Environment `crates-io`. Delete the first-publish
   secret. Later releases refuse a leftover secret when both crates exist.
   If the first run stops after creating only one crate, configure Trusted
   Publishing for that crate before rerunning; the token serves only the
   remaining new crate.
5. Ensure Actions policy permits the pinned actions and reusable workflows in
   `qso-graph/.github`. The signing workflows are pinned to
   `ce98621da707d49c828805d92f3117b6bf282cd5`, as in the reference release.

## Run a release

1. Prepare a pull request with the workspace version, CLI library dependency,
   lockfiles, and one `## [<version>]` changelog section in agreement. Versions
   are `X.Y.Z` or `X.Y.Z-beta.N`. Refresh license copies with
   `uv run --no-project python scripts/package-files.py`, then run
   `cargo xtask verify`.
2. Merge after CI passes. Release checks require a successful same-repository
   pull request CI run whose source tree exactly matches `main`.
3. Open **Actions > Release > Run workflow**, select `main`, enter the manifest
   version, and keep `dry_run` checked. Inspect the four platform artifacts:
   four CLI archives and four Linux packages. This run signs, notarizes,
   assembles, and attests without publishing crates, a tag, or a GitHub Release.
4. Run the same version with `dry_run` unchecked. The release adds
   `qgdxc-spec-<version>.tar.gz`, containing `spec/` from the release commit, and
   `SHA256SUMS.txt` covering all nine downloads. Both added files are attested.
   The resulting GitHub Release has ten assets. Betas are prereleases.

Downloads target macOS Apple silicon, Linux x86_64 and arm64 with static musl,
and Windows x86_64 with a static CRT. Builds reject Linux dynamic linking,
non-system macOS libraries, and Windows DLLs outside the explicit system allowlist.
Linux packages are named `qg-dxc`; each architecture has a `.deb` and an `.rpm`.

A stopped release can be rerun. An existing crates.io version must have the same
package checksum, and an existing tag must point to the same commit. Existing
GitHub Release assets are replaced by name. The source and registry checks are
in `scripts/release_check.py`, with refusal cases in `scripts/test_release_check.py`.
