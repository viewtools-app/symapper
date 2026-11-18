#!/usr/bin/env bash
# Deterministic task mining for the public benchmark. See bench/README.md for the rule.
# Usage: select-tasks.sh <repos-dir> > tasks.tsv   (repos from fetch-repos.sh)
set -euo pipefail
set -f
export LC_ALL=C GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
here=$(cd "$(dirname "$0")" && pwd -P)
[ "$#" -eq 1 ] && [ -d "$1" ] || { echo 'Usage: select-tasks.sh <repos-dir>' >&2; exit 2; }
repos=$(cd "$1" && pwd -P)
per_repo=${PER_REPO:-3}

is_source() { [[ $1 =~ \.(rs|go|ts|tsx|js|mjs|py|swift)$ && ! $1 =~ \.d\.ts$ ]]; }
# Same test-file rule as scripts/bench-recall.sh.
is_test() {
    local base=${1##*/}
    [[ $1 =~ (^|/)(tests|__tests__)/ || $base =~ (test|spec) ]]
}
# Drop a conventional prefix, #123 references and punctuation; collapse spaces.
make_focus() {
    printf '%s\n' "$1" | sed -E \
        -e 's/^[A-Za-z]+(\([^)]*\))?!?:[[:space:]]*//' \
        -e 's/#[0-9]+//g' \
        -e 's/[^A-Za-z0-9_]+/ /g' |
        awk '{ $1=$1; print }'
}

printf 'id\trepo\tcommit\tfocus\ttruth\tnew\tk_base\tset\n'
while IFS=$'\t' read -r name _url sha _lang; do
    case "$name" in '#'*|'') continue ;; esac
    git_r() { git -C "$repos/$name" -c core.quotePath=false "$@"; }
    found=0
    while IFS=$'\t' read -r commit subject; do
        [ "$found" -lt "$per_repo" ] || break
        git_r rev-parse -q --verify "$commit^" >/dev/null || continue
        n=0 ok=0
        while IFS= read -r path; do
            is_source "$path" || continue
            n=$((n + 1))
            if ! is_test "$path" && git_r cat-file -e "$commit^:$path" 2>/dev/null; then ok=1; fi
        done < <(git_r diff --no-ext-diff --no-textconv --name-only "$commit^" "$commit" --)
        { [ "$n" -ge 1 ] && [ "$n" -le 6 ] && [ "$ok" -eq 1 ]; } || continue
        focus=$(make_focus "$subject")
        [ -n "$focus" ] || continue
        found=$((found + 1))
        printf '%s-%s\t%s\t%s\t%s\t\t\t\tpublic\n' "$name" "${commit:0:7}" "$name" "${commit:0:12}" "$focus"
    done < <(git_r log --first-parent --format='%H%x09%s' "$sha" |
        awk -F '\t' '{ s=tolower($2) } s ~ /(^|[^a-z0-9_])fix([^a-z0-9_]|$)/')
    echo "select-tasks: $name $found task(s)" >&2
done < "$here/repos.tsv"
