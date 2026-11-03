// Configuration failures must be detected without opening a real account database.
import test from "node:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { config } from "../src/config.mjs";

const environment = () => ({
  PUBLIC_BASE_URL: "http://localhost:8080",
  DEV_LOCALHOST: "1",
  GOOGLE_CLIENT_ID: "fixture.invalid",
  GOOGLE_CLIENT_SECRET: "synthetic",
  STORE_KEY: "ab".repeat(32),
});

test("only explicit localhost development permits HTTP", () => {
  assert.equal(config(environment()).port, 8080);
  assert.throws(
    () => config({ ...environment(), DEV_LOCALHOST: "0" }),
    /HTTPS/,
  );
  assert.throws(
    () =>
      config({ ...environment(), PUBLIC_BASE_URL: "http://192.0.2.1:8080" }),
    /HTTPS/,
  );
  assert.equal(
    config({ ...environment(), PUBLIC_BASE_URL: "https://gateway.example.org" })
      .dev,
    false,
  );
});

test("check rejects a missing encryption key and checkout-local account storage", () => {
  assert.throws(() => config({ ...environment(), STORE_KEY: "" }), /STORE_KEY/);
  const checkout = fileURLToPath(new URL("../private-state", import.meta.url));
  assert.throws(
    () => config({ ...environment(), GATEWAY_DATA_DIR: checkout }),
    /outside/,
  );
});
