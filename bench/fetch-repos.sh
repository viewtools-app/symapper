#!/usr/bin/env bash
# Full (not blobless) clones: the harness clones with --shared, which cannot lazy-fetch blobs.
# Clone the pinned benchmark repos into <dir>/<repo>. Never runs repo code or hooks.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd -P)
[ "$#" -eq 1 ] || { echo 'Usage: fetch-repos.sh <dir>' >&2; exit 2; }
mkdir -p "$1"
dest=$(cd "$1" && pwd -P)
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 GIT_TERMINAL_PROMPT=0
while IFS=$'\t' read -r name url sha _lang; do
    case "$name" in '#'*|'') continue ;; esac
    [[ $name =~ ^[A-Za-z0-9_.-]+$ && $sha =~ ^[0-9a-f]{40}$ ]] || { echo "bad row: $name" >&2; exit 2; }
    if [ -d "$dest/$name/.git" ]; then
        echo "skip $name (exists)"
    else
        echo "clone $name"
        git -c core.hooksPath=/dev/null clone -q --no-checkout "$url" "$dest/$name"
    fi
    git -C "$dest/$name" -c core.hooksPath=/dev/null -c advice.detachedHead=false checkout -q "$sha"
done < "$here/repos.tsv"
