#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: npm-stage.sh <version> <dist> <out>" >&2
  exit 2
fi
root=$(cd "$(dirname "$0")/.." && pwd)
version=$1
dist=$2
out=$3
cargo_version=$(sed -n '/^\[package\]$/,/^\[/p' "$root/Cargo.toml" |
  sed -n 's/^version *= *"\([^"]*\)".*/\1/p')
if [[ "$version" != "$cargo_version" ]]; then
  echo "version mismatch: expected $cargo_version, got $version" >&2
  exit 2
fi
if [[ -e "$out" || -L "$out" ]]; then
  if [[ ! -d "$out" || -L "$out" || -n "$(ls -A "$out")" ]]; then
    echo "output must not exist or must be empty: $out" >&2
    exit 2
  fi
fi

triples=(aarch64-apple-darwin x86_64-apple-darwin x86_64-unknown-linux-musl aarch64-unknown-linux-musl)
systems=(darwin darwin linux linux)
cpus=(arm64 x64 x64 arm64)
count=0
for i in "${!triples[@]}"; do
  if [[ -f "$dist/${triples[$i]}/symapper" ]]; then
    count=$((count + 1))
  else
    echo "note: skipping @symapper-cli/${systems[$i]}-${cpus[$i]} (no binary)" >&2
  fi
done
if [[ "$count" -eq 0 ]]; then
  echo "no binaries to stage" >&2
  exit 2
fi

mkdir -p "$out/symapper"
# Copy all launcher content except development tests, including hidden files.
for entry in "$root/npm/symapper/"* "$root/npm/symapper/".[!.]* "$root/npm/symapper/"..?*; do
  [[ -e "$entry" ]] || continue
  [[ "$(basename "$entry")" == test ]] && continue
  cp -R "$entry" "$out/symapper/"
done
node -e '
  const fs = require("node:fs");
  const file = process.argv[1];
  const version = process.argv[2];
  const pkg = JSON.parse(fs.readFileSync(file, "utf8"));
  pkg.version = version;
  for (const name of Object.keys(pkg.optionalDependencies)) {
    pkg.optionalDependencies[name] = version;
  }
  fs.writeFileSync(file, JSON.stringify(pkg, null, 2) + "\n");
' "$out/symapper/package.json" "$version"
cp "$root/LICENSE" "$root/NOTICE" "$root/THIRD-PARTY-LICENSES" "$out/symapper/"
printf '%s\n' "$out/symapper"

for i in "${!triples[@]}"; do
  binary="$dist/${triples[$i]}/symapper"
  [[ -f "$binary" ]] || continue
  system=${systems[$i]}
  cpu=${cpus[$i]}
  name="@symapper-cli/$system-$cpu"
  pkg="$out/$name"
  mkdir -p "$pkg/bin"
  cp "$binary" "$pkg/bin/symapper"
  chmod 0755 "$pkg/bin/symapper"
  cp "$root/LICENSE" "$root/NOTICE" "$root/THIRD-PARTY-LICENSES" "$pkg/"
  node -e '
    const fs = require("node:fs");
    const [dir, name, version, os, cpu, launcher] = process.argv.slice(1);
    const repository = JSON.parse(fs.readFileSync(launcher, "utf8")).repository;
    const pkg = {
      name, version, description: "Prebuilt symapper binary for " + os + " " + cpu + ".",
      license: "Apache-2.0", repository, publishConfig: { access: "public" }, os: [os], cpu: [cpu],
      files: ["bin/symapper", "LICENSE", "NOTICE", "THIRD-PARTY-LICENSES"], preferUnplugged: true
    };
    fs.writeFileSync(dir + "/package.json", JSON.stringify(pkg, null, 2) + "\n");
  ' "$pkg" "$name" "$version" "$system" "$cpu" "$out/symapper/package.json"
  printf '# %s\nPrebuilt symapper binary for %s %s.\n' "$name" "$system" "$cpu" > "$pkg/README.md"
  printf '%s\n' "$pkg"
done
