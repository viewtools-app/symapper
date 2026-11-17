#!/usr/bin/env node
"use strict";

const path = require("node:path");
const { spawnSync } = require("node:child_process");

const sourceInstall = "cargo install --git https://github.com/viewtools-app/symapper --locked";

function packageFor(platform, arch) {
  if ((platform === "darwin" || platform === "linux") &&
      (arch === "arm64" || arch === "x64")) {
    return `@symapper-cli/${platform}-${arch}`;
  }
  return null;
}

function binaryPath(pkg, resolve = require.resolve) {
  try {
    return path.join(path.dirname(resolve(`${pkg}/package.json`)), "bin", "symapper");
  } catch {
    return null;
  }
}

function run(argv, deps = {}) {
  const platform = deps.platform ?? process.platform;
  const arch = deps.arch ?? process.arch;
  const stderr = deps.stderr ?? process.stderr;
  const pkg = packageFor(platform, arch);
  if (!pkg) {
    stderr.write(`symapper: no prebuilt binary for ${platform}-${arch}. Install from source: ${sourceInstall}\n`);
    return 1;
  }
  const binary = binaryPath(pkg, deps.resolve);
  if (!binary) {
    stderr.write(`symapper: package ${pkg} is not installed. Reinstall without --omit=optional, or install from source: ${sourceInstall}\n`);
    return 1;
  }
  const child = (deps.spawnSync ?? spawnSync)(binary, argv, { stdio: "inherit" });
  if (child.error) {
    stderr.write(`symapper: cannot run ${binary}: ${child.error.message}\n`);
    return 1;
  }
  if (child.signal) {
    try {
      (deps.kill ?? process.kill)(process.pid, child.signal);
    } catch {
      // A failed signal delivery still means the child did not complete successfully.
    }
    return 1;
  }
  return child.status ?? 1;
}

if (require.main === module) {
  process.exit(run(process.argv.slice(2)));
} else {
  module.exports = { packageFor, binaryPath, run };
}
