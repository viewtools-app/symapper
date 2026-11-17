#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 || (${2:-} != "" && ${2:-} != --dry-run) ]]; then
  echo "usage: npm-publish.sh <stage-out> [--dry-run]" >&2
  exit 2
fi
stage=$(cd "$1" && pwd)
args=(--access public)
if [[ ${2:-} == --dry-run ]]; then
  args+=(--dry-run)
fi
packages=(@symapper-cli/darwin-arm64 @symapper-cli/darwin-x64 @symapper-cli/linux-x64 @symapper-cli/linux-arm64 symapper)
for package in "${packages[@]}"; do
  if [[ ! -d "$stage/$package" || ! -f "$stage/$package/package.json" ]]; then
    echo "missing package: $package" >&2
    exit 2
  fi
done
# Verify every existing version before publishing any package.
existing=()
for package in "${packages[@]}"; do
  cd "$stage/$package"
  name=$(node -p 'require("./package.json").name')
  version=$(node -p 'require("./package.json").version')
  published=$(npm view "$name@$version" version 2>/dev/null) || published=""
  if [[ "$published" == "$version" ]]; then
    local_integrity=$(npm pack --dry-run --json | node -e '
      let input = "";
      process.stdin.on("data", chunk => input += chunk);
      process.stdin.on("end", () => {
        const integrity = JSON.parse(input)[0].integrity;
        if (!integrity) process.exit(1);
        console.log(integrity);
      });
    ')
    registry_integrity=$(npm view "$name@$version" dist.integrity)
    if [[ "$local_integrity" != "$registry_integrity" ]]; then
      echo "error: $name@$version on the registry differs from the staged package" >&2
      exit 1
    fi
    existing+=(yes)
  else
    existing+=(no)
  fi
done
for i in "${!packages[@]}"; do
  (
    cd "$stage/${packages[$i]}"
    if [[ ${existing[$i]} == yes ]]; then
      name=$(node -p 'require("./package.json").name')
      version=$(node -p 'require("./package.json").version')
      echo "skip $name@$version (already published)"
    else
      npm publish "${args[@]}"
    fi
  )
done
