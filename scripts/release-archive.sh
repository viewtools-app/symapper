#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 4 ]]; then
  echo "usage: release-archive.sh <triple> <binary> <version> <outdir>" >&2
  exit 2
fi
root=$(cd "$(dirname "$0")/.." && pwd)
triple=$1
binary=$2
version=$3
out=$4
case "$triple" in
  aarch64-apple-darwin|x86_64-apple-darwin|x86_64-unknown-linux-musl|aarch64-unknown-linux-musl) ;;
  *) echo "unknown target: $triple" >&2; exit 2 ;;
esac
cargo_version=$(sed -n '/^\[package\]$/,/^\[/p' "$root/Cargo.toml" |
  sed -n 's/^version *= *"\([^"]*\)".*/\1/p')
if [[ "$version" != "$cargo_version" ]]; then
  echo "version mismatch: expected $cargo_version, got $version" >&2
  exit 2
fi

mkdir -p "$out/dist/$triple"
out=$(cd "$out" && pwd)
tmp=$(mktemp -d)
trap 'rm -r "$tmp"' EXIT
name="symapper-$version-$triple"
mkdir "$tmp/$name"
cp "$binary" "$tmp/$name/symapper"
chmod 0755 "$tmp/$name/symapper"
cp "$root/LICENSE" "$root/NOTICE" "$root/THIRD-PARTY-LICENSES" "$root/README.md" "$tmp/$name/"
# List members explicitly in lexical order on both BSD tar and GNU tar.
COPYFILE_DISABLE=1 tar -cf - -C "$tmp" --no-recursion "$name/" "$name/LICENSE" "$name/NOTICE" \
  "$name/README.md" "$name/THIRD-PARTY-LICENSES" "$name/symapper" | gzip -n > "$out/$name.tar.gz"
cp "$tmp/$name/symapper" "$out/dist/$triple/symapper"
chmod 0755 "$out/dist/$triple/symapper"
