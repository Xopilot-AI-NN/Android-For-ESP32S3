import test from "node:test";
import assert from "node:assert/strict";
import {
  Providers,
  publicAddress,
  pageText,
  plainText,
} from "../src/providers.mjs";
import { Store } from "../src/store.mjs";

test("text reader rejects private, mapped, obfuscated and credential-bearing targets before connecting", async () => {
  for (const ip of [
    "127.0.0.1",
    "10.0.0.1",
    "169.254.169.254",
    "192.168.1.1",
    "100.64.0.1",
    "198.18.0.1",
    "::1",
    "::ffff:8.8.8.8",
  ])
    assert.equal(publicAddress(ip), false, ip);
  assert.equal(publicAddress("8.8.8.8"), true);
  let connected = false;
  const request = () => {
    connected = true;
    throw Error("must not connect");
  };
  for (const url of [
    "https://127.0.0.1/",
    "https://2130706433/",
    "https://user:secret@example.org/",
    "http://example.org/",
    "https://example.org:8443/",
  ])
    await assert.rejects(pageText(url, { request }), /PAGE_NOT_ALLOWED/);
  await assert.rejects(
    pageText("https://private.example/", {
      resolve: async () => [{ address: "127.0.0.1" }],
      request,
    }),
    /PAGE_NOT_ALLOWED/,
  );
  assert.equal(connected, false);
  assert.equal(
    plainText('<script>bad()</script><p>A &amp; B</p><img src="private">'),
    "A & B",
  );
});

test("Brave results are labelled and page capabilities are bound to device and expiry", async () => {
  const original = globalThis.fetch;
  let now = 1000;
  globalThis.fetch = async (url, options) => {
    assert.ok(url.startsWith("https://api.search.brave.com/"));
    assert.equal(options.headers["X-Subscription-Token"], "synthetic-search");
    return Response.json({
      web: {
        results: [
          {
            title: "Fixture",
            description: "Text",
            url: "https://example.org/article",
          },
        ],
      },
    });
  };
  try {
    const p = new Providers(
      { searchKey: "synthetic-search" },
      { now: () => now, readPage: async () => "x".repeat(2048) + "tail" },
    );
    const result = await p.search("watch-one", "fixture");
    assert.equal(result.provider, "Brave Search");
    const id = result.results[0].id;
    await assert.rejects(p.page("watch-two", id, 0), /PAGE_EXPIRED/);
    assert.equal((await p.page("watch-one", id, 0)).has_more, true);
    assert.equal((await p.page("watch-one", id, 1)).body, "tail");
    now += 300001;
    await assert.rejects(p.page("watch-one", id, 0), /PAGE_EXPIRED/);
  } finally {
    globalThis.fetch = original;
  }
});

test("Cloud Translation uses a separate key, validates Unicode length, applies a hard local quota", async () => {
  const original = globalThis.fetch;
  let calls = 0;
  globalThis.fetch = async (url, options) => {
    calls++;
    assert.equal(
      url,
      "https://translation.googleapis.com/language/translate/v2",
    );
    assert.equal(options.headers["X-Goog-Api-Key"], "synthetic-translation");
    assert.equal(JSON.parse(options.body).format, "text");
    return Response.json({
      data: {
        translations: [
          {
            translatedText: "Hello &amp; welcome",
            detectedSourceLanguage: "ru",
          },
        ],
      },
    });
  };
  try {
    const p = new Providers({ translationKey: "synthetic-translation" });
    assert.equal(
      (
        await p.translate("watch", {
          text: "Привет",
          source: "auto",
          target: "en",
        })
      ).text,
      "Hello & welcome",
    );
    await assert.rejects(
      p.translate("watch", { text: "😀".repeat(1025) }),
      /INVALID_TRANSLATION/,
    );
    for (let i = 0; i < 4; i++)
      await p.translate("watch", { text: "x".repeat(1024) });
    await assert.rejects(
      p.translate("watch", { text: "x".repeat(1024) }),
      /BUDGET_EXCEEDED/,
    );
    assert.equal(calls, 5);
    await assert.rejects(
      new Providers({}).search("watch", "fixture"),
      /PROVIDER_NOT_CONFIGURED/,
    );
    await assert.rejects(
      new Providers({}).translate("watch", { text: "text" }),
      /PROVIDER_NOT_CONFIGURED/,
    );
  } finally {
    globalThis.fetch = original;
  }
});

test("daily budgets survive provider reconstruction and roll over on a new UTC day", () => {
  const store = new Store(":memory:", "ab".repeat(32));
  let now = 1000;
  try {
    const first = new Providers({}, { store, now: () => now });
    first.charge("device", "search", 2, 2);
    const restarted = new Providers({}, { store, now: () => now });
    assert.throws(
      () => restarted.charge("device", "search", 1, 2),
      /BUDGET_EXCEEDED/,
    );
    now += 86400000;
    restarted.charge("device", "search", 1, 2);
  } finally {
    store.close();
  }
});
