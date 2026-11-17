#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
host=$(rustc -vV | sed -n 's/^host: //p')
case "$host" in
  aarch64-apple-darwin) triple=$host; package=@symapper-cli/darwin-arm64 ;;
  x86_64-apple-darwin) triple=$host; package=@symapper-cli/darwin-x64 ;;
  x86_64-unknown-linux-musl|x86_64-unknown-linux-gnu)
    triple=x86_64-unknown-linux-musl; package=@symapper-cli/linux-x64 ;;
  aarch64-unknown-linux-musl|aarch64-unknown-linux-gnu)
    triple=aarch64-unknown-linux-musl; package=@symapper-cli/linux-arm64 ;;
  *) echo "smoke: unsupported host triple: $host" >&2; exit 2 ;;
esac
version=$(sed -n '/^\[package\]$/,/^\[/p' Cargo.toml |
  sed -n 's/^version *= *"\([^"]*\)".*/\1/p')
tmp=$(mktemp -d "${TMPDIR:-/tmp}/symapper-smoke.XXXXXX")
trap 'rm -r "$tmp"' EXIT
export npm_config_cache="$tmp/npm-cache"
build_args=(--release --locked)
if [[ "${SMOKE_CARGO_OFFLINE:-0}" == 1 ]]; then
  build_args+=(--offline)
fi
cargo build "${build_args[@]}"
mkdir -p "$tmp/dist/$triple" "$tmp/tarballs"
# GNU host builds exercise the Linux package locally; releases use musl.
cp "${CARGO_TARGET_DIR:-target}/release/symapper" "$tmp/dist/$triple/"
bash "$root/scripts/npm-stage.sh" "$version" "$tmp/dist" "$tmp/out"
(
  cd "$tmp/out/symapper"
  npm pack --offline --ignore-scripts --pack-destination "$tmp/tarballs"
  cd "$tmp/out/$package"
  npm pack --offline --ignore-scripts --pack-destination "$tmp/tarballs"
)
# Supply both tarballs in one resolution so the matching optional dependency is
# local. Unavailable optional platform packages may be skipped by offline npm.
npm install --prefix "$tmp/prefix" --offline --no-audit --no-fund --omit=dev \
  --ignore-scripts "$tmp/tarballs/symapper-$version.tgz" \
  "$tmp/tarballs/symapper-cli-${package#@symapper-cli/}-$version.tgz"
launcher="$tmp/prefix/node_modules/.bin/symapper"
actual=$("$launcher" --version)
[[ "$actual" == "symapper $version" ]] || {
  echo "smoke: unexpected version: $actual" >&2; exit 1;
}
mkdir -p "$tmp/repo/src"
printf 'pub fn hello() {}\npub fn caller() { hello(); }\n' > "$tmp/repo/src/lib.rs"
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
git -C "$tmp/repo" init -q
git -C "$tmp/repo" add src/lib.rs
"$launcher" "$tmp/repo" > "$tmp/map"
grep -q hello "$tmp/map"
# Compare the launcher with the same native binary, including its diagnostic.
set +e
"$tmp/dist/$triple/symapper" /nonexistent > "$tmp/native.out" 2> "$tmp/native.err"
native_rc=$?
"$launcher" /nonexistent > "$tmp/launcher.out" 2> "$tmp/launcher.err"
launcher_rc=$?
set -e
[[ "$native_rc" -ne 0 && "$launcher_rc" -eq "$native_rc" ]]
[[ -s "$tmp/native.err" ]]
cmp "$tmp/native.out" "$tmp/launcher.out"
cmp "$tmp/native.err" "$tmp/launcher.err"
echo "smoke: ok"
