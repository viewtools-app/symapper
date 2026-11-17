"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const root = path.resolve(__dirname, "../../..");
const packages = ["@symapper-cli/darwin-arm64", "@symapper-cli/darwin-x64", "@symapper-cli/linux-x64", "@symapper-cli/linux-arm64", "symapper"];
function fixture(t) {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "publish-test-"));
  t.after(() => fs.rmSync(tmp, { recursive: true, force: true }));
  for (const name of packages) {
    fs.mkdirSync(path.join(tmp, name), { recursive: true });
    fs.writeFileSync(path.join(tmp, name, "package.json"), JSON.stringify({ name, version: "1.2.3" }));
  }
  const bin = path.join(tmp, "fake-bin");
  fs.mkdirSync(bin);
  fs.writeFileSync(path.join(bin, "npm"), `#!/usr/bin/env node
const fs = require("node:fs");
const name = require(process.cwd() + "/package.json").name;
const args = process.argv.slice(2);
if (args[0] === "view") {
  if (process.env.SCENARIO === "new" || (process.env.SCENARIO === "mismatch" && name !== "symapper")) process.exit(1);
  console.log(args[2] === "version" ? "1.2.3" : "sha512-registry");
} else if (args[0] === "pack") {
  console.log(JSON.stringify([{ integrity: process.env.SCENARIO === "mismatch" ? "sha512-different" : "sha512-registry" }]));
} else if (args[0] === "publish") {
  fs.appendFileSync(process.env.PUBLISH_LOG, name + " " + args.join(" ") + "\\n");
} else process.exit(2);
`, { mode: 0o755 });
  const log = path.join(tmp, "published");
  return { log, run: (scenario, args = []) => spawnSync("bash", [path.join(root, "scripts/npm-publish.sh"), tmp, ...args], {
    encoding: "utf8", env: { ...process.env, PATH: bin + path.delimiter + process.env.PATH,
      SCENARIO: scenario, PUBLISH_LOG: log, npm_config_cache: path.join(tmp, "npm-cache") },
  }) };
}
test("publish skips existing packages only when integrity matches", (t) => {
  const f = fixture(t);
  const result = f.run("equal");
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.match(/skip /g).length, 5);
  assert.equal(fs.existsSync(f.log), false);
});
test("publish refuses a later integrity mismatch before publishing anything", (t) => {
  const f = fixture(t);
  const result = f.run("mismatch");
  assert.equal(result.status, 1, result.stderr);
  assert.match(result.stderr, /error: symapper@1.2.3 on the registry differs from the staged package/);
  assert.equal(fs.existsSync(f.log), false);
});
test("publish preserves platform order and dry-run for new versions", (t) => {
  const f = fixture(t);
  const result = f.run("new", ["--dry-run"]);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(fs.readFileSync(f.log, "utf8").trim().split("\n"),
    packages.map((name) => name + " publish --access public --dry-run"));
});
