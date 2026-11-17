<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Changelog

## Unreleased 0.1.0

- Request readiness tokens and retry jitter per attempt, using fresh OS randomness
  in the CLI and constant engine storage (QGDXC-DSP-0004, QGDXC-SES-0010).

- Add the dependency-free receive-only library, bounded framing, passive Telnet
  stripping, all four server profiles, and automatic profile detection.
- Add caller-driven session timing, paced startup, retry budgets, and privacy
  transformations with explicit evidence status.
- Add `qg-dxc connect` and offline `replay`, Annex A JSON output, terminal escaping,
  optional exact recording, and Ctrl-C shutdown.
- Add independently specified sanitized vectors, requirement coverage checks,
  source policy, offline verification, and a three-platform CI matrix.
- Clarify AR-Cluster banner extension context, terminal notices during EOF
  finalization, synthetic privacy-test addresses, vector license metadata, and local configuration-rejection vectors.
