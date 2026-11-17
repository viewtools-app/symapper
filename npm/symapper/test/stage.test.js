"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const root = path.resolve(__dirname, "../../..");
const version = fs.readFileSync(path.join(root, "Cargo.toml"), "utf8")
  .match(/\[package\][\s\S]*?^version = "([^"]+)"/m)[1];
const platforms = [
  ["aarch64-apple-darwin", "darwin", "arm64"],
  ["x86_64-apple-darwin", "darwin", "x64"],
  ["x86_64-unknown-linux-musl", "linux", "x64"],
  ["aarch64-unknown-linux-musl", "linux", "arm64"],
];
function fixture(t) {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "stage-test-"));
  t.after(() => fs.rmSync(tmp, { recursive: true, force: true }));
  const dist = path.join(tmp, "dist");
  fs.mkdirSync(dist);
  const out = path.join(tmp, "out");
  const run = (v = version) => spawnSync("bash", [path.join(root, "scripts/npm-stage.sh"), v, dist, out], { encoding: "utf8" });
  return { dist, out, run };
}
test("stage rejects version mismatch, occupied output and no binaries", (t) => {
  const f = fixture(t);
  assert.equal(f.run("0.0.0-wrong").status, 2);
  fs.mkdirSync(f.out);
  fs.writeFileSync(path.join(f.out, "keep"), "keep");
  assert.equal(f.run().status, 2);
  assert.equal(fs.readFileSync(path.join(f.out, "keep"), "utf8"), "keep");
  fs.unlinkSync(path.join(f.out, "keep"));
  assert.equal(f.run().status, 2);
});
test("stage creates portable manifests, executable binaries and launcher", (t) => {
  const f = fixture(t);
  for (const [triple] of platforms) {
    fs.mkdirSync(path.join(f.dist, triple));
    fs.writeFileSync(path.join(f.dist, triple, "symapper"), "fixture binary");
  }
  const result = f.run();
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.trim().split("\n").length, 5);
  const launcher = JSON.parse(fs.readFileSync(path.join(f.out, "symapper/package.json")));
  assert.equal(launcher.version, version);
  assert.deepEqual(launcher.files, ["bin/symapper.js", "README.md", "LICENSE", "NOTICE", "THIRD-PARTY-LICENSES"]);
  assert.deepEqual(Object.values(launcher.optionalDependencies), Array(4).fill(version));
  assert.equal(fs.existsSync(path.join(f.out, "symapper/test")), false);
  for (const [triple, osName, cpu] of platforms) {
    const pkg = path.join(f.out, `@symapper-cli/${osName}-${cpu}`);
    const manifest = JSON.parse(fs.readFileSync(path.join(pkg, "package.json")));
    assert.equal(manifest.name, "@symapper-cli/" + path.basename(pkg));
    assert.equal(manifest.version, version);
    assert.deepEqual(manifest.os, [osName]);
    assert.deepEqual(manifest.cpu, [cpu]);
    assert.equal(manifest.description, `Prebuilt symapper binary for ${osName} ${cpu}.`);
    assert.equal(manifest.license, "Apache-2.0");
    assert.deepEqual(manifest.publishConfig, { access: "public" });
    assert.deepEqual(manifest.repository, launcher.repository);
    assert.deepEqual(manifest.files, ["bin/symapper", "LICENSE", "NOTICE", "THIRD-PARTY-LICENSES"]);
    assert.equal(manifest.preferUnplugged, true);
    assert.equal(fs.statSync(path.join(pkg, "bin/symapper")).mode & 0o777, 0o755);
    assert.equal(fs.readFileSync(path.join(pkg, "bin/symapper"), "utf8"), "fixture binary");
    assert.equal(fs.readFileSync(path.join(pkg, "README.md"), "utf8").trimEnd().split("\n").length, 2);
    for (const file of ["LICENSE", "NOTICE", "THIRD-PARTY-LICENSES"]) {
      assert.deepEqual(fs.readFileSync(path.join(pkg, file)), fs.readFileSync(path.join(root, file)));
      assert.deepEqual(fs.readFileSync(path.join(f.out, "symapper", file)), fs.readFileSync(path.join(root, file)));
    }
  }
});
test("stage accepts empty output and skips unavailable targets", (t) => {
  const f = fixture(t);
  fs.mkdirSync(f.out);
  fs.mkdirSync(path.join(f.dist, platforms[0][0]));
  fs.writeFileSync(path.join(f.dist, platforms[0][0], "symapper"), "binary");
  const result = f.run();
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.trim().split("\n").length, 2);
  assert.match(result.stderr, /note: skipping @symapper-cli\/linux-x64 \(no binary\)/);
});
