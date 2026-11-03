// Synthetic identities/tokens only. No real Google requests, secrets or account data in tests.
import test from "node:test";
import assert from "node:assert/strict";
import { createGateway } from "../src/server.mjs";
import { Store } from "../src/store.mjs";
import { SCOPES } from "../src/google.mjs";
const mockGoogle = {
  async exchange() {
    return {
      access_token: "fixture_access",
      refresh_token: "fixture_refresh",
      expires_in: 3600,
      scope: SCOPES.calendar + " " + SCOPES.gmail,
      id_token: "ignored-unverified-jwt",
    };
  },
  async profile() {
    return {
      sub: "fixture-sub",
      email: "watch@example.invalid",
      email_verified: true,
      name: "Fixture",
    };
  },
  async refresh() {
    return { access_token: "refreshed_fixture_access", expires_in: 3600 };
  },
  async events() {
    return { events: [{ id: "fixture-event", title: "Test event" }] };
  },
  async messages() {
    return { messages: [] };
  },
  async message() {
    return { body: "fixture" };
  },
  async revoke() {},
};
async function setup(t) {
  let time = 1000000;
  const store = new Store(":memory:", "12".repeat(32));
  const device = store.register("Fixture Watch"),
    other = store.register("Other Watch");
  const server = createGateway({
    config: {
      base: "http://localhost:8080",
      dev: true,
      clientId: "fixture-client",
      clientSecret: "fixture-secret",
    },
    store,
    google: mockGoogle,
    now: () => time,
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const base = "http://127.0.0.1:" + server.address().port;
  t.after(async () => {
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
    store.close();
  });
  const auth = (d) => ({
    "X-Device-ID": d.device_id,
    Authorization: "Device " + d.device_credential,
    "Content-Type": "application/json",
  });
  async function create(services = ["calendar"]) {
    const response = await fetch(base + "/api/v1/google/pairings", {
      method: "POST",
      headers: auth(device),
      body: JSON.stringify({ services }),
    });
    assert.equal(response.status, 201);
    return response.json();
  }
  async function login(pair) {
    const start = await fetch(base + "/p/" + pair.pairing_id + "/start", {
      method: "POST",
      redirect: "manual",
    });
    assert.equal(start.status, 303);
    const state = new URL(start.headers.get("location")).searchParams.get(
      "state",
    );
    const cookie = start.headers.get("set-cookie").split(";")[0];
    return { state, cookie };
  }
  return {
    store,
    device,
    other,
    auth,
    base,
    create,
    login,
    advance(ms) {
      time += ms;
    },
  };
}

test("browser login needs separate device confirmation; tokens never reach the watch", async (t) => {
  const s = await setup(t),
    pair = await s.create(),
    login = await s.login(pair);
  const callback = await fetch(
    s.base + "/oauth/callback?state=" + login.state + "&code=fixture",
    { headers: { Cookie: login.cookie } },
  );
  assert.equal(callback.status, 200);
  assert.equal(s.store.account(s.device.device_id), null);
  const poll = await fetch(
    s.base + "/api/v1/google/pairings/" + pair.pairing_id,
    { headers: s.auth(s.device) },
  );
  const pending = await poll.json();
  assert.equal(pending.status, "awaiting_watch_confirmation");
  assert.deepEqual(pending.granted_services, ["calendar"]);
  assert.equal(pending.refresh_token, undefined);
  const denied = await fetch(
    s.base + "/api/v1/google/pairings/" + pair.pairing_id + "/confirm",
    { method: "POST", headers: s.auth(s.other) },
  );
  assert.equal(denied.status, 404);
  const confirm = await fetch(
    s.base + "/api/v1/google/pairings/" + pair.pairing_id + "/confirm",
    { method: "POST", headers: s.auth(s.device) },
  );
  assert.equal(confirm.status, 200);
  const linked = await confirm.json();
  assert.ok(linked.session);
  assert.equal(linked.access_token, undefined);
  const again = await fetch(
    s.base + "/api/v1/google/pairings/" + pair.pairing_id + "/confirm",
    { method: "POST", headers: s.auth(s.device) },
  );
  assert.equal(again.status, 409);
  const headers = {
    "X-Device-ID": s.device.device_id,
    Authorization: "Bearer " + linked.session,
  };
  const events = await fetch(s.base + "/api/v1/google/calendar/events", {
    headers,
  });
  assert.equal(events.status, 200);
  assert.equal((await events.json()).events[0].id, "fixture-event");
  const gmail = await fetch(s.base + "/api/v1/google/gmail/messages", {
    headers,
  });
  assert.equal(gmail.status, 403);
  const encrypted = s.store.device(s.device.device_id).account;
  assert.ok(!encrypted.includes("fixture_refresh"));
  assert.ok(!encrypted.includes("watch@example.invalid"));
  const logout = await fetch(s.base + "/api/v1/google/session", {
    method: "DELETE",
    headers,
  });
  assert.equal(logout.status, 200);
  const revoked = await fetch(s.base + "/api/v1/google/me", { headers });
  assert.equal(revoked.status, 401);
});

test("wrong browser cookie, reused state, unknown credentials and expired QR are rejected", async (t) => {
  const s = await setup(t),
    pair = await s.create(),
    login = await s.login(pair);
  const invalid = await fetch(
    s.base + "/oauth/callback?state=" + login.state + "&code=fixture",
    { headers: { Cookie: "watch_oauth=wrong" } },
  );
  assert.equal(invalid.status, 403);
  const valid = await fetch(
    s.base + "/oauth/callback?state=" + login.state + "&code=fixture",
    { headers: { Cookie: login.cookie } },
  );
  assert.equal(valid.status, 200);
  const replay = await fetch(
    s.base + "/oauth/callback?state=" + login.state + "&code=fixture",
    { headers: { Cookie: login.cookie } },
  );
  assert.equal(replay.status, 403);
  const noSecret = await fetch(s.base + "/api/v1/google/pairings", {
    method: "POST",
    headers: {
      "X-Device-ID": s.device.device_id,
      "Content-Type": "application/json",
    },
    body: "{}",
  });
  assert.equal(noSecret.status, 401);
  s.advance(300001);
  const expired = await fetch(
    s.base + "/api/v1/google/pairings/" + pair.pairing_id + "/confirm",
    { method: "POST", headers: s.auth(s.device) },
  );
  assert.equal(expired.status, 410);
  assert.equal(s.store.account(s.device.device_id), null);
});

test("AEAD prevents moving encrypted credentials between devices", () => {
  const store = new Store(":memory:", "34".repeat(32));
  try {
    const bytes = store.seal("first", { refresh_token: "fixture_refresh" });
    assert.deepEqual(store.open("first", bytes), {
      refresh_token: "fixture_refresh",
    });
    assert.throws(() => store.open("second", bytes));
  } finally {
    store.close();
  }
});
