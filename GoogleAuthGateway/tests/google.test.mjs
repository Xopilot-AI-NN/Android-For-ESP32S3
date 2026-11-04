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

test("calendar month/pagination and Gmail charset/attachment handling use readonly endpoints", async () => {
  const original = globalThis.fetch;
  const calls = [];
  globalThis.fetch = async (url) => {
    calls.push(new URL(url));
    if (url.includes("/calendar/"))
      return Response.json({
        timeZone: "Europe/Moscow",
        nextPageToken: "next-token",
        items: [],
      });
    return Response.json({
      payload: {
        mimeType: "multipart/mixed",
        parts: [
          {
            mimeType: "text/plain",
            headers: [
              {
                name: "Content-Type",
                value: "text/plain; charset=windows-1251",
              },
            ],
            body: {
              data: Buffer.from([0xcf, 0xf0, 0xe8, 0xe2, 0xe5, 0xf2]).toString(
                "base64url",
              ),
            },
          },
          {
            mimeType: "text/plain",
            filename: "attachment.txt",
            body: {
              data: Buffer.from("must not render").toString("base64url"),
            },
          },
        ],
      },
    });
  };
  try {
    const google = new Google({});
    const events = await google.events(
      "synthetic",
      new URLSearchParams({ month: "2026-11", page: "page-token" }),
    );
    assert.equal(events.next_page, "next-token");
    assert.equal(calls[0].searchParams.get("pageToken"), "page-token");
    assert.equal(
      calls[0].searchParams.get("timeMin"),
      "2026-10-31T00:00:00.000Z",
    );
    assert.equal(
      (await google.message("synthetic", "fixture", new URLSearchParams()))
        .body,
      "Привет",
    );
  } finally {
    globalThis.fetch = original;
  }
});

// Both Google error envelopes occur in practice; neither means the user revoked consent.
test("disabled Google API is reported as project configuration, not missing consent", async () => {
  const original = globalThis.fetch;
  try {
    for (const error of [
      { errors: [{ reason: "accessNotConfigured" }] },
      { status: "PERMISSION_DENIED", details: [{ reason: "SERVICE_DISABLED" }] },
    ]) {
      globalThis.fetch = async () => Response.json({ error }, { status: 403 });
      await assert.rejects(new Google({}).messages("synthetic", new URLSearchParams()),
        error => error.code === "GOOGLE_API_DISABLED" && error.status === 503);
    }
  } finally { globalThis.fetch = original; }
});
