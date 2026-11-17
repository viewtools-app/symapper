"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const path = require("node:path");
const { packageFor, binaryPath, run } = require("../bin/symapper.js");

const source = "cargo install --git https://github.com/viewtools-app/symapper --locked";
const binary = path.resolve("fixture/bin/symapper");
function harness(overrides = {}) {
  const messages = [];
  const calls = [];
  const deps = {
    platform: "linux", arch: "x64",
    resolve: () => path.resolve("fixture/package.json"),
    spawnSync: (...args) => { calls.push(args); return { status: 0 }; },
    stderr: { write: (message) => messages.push(message) },
    ...overrides,
  };
  return { deps, messages, calls };
}

test("maps only the four supported platforms", () => {
  for (const [platform, arch] of [
    ["darwin", "arm64"], ["darwin", "x64"],
    ["linux", "x64"], ["linux", "arm64"],
  ]) assert.equal(packageFor(platform, arch), `@symapper-cli/${platform}-${arch}`);
  assert.equal(packageFor("win32", "x64"), null);
  assert.equal(packageFor("linux", "ia32"), null);
});

test("resolves the package binary using its manifest", () => {
  assert.equal(binaryPath("@symapper-cli/linux-x64", (request) => {
    assert.equal(request, "@symapper-cli/linux-x64/package.json");
    return path.resolve("fixture/package.json");
  }), binary);
  assert.equal(binaryPath("missing", () => { throw new Error("missing"); }), null);
});

test("unsupported platform reports the source install command", () => {
  const h = harness({ platform: "win32" });
  assert.equal(run([], h.deps), 1);
  assert.deepEqual(h.messages, [`symapper: no prebuilt binary for win32-x64. Install from source: ${source}\n`]);
  assert.equal(h.calls.length, 0);
});

test("missing optional package reports reinstall guidance", () => {
  const h = harness({ resolve: () => { throw new Error("missing"); } });
  assert.equal(run([], h.deps), 1);
  assert.deepEqual(h.messages, [`symapper: package @symapper-cli/linux-x64 is not installed. Reinstall without --omit=optional, or install from source: ${source}\n`]);
  assert.equal(h.calls.length, 0);
});

test("passes through argv, inherited stdio and child exit status", () => {
  const h = harness({ spawnSync: (...args) => { h.calls.push(args); return { status: 3 }; } });
  const argv = ["repo with spaces", "--focus", "a;b", "--tokens", "100"];
  assert.equal(run(argv, h.deps), 3);
  assert.deepEqual(h.calls, [[binary, argv, { stdio: "inherit" }]]);
  assert.deepEqual(h.messages, []);
});

test("reports spawn errors", () => {
  const h = harness({ spawnSync: () => ({ error: new Error("permission denied"), status: null }) });
  assert.equal(run([], h.deps), 1);
  assert.deepEqual(h.messages, [`symapper: cannot run ${binary}: permission denied\n`]);
});

test("re-raises the child signal using injected kill, then falls back to 1", () => {
  const calls = [];
  const h = harness({
    spawnSync: () => ({ status: null, signal: "SIGTERM" }),
    kill: (...args) => calls.push(args),
  });
  assert.equal(run([], h.deps), 1);
  assert.deepEqual(calls, [[process.pid, "SIGTERM"]]);
});

test("falls back to 1 if re-raising the signal fails", () => {
  const h = harness({
    spawnSync: () => ({ status: null, signal: "SIGTERM" }),
    kill: () => { throw new Error("kill failed"); },
  });
  assert.equal(run([], h.deps), 1);
});

test("missing child status falls back to 1", () => {
  const h = harness({ spawnSync: () => ({ status: null }) });
  assert.equal(run([], h.deps), 1);
});
