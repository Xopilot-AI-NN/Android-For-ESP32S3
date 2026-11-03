// Exercise the real upstream adapter with synthetic HTTPS responses, never Google accounts.
import test from "node:test";
import assert from "node:assert/strict";
import { Google } from "../src/google.mjs";

test("calendar bounds range and normalizes readonly results", async () => {
  const original = globalThis.fetch;
  const calls = [];
  globalThis.fetch = async (url, options) => {
    calls.push([new URL(url), options]);
    return Response.json({
      timeZone: "UTC",
      items: [
        {
          id: "fixture",
          summary: "Test",
          start: { date: "2026-11-03" },
          end: { date: "2026-11-04" },
        },
      ],
    });
  };
  try {
    const google = new Google({});
    const result = await google.events(
      "synthetic",
      new URLSearchParams({
        from: "2026-11-03",
        to: "2026-11-04",
        limit: "50",
      }),
    );
    assert.equal(result.events[0].title, "Test");
    assert.equal(calls[0][0].origin, "https://www.googleapis.com");
    assert.equal(calls[0][0].searchParams.get("maxResults"), "20");
    assert.equal(calls[0][1].redirect, "error");
    assert.equal(calls[0][1].method, undefined); // Read-only GET, no calendar mutations.
    await assert.rejects(
      google.events(
        "synthetic",
        new URLSearchParams({ from: "2026-01-01", to: "2027-01-01" }),
      ),
      /INVALID_RANGE/,
    );
    assert.equal(calls.length, 1);
  } finally {
    globalThis.fetch = original;
  }
});

test("gmail extracts plain text and pages Unicode without fetching attachments", async () => {
  const original = globalThis.fetch;
  const text = "Ж".repeat(4095) + "😀" + "tail";
  const calls = [];
  globalThis.fetch = async (url) => {
    calls.push(new URL(url));
    return Response.json({
      payload: {
        mimeType: "multipart/mixed",
        parts: [
          {
            mimeType: "text/plain",
            body: { data: Buffer.from(text).toString("base64url") },
          },
          {
            mimeType: "application/pdf",
            body: { attachmentId: "not-downloaded" },
          },
        ],
      },
    });
  };
  try {
    const google = new Google({});
    const first = await google.message(
      "synthetic",
      "fixture",
      new URLSearchParams(),
    );
    assert.equal(Array.from(first.body).length, 4096);
    assert.ok(first.body.endsWith("😀"));
    assert.equal(first.has_more, true);
    const second = await google.message(
      "synthetic",
      "fixture",
      new URLSearchParams({ page: "1" }),
    );
    assert.equal(second.body, "tail");
    assert.equal(second.has_more, false);
    assert.equal(calls.length, 2);
    assert.ok(
      calls.every(
        (url) =>
          url.origin === "https://gmail.googleapis.com" &&
          url.searchParams.get("format") === "full",
      ),
    );
    await assert.rejects(
      google.message("synthetic", "../escape", new URLSearchParams()),
      /INVALID_MESSAGE_ID/,
    );
  } finally {
    globalThis.fetch = original;
  }
});
