#!/usr/bin/env bash
# Prints the GitHub Release notes for a tag such as v0.1.3: dist/release-notes.md with the
# version filled in and, in place of its @CHANGES@ line, the "## 0.1.3" section of CHANGELOG.md
# under a "What's new" heading (nothing when the changelog has no section for the version).
# Run from the repository root: dist/release-notes.sh v0.1.3
set -euo pipefail

tag=${1:?usage: dist/release-notes.sh <tag>}
version=${tag#v}

changes=$(awk -v head="## $version" '
    $0 == head { found = 1; next }
    /^## /     { found = 0 }
    found
' CHANGELOG.md)

whats_new=$(mktemp)
trap 'rm -f "$whats_new"' EXIT
if [ -n "$(printf '%s' "$changes" | tr -d '[:space:]')" ]; then
    printf "## What's new in %s\n%s\n" "$version" "$changes" > "$whats_new"
fi

sed "s/@VERSION@/$version/g; s/@TAG@/$tag/g" dist/release-notes.md |
    sed -e "/^@CHANGES@\$/{r $whats_new" -e 'd}'
