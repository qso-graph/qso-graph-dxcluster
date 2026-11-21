#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Check scripts/changelog-release.sh against a scratch repository: the section it writes,
# what it leaves out, and what it refuses.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }
[[ $(cog --version) == "cog 7.0.0" ]] || fail "need cog 7.0.0, found: $(cog --version 2>&1)"
cd "$work"
git init -q -b main
git config user.name Q1ABC
git config user.email Q1ABC
git config commit.gpgsign false
git config tag.gpgsign false
mkdir scripts
cp "$root/cog.toml" .
cp "$root/scripts/changelog.tera" "$root/scripts/changelog-release.sh" scripts/
printf '# Changelog\n\n## [0.1.0]\n\n- First.\n' >CHANGELOG.md
git add -A && git commit -qm 'Initial import' && git tag v0.1.0
commit() { git commit -q --allow-empty -m "$1"; }
commit 'feat(tui): show the band plan'
commit 'fix: retry once after a reset'
commit 'ci: pin cog'
commit 'docs: explain the limits editor'
commit 'feat!: drop the old replay format'

bash scripts/changelog-release.sh 0.2.0 >/dev/null
expected='# Changelog

## [0.2.0]

### Features

- **Breaking:** drop the old replay format
- tui: show the band plan

### Bug Fixes

- retry once after a reset

### Documentation

- explain the limits editor

## [0.1.0]

- First.'
[[ $(cat CHANGELOG.md) == "$expected" ]] || { diff <(echo "$expected") CHANGELOG.md >&2; fail "section differs"; }

refused() { if bash scripts/changelog-release.sh "$@" >/dev/null 2>&1; then fail "accepted: $*"; fi; }
refused 0.2.0                 # already written
refused v0.3.0                # not X.Y.Z
refused '0.3.0;id'            # not X.Y.Z
git tag v0.2.0 && commit 'ci: nothing for users'
refused 0.2.1                 # no user-facing commits
commit 'Add a thing'
cog check >/dev/null 2>&1 && fail "cog check accepted a non-conventional commit"
echo "changelog release script: section written; 4 refusals; non-conventional commit rejected"
