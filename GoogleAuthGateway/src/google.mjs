import { mailPages, mailImage } from "./mail.mjs";
// Fixed upstream hosts only. No endpoint accepts a URL supplied by a watch or email.
export const SCOPES = {
  calendar: "https://www.googleapis.com/auth/calendar.events.readonly",
  gmail: "https://www.googleapis.com/auth/gmail.readonly",
  contacts: "https://www.googleapis.com/auth/contacts.readonly",
  photos: "https://www.googleapis.com/auth/photospicker.mediaitems.readonly",
};
export class ApiError extends Error {
  constructor(code, status = 502) {
    super(code);
    this.code = code;
    this.status = status;
  }
}
export async function json(url, options = {}, limit = 1024 * 1024) {
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
    const reasons = (data.error?.errors || []).map((item) => item.reason);
    const details = (data.error?.details || []).map((item) => item.reason);
    // Consent grants account access, but the API must also be enabled in its Cloud project.
    // This is configuration trouble: keep the account and its granted scopes intact.
    if (response.status === 403 &&
        (reasons.includes("accessNotConfigured") || details.includes("SERVICE_DISABLED")))
      throw new ApiError("GOOGLE_API_DISABLED", 503);
    if (
      reasons.some((reason) =>
        [
          "rateLimitExceeded",
          "userRateLimitExceeded",
          "quotaExceeded",
        ].includes(reason),
      )
    )
      throw new ApiError("RATE_LIMITED", 429);
    if (reasons.includes("insufficientPermissions"))
      throw new ApiError("SCOPE_MISSING", 403);
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
function calendarWhen(start, timeZone = "UTC") {
  if (start?.date) return start.date.split("-").reverse().join(".") + " · весь день";
  if (!start?.dateTime) return "";
  try {
    return new Intl.DateTimeFormat("ru-RU", {day:"2-digit",month:"2-digit",hour:"2-digit",minute:"2-digit",
      timeZone:start.timeZone || timeZone}).format(new Date(start.dateTime));
  } catch { return String(start.dateTime).slice(0,32); }
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
        redirect_uri: this.config.oauthRedirect || this.config.base + "/oauth/callback",
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
    let start = query.get("from") || new Date().toISOString(),
      end =
        query.get("to") || new Date(Date.now() + 14 * 86400000).toISOString();
    const month = query.get("month");
    if (month) {
      if (!/^(19|20)\d{2}-(0[1-9]|1[0-2])$/.test(month))
        throw new ApiError("INVALID_MONTH", 400);
      const [y, m] = month.split("-").map(Number);
      start = new Date(Date.UTC(y, m - 1, 0)).toISOString();
      end = new Date(Date.UTC(y, m, 2)).toISOString();
    }
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
    const page = query.get("page") || "";
    if (page.length > 512 || !/^[\w+/=-]*$/.test(page))
      throw new ApiError("INVALID_PAGE", 400);
    if (page) params.set("pageToken", page);
    const data = await json(
      "https://www.googleapis.com/calendar/v3/calendars/primary/events?" +
        params,
      { headers: { Authorization: `Bearer ${token}` } },
    );
    return {
      time_zone: data.timeZone,
      next_page: data.nextPageToken || null,
      events: (data.items || []).slice(0, limit).map((e) => ({
        id: e.id,
        title: String(e.summary || "Без названия").slice(0, 160),
        start: e.start,
        end: e.end,
        when: calendarWhen(e.start, data.timeZone),
        location: String(e.location || "").slice(0, 160),
        reminders: e.reminders
          ? {
              use_default: Boolean(e.reminders.useDefault),
              overrides: (e.reminders.overrides || [])
                .slice(0, 5)
                .map((value) => ({
                  method: String(value.method || "").slice(0, 16),
                  minutes: value.minutes,
                })),
            }
          : null,
      })),
    };
  }
  async contacts(token, query) {
    const page = query.get("page") || "";
    if (page.length > 512 || !/^[\w+/=-]*$/.test(page)) throw new ApiError("INVALID_PAGE", 400);
    const params = new URLSearchParams({pageSize:"10",personFields:"names,emailAddresses,phoneNumbers",sortOrder:"FIRST_NAME_ASCENDING"});
    if (page) params.set("pageToken", page);
    const data = await json("https://people.googleapis.com/v1/people/me/connections?" + params,
      {headers:{Authorization:`Bearer ${token}`}});
    return {contacts:(data.connections || []).slice(0,10).map(person => ({
      id:String(person.resourceName || "").slice(0,128),
      title:String(person.names?.[0]?.displayName || person.emailAddresses?.[0]?.value || "Без имени").slice(0,160),
      subtitle:String(person.phoneNumbers?.[0]?.value || person.emailAddresses?.[0]?.value || "").slice(0,200),
      body:[...(person.phoneNumbers || []).slice(0,3).map(p => p.value),
        ...(person.emailAddresses || []).slice(0,3).map(p => p.value)].join("\n").slice(0,1024),
    })),page_token:page,next_page:data.nextPageToken || null};
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
    return { messages: items, page_token: page, next_page: data.nextPageToken || null };
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
      html = [], inline = new Map();
    function visit(part, depth = 0) {
      if (depth > 10 || !part) return;
      const headers = part.headers || [];
      const cid = headers.find(h => String(h.name).toLowerCase() === "content-id")?.value?.replace(/^<|>$/g, "");
      if (cid && /^image\/(jpeg|png|gif|webp)$/.test(part.mimeType || "")) inline.set(cid, part);
      if (
        part.filename ||
        headers.some(
          (h) =>
            String(h.name).toLowerCase() === "content-disposition" &&
            /^attachment\b/i.test(h.value || ""),
        )
      )
        return;
      if (part.body?.data) {
        const raw = Buffer.from(part.body.data, "base64url");
        const contentType =
          headers.find((h) => String(h.name).toLowerCase() === "content-type")
            ?.value || "";
        const charset =
          /charset\s*=\s*["']?([\w-]+)/i.exec(contentType)?.[1] || "utf-8";
        let text;
        try {
          text = new TextDecoder(charset).decode(raw);
        } catch {
          text = raw.toString("utf8");
        }
        if (part.mimeType === "text/plain") plain.push(text);
        else if (part.mimeType === "text/html") html.push(text);
      }
      for (const child of part.parts || []) visit(child, depth + 1);
    }
    visit(data.payload);
    const pages = await mailPages(html.join("\n"), plain.join("\n"));
    const selected = pages[page] || {type:"text",text:""};
    const result = {id,body:selected.text || "",page,has_more:pages.length > page+1,
      page_count:pages.length,format:html.length ? "text/html-native" : "text/plain"};
    if(selected.type === "image") {
      result.body = String(selected.alt || "Изображение").slice(0,120);
      try {
        result.image = await mailImage(selected,inline,async attachmentId => {
          const value = await json("https://gmail.googleapis.com/gmail/v1/users/me/messages/" + id +
            "/attachments/" + encodeURIComponent(attachmentId),{headers:{Authorization:`Bearer ${token}`}},3*1024*1024);
          return Buffer.from(value.data || "","base64url");
        });
      } catch { result.image_unavailable = true; }
    }
    return result;
  }
}
