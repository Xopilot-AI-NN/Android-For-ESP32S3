// Fixed upstream hosts only. No endpoint accepts a URL supplied by a watch or email.
export const SCOPES = {
  calendar: "https://www.googleapis.com/auth/calendar.events.readonly",
  gmail: "https://www.googleapis.com/auth/gmail.readonly",
};
export class ApiError extends Error {
  constructor(code, status = 502) {
    super(code);
    this.code = code;
    this.status = status;
  }
}
async function json(url, options = {}, limit = 1024 * 1024) {
  let response;
  try {
    response = await fetch(url, {
      ...options,
      redirect: "error",
      signal: AbortSignal.timeout(10000),
    });
  } catch {
    throw new ApiError("UPSTREAM_UNAVAILABLE");
  }
  let size = 0;
  const chunks = [];
  for await (const chunk of response.body) {
    size += chunk.length;
    if (size > limit) throw new ApiError("UPSTREAM_TOO_LARGE");
    chunks.push(chunk);
  }
  let data;
  try {
    data = JSON.parse(Buffer.concat(chunks).toString("utf8"));
  } catch {
    throw new ApiError("UPSTREAM_INVALID_RESPONSE");
  }
  if (!response.ok) {
    if (data.error === "invalid_grant")
      throw new ApiError("REAUTH_REQUIRED", 401);
    if (response.status === 429) throw new ApiError("RATE_LIMITED", 429);
    if (response.status === 401) throw new ApiError("REAUTH_REQUIRED", 401);
    throw new ApiError(
      "UPSTREAM_REJECTED",
      response.status === 403 ? 403 : 502,
    );
  }
  return data;
}
export class Google {
  constructor(config) {
    this.config = config;
  }
  exchange(code, verifier) {
    return json("https://oauth2.googleapis.com/token", {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: new URLSearchParams({
        grant_type: "authorization_code",
        code,
        code_verifier: verifier,
        client_id: this.config.clientId,
        client_secret: this.config.clientSecret,
        redirect_uri: this.config.base + "/oauth/callback",
      }),
    });
  }
  refresh(refresh_token) {
    return json("https://oauth2.googleapis.com/token", {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: new URLSearchParams({
        grant_type: "refresh_token",
        refresh_token,
        client_id: this.config.clientId,
        client_secret: this.config.clientSecret,
      }),
    });
  }
  // Identity comes from Google's TLS-protected userinfo endpoint. We never trust/decode an unchecked id_token.
  profile(token) {
    return json("https://openidconnect.googleapis.com/v1/userinfo", {
      headers: { Authorization: `Bearer ${token}` },
    });
  }
  async revoke(token) {
    try {
      await fetch("https://oauth2.googleapis.com/revoke", {
        method: "POST",
        headers: { "Content-Type": "application/x-www-form-urlencoded" },
        body: new URLSearchParams({ token }),
        signal: AbortSignal.timeout(8000),
        redirect: "error",
      });
    } catch {
      /* Local credentials are deleted even if Google is unreachable. */
    }
  }
  async events(token, query) {
    const start = query.get("from") || new Date().toISOString(),
      end =
        query.get("to") || new Date(Date.now() + 14 * 86400000).toISOString();
    const from = Date.parse(start),
      to = Date.parse(end);
    if (
      !Number.isFinite(from) ||
      !Number.isFinite(to) ||
      to <= from ||
      to - from > 90 * 86400000
    )
      throw new ApiError("INVALID_RANGE", 400);
    const limit = Math.min(Math.max(Number(query.get("limit") || 10), 1), 20);
    if (!Number.isInteger(limit)) throw new ApiError("INVALID_LIMIT", 400);
    const params = new URLSearchParams({
      timeMin: new Date(from).toISOString(),
      timeMax: new Date(to).toISOString(),
      singleEvents: "true",
      orderBy: "startTime",
      maxResults: String(limit),
    });
    const data = await json(
      "https://www.googleapis.com/calendar/v3/calendars/primary/events?" +
        params,
      { headers: { Authorization: `Bearer ${token}` } },
    );
    return {
      time_zone: data.timeZone,
      events: (data.items || [])
        .slice(0, limit)
        .map((e) => ({
          id: e.id,
          title: String(e.summary || "Без названия").slice(0, 160),
          start: e.start,
          end: e.end,
          location: String(e.location || "").slice(0, 160),
        })),
    };
  }
  async messages(token, query) {
    const limit = Math.min(Math.max(Number(query.get("limit") || 10), 1), 20);
    if (!Number.isInteger(limit)) throw new ApiError("INVALID_LIMIT", 400);
    const page = query.get("page") || "";
    if (page.length > 512 || !/^[\w+/=-]*$/.test(page))
      throw new ApiError("INVALID_PAGE", 400);
    const params = new URLSearchParams({
      maxResults: String(limit),
      labelIds: "INBOX",
    });
    if (page) params.set("pageToken", page);
    const data = await json(
      "https://gmail.googleapis.com/gmail/v1/users/me/messages?" + params,
      { headers: { Authorization: `Bearer ${token}` } },
    );
    const items = [];
    // Sequential metadata requests bound concurrency and avoid a burst of 20 upstream connections.
    for (const entry of (data.messages || []).slice(0, limit)) {
      const detail = await json(
        "https://gmail.googleapis.com/gmail/v1/users/me/messages/" +
          encodeURIComponent(entry.id) +
          "?format=metadata",
        { headers: { Authorization: `Bearer ${token}` } },
      );
      const headers = detail.payload?.headers || [],
        field = (name) =>
          String(
            headers.find((h) => h.name.toLowerCase() === name)?.value || "",
          ).slice(0, 200);
      items.push({
        id: detail.id,
        subject: field("subject"),
        from: field("from"),
        date: field("date"),
        snippet: String(detail.snippet || "").slice(0, 300),
        unread: (detail.labelIds || []).includes("UNREAD"),
      });
    }
    return { messages: items, next_page: data.nextPageToken || null };
  }
  async message(token, id, query) {
    if (!/^[\w-]{1,128}$/.test(id))
      throw new ApiError("INVALID_MESSAGE_ID", 400);
    const page = Number(query.get("page") || 0);
    if (!Number.isInteger(page) || page < 0 || page > 255)
      throw new ApiError("INVALID_PAGE", 400);
    const data = await json(
      "https://gmail.googleapis.com/gmail/v1/users/me/messages/" +
        id +
        "?format=full",
      { headers: { Authorization: `Bearer ${token}` } },
      2 * 1024 * 1024,
    );
    const plain = [],
      html = [];
    function visit(part, depth = 0) {
      if (depth > 10 || !part) return;
      if (part.body?.data) {
        const text = Buffer.from(part.body.data, "base64url").toString("utf8");
        if (part.mimeType === "text/plain") plain.push(text);
        else if (part.mimeType === "text/html") html.push(text);
      }
      for (const child of part.parts || []) visit(child, depth + 1);
    }
    visit(data.payload);
    let text = plain.join("\n");
    if (!text)
      text = html
        .join("\n")
        .replace(/<(script|style)\b[^>]*>[\s\S]*?<\/\1>/gi, "")
        .replace(/<[^>]*>/g, " ")
        .replace(/&nbsp;/gi, " ")
        .replace(/&lt;/gi, "<")
        .replace(/&gt;/gi, ">")
        .replace(/&amp;/gi, "&");
    const chars = Array.from(text);
    const body = chars.slice(page * 4096, (page + 1) * 4096).join("");
    return {
      id,
      body,
      page,
      has_more: chars.length > (page + 1) * 4096,
      format: "text/plain",
    };
  }
}
