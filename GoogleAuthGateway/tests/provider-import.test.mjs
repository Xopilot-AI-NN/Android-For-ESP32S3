import test from "node:test";
import assert from "node:assert/strict";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  readFileSync,
  rmSync,
} from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
test("provider import never echoes malformed credentials and preserves existing OAuth settings", () => {
  const root = mkdtempSync(join(tmpdir(), "watch-import-"));
  try {
    const config = join(root, "aosp-wear-gateway");
    mkdirSync(config, { mode: 0o700 });
    writeFileSync(
      join(config, "config.env"),
      'GOOGLE_CLIENT_SECRET="synthetic-oauth"\nSTORE_KEY="synthetic-store"\n',
      { mode: 0o600 },
    );
    const secret = "synthetic-private-provider-value";
    const input = join(root, "input.json");
    const command = fileURLToPath(
      new URL("../src/configure-providers.mjs", import.meta.url),
    );
    const run = () =>
      spawnSync(process.execPath, [command, input], {
        env: { ...process.env, XDG_CONFIG_HOME: root },
        encoding: "utf8",
      });
    writeFileSync(input, secret);
    const bad = run();
    assert.notEqual(bad.status, 0);
    assert.ok(!(bad.stderr + bad.stdout).includes(secret));
    writeFileSync(input, JSON.stringify({ google_translate_api_key: secret }));
    assert.equal(run().status, 0);
    const contents = readFileSync(join(config, "config.env"), "utf8");
    assert.ok(contents.includes("synthetic-oauth"));
    assert.ok(contents.includes("GOOGLE_TRANSLATE_API_KEY="));
    writeFileSync(input, JSON.stringify({brave_search_api_key:secret}));
    assert.equal(run().status,0);
    assert.ok(!readFileSync(join(config,"config.env"),"utf8").includes("BRAVE_SEARCH_API_KEY="));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
