#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Write the CHANGELOG.md section for VERSION from the commits since the latest release tag.
# Run in the release pull request; `cog bump --auto --dry-run` suggests VERSION.
set -euo pipefail
version=${1:-}
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$ ]] || { echo "usage: $0 X.Y.Z[-pre]" >&2; exit 2; }
cd "$(git rev-parse --show-toplevel)"
file=CHANGELOG.md
grep -qF "## [$version]" "$file" && { echo "$file already has [$version]" >&2; exit 1; }
tag=$(git describe --tags --abbrev=0 --match 'v[0-9]*')
notes=$(cog changelog "$tag..HEAD")
[[ -n ${notes//[[:space:]]/} ]] || { echo "no feat, fix, perf, revert or docs commits since $tag" >&2; exit 1; }
tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT
NOTES="$notes" VERSION="$version" awk '
  !done && /^## \[/ { print "## [" ENVIRON["VERSION"] "]\n\n" ENVIRON["NOTES"] "\n"; done = 1 }
  { print }
  END { if (!done) exit 3 }
' "$file" >"$tmp" || { echo "$file has no release section to insert above" >&2; exit 1; }
cat "$tmp" >"$file"
echo "wrote [$version] from $tag..HEAD"
