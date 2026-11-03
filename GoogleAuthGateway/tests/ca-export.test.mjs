// Certificate material is generated only under a private temporary directory.
import test from "node:test";
import assert from "node:assert/strict";
import https from "node:https";
import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
  rmSync,
  existsSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync, spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
test("CA export requires verified TLS and never trusts a certificate just because a peer sends it", async (t) => {
  const root = mkdtempSync(join(tmpdir(), "watch-ca-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const cert = join(root, "cert.pem"),
    key = join(root, "key.pem");
  const generated = spawnSync(
    "openssl",
    [
      "req",
      "-x509",
      "-newkey",
      "ec",
      "-pkeyopt",
      "ec_paramgen_curve:P-256",
      "-nodes",
      "-days",
      "1",
      "-subj",
      "/CN=localhost",
      "-addext",
      "subjectAltName=DNS:localhost",
      "-keyout",
      key,
      "-out",
      cert,
    ],
    { encoding: "utf8" },
  );
  assert.equal(generated.status, 0);
  const server = https.createServer({
    cert: readFileSync(cert),
    key: readFileSync(key),
  });
  server.on("tlsClientError", () => {});
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  t.after(() => new Promise((resolve) => server.close(resolve)));
  const directory = join(root, "aosp-wear-gateway");
  mkdirSync(directory, { mode: 0o700 });
  const envFile = join(directory, "config.env");
  writeFileSync(
    envFile,
    [
      `PUBLIC_BASE_URL="https://localhost:${server.address().port}"`,
      'GOOGLE_CLIENT_ID="synthetic"',
      'GOOGLE_CLIENT_SECRET="synthetic"',
      `STORE_KEY="${"ab".repeat(32)}"`,
    ].join("\n"),
    { mode: 0o600 },
  );
  const script = fileURLToPath(
    new URL("../src/export-ca.mjs", import.meta.url),
  );
  async function run(trusted) {
    return new Promise((resolve) => {
      const env = { ...process.env, XDG_CONFIG_HOME: root };
      delete env.NODE_EXTRA_CA_CERTS;
      delete env.NODE_TLS_REJECT_UNAUTHORIZED;
      if (trusted) env.NODE_EXTRA_CA_CERTS = cert;
      const child = spawn(process.execPath, ["--env-file=" + envFile, script], {
        env,
        stdio: ["ignore", "pipe", "pipe"],
      });
      let output = "";
      child.stdout.on("data", (data) => {
        output += data;
      });
      child.stderr.on("data", (data) => {
        output += data;
      });
      child.on("exit", (status) => resolve({ status, output }));
    });
  }
  assert.notEqual((await run(false)).status, 0);
  assert.equal(existsSync(join(directory, "gateway-ca.pem")), false);
  assert.equal((await run(true)).status, 0);
  assert.equal(
    readFileSync(join(directory, "gateway-ca.pem"), "utf8").trim(),
    readFileSync(cert, "utf8").trim(),
  );
});
