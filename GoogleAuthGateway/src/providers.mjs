// Independent services use server API keys, never Gmail OAuth tokens.
import { duckSearch } from "./duckduckgo.mjs";
import https from "node:https";
import { lookup } from "node:dns/promises";
import { isIP } from "node:net";
import { opaque } from "./store.mjs";
import { ApiError } from "./google.mjs";

export function plainText(html) {
  return String(html)
    .replace(/<(script|style|noscript|svg)\b[^>]*>[\s\S]*?<\/\1>/gi, "")
    .replace(/<\/(p|div|h[1-6]|li)>|<br\s*\/?>/gi, "\n")
    .replace(/<[^>]*>/g, " ")
    .replace(/&(#x[0-9a-f]+|#\d+|amp|lt|gt|quot|apos|nbsp);/gi, (_, entity) => {
      const named = {
        amp: "&",
        lt: "<",
        gt: ">",
        quot: '"',
        apos: "'",
        nbsp: " ",
      };
      if (entity[0] !== "#") return named[entity.toLowerCase()] || "";
      const value =
        entity[1].toLowerCase() === "x"
          ? parseInt(entity.slice(2), 16)
          : parseInt(entity.slice(1), 10);
      return value > 0 &&
        value <= 0x10ffff &&
        !(value >= 0xd800 && value <= 0xdfff)
        ? String.fromCodePoint(value)
        : "";
    })
    .replace(/[ \t]+/g, " ")
    .replace(/\n\s*\n\s*\n/g, "\n\n")
    .trim();
}
// Keep the page reader on public IPv4. Each redirect is revalidated and its DNS
// answer is pinned into the HTTPS lookup callback, preventing DNS rebinding.
export function publicAddress(address) {
  if (isIP(address) !== 4) return false;
  const [a, b, c] = address.split(".").map(Number);
  return !(
    a === 0 ||
    a === 10 ||
    a === 127 ||
    a >= 224 ||
    (a === 169 && b === 254) ||
    (a === 172 && b >= 16 && b <= 31) ||
    (a === 192 && (b === 168 || b === 0 || b === 2)) ||
    (a === 100 && b >= 64 && b <= 127) ||
    (a === 198 && (b === 18 || b === 19 || (b === 51 && c === 100))) ||
    (a === 203 && b === 0 && c === 113)
  );
}
export async function pageText(raw, dependencies = {}, depth = 0) {
  return plainText((await publicResource(raw, "text", dependencies, depth)).toString("utf8"));
}
// Image downloads share the reader's DNS pinning, redirect checks and absolute deadline.
export async function publicResource(
  raw, kind = "text",
  { resolve = lookup, request = https.request } = {},
  depth = 0,
  metadata = false,
) {
  let url;
  try {
    url = new URL(raw);
  } catch {
    throw new ApiError("INVALID_PAGE", 400);
  }
  if (
    url.protocol !== "https:" ||
    url.username ||
    url.password ||
    (url.port && url.port !== "443") ||
    url.href.length > 2048 ||
    isIP(url.hostname)
  )
    throw new ApiError("PAGE_NOT_ALLOWED", 400);
  let dnsTimer;
  let addresses;
  try {
    addresses = await Promise.race([
      resolve(url.hostname, { all: true, family: 4 }),
      new Promise((_, reject) => {
        dnsTimer = setTimeout(
          () => reject(new ApiError("UPSTREAM_UNAVAILABLE")),
          5000,
        );
      }),
    ]);
  } catch {
    throw new ApiError("UPSTREAM_UNAVAILABLE");
  } finally {
    clearTimeout(dnsTimer);
  }
  if (
    !addresses.length ||
    addresses.some((item) => !publicAddress(item.address))
  )
    throw new ApiError("PAGE_NOT_ALLOWED", 400);
  const result = await new Promise((resolve, reject) => {
    const req = request(
      url,
      {
        method: "GET",
        headers: {
          Accept: kind === "image" ? "image/jpeg,image/png,image/gif,image/webp" : "text/html,text/plain",
          "Accept-Encoding": "identity",
          "User-Agent": "AOSP-Wear-Text/1",
        },
        lookup: (_host, options, callback) => {
          const item = { address: addresses[0].address, family: 4 };
          if (options.all) callback(null, [item]);
          else callback(null, item.address, item.family);
        },
      },
      (res) => {
        if ([301, 302, 303, 307, 308].includes(res.statusCode)) {
          res.resume();
          resolve({ redirect: res.headers.location });
          return;
        }
        if (
          res.statusCode !== 200 ||
          !(kind === "image" ? /^image\/(jpeg|png|gif|webp)(;|$)/i : /^text\/(html|plain)(;|$)/i).test(
            res.headers["content-type"] || "",
          ) ||
          (res.headers["content-encoding"] &&
            res.headers["content-encoding"] !== "identity")
        ) {
          res.resume();
          reject(new ApiError("PAGE_NOT_READABLE", 422));
          return;
        }
        let size = 0;
        const chunks = [];
        res.on("data", (chunk) => {
          size += chunk.length;
          if (size > (kind === "image" ? 2 * 1024 * 1024 : 256 * 1024)) {
            req.destroy(new ApiError("PAGE_TOO_LARGE", 413));
          } else chunks.push(chunk);
        });
        res.on("end", () =>
          resolve({ bytes: Buffer.concat(chunks), contentType: res.headers["content-type"] || "" }),
        );
        res.on("error", () => reject(new ApiError("UPSTREAM_UNAVAILABLE")));
      },
    );
    // Absolute deadline also covers DNS/connect/header/slow-body attacks.
    const timer = setTimeout(
      () => req.destroy(new ApiError("UPSTREAM_UNAVAILABLE")),
      10000,
    );
    req.on("close", () => clearTimeout(timer));
    req.on("error", (error) =>
      reject(
        error instanceof ApiError
          ? error
          : new ApiError("UPSTREAM_UNAVAILABLE"),
      ),
    );
    req.end();
  });
  if (result.redirect) {
    if (depth >= 3) throw new ApiError("TOO_MANY_REDIRECTS", 422);
    return publicResource(
      new URL(result.redirect, url).href, kind,
      { resolve, request },
      depth + 1, metadata,
    );
  }
  return metadata ? {bytes:result.bytes,url:url.href,contentType:result.contentType} : result.bytes;
}
async function json(url, options) {
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
  for await (const part of response.body) {
    size += part.length;
    if (size > 256 * 1024) throw new ApiError("UPSTREAM_TOO_LARGE");
    chunks.push(part);
  }
  if (!response.ok)
    throw new ApiError(
      response.status === 429 ? "RATE_LIMITED" : "UPSTREAM_REJECTED",
      response.status === 429 ? 429 : 502,
    );
  try {
    return JSON.parse(Buffer.concat(chunks).toString("utf8"));
  } catch {
    throw new ApiError("UPSTREAM_INVALID_RESPONSE");
  }
}
export class Providers {
  constructor(
    config,
    { now = () => Date.now(), readPage = pageText, search = duckSearch, store } = {},
  ) {
    this.config = config;
    this.now = now;
    this.readPage = readPage;
    this.searchProvider = search;
    this.searchCache = new Map();
    this.searchBlockedUntil = 0;
    this.store = store;
    this.pages = new Map();
    this.budgets = new Map();
  }
  charge(device, service, amount, limit) {
    const key = device + ":" + service,
      day = Math.floor(this.now() / 86400000);
    if (this.store) {
      if (amount > limit) throw new ApiError("BUDGET_EXCEEDED", 429);
      this.store.db
        .prepare("DELETE FROM provider_usage WHERE day<?")
        .run(day - 7);
      const result = this.store.db
        .prepare(
          "INSERT INTO provider_usage(day,device,service,spent) VALUES(?,?,?,?) ON CONFLICT(day,device,service) DO UPDATE SET spent=spent+excluded.spent WHERE spent+excluded.spent<=?",
        )
        .run(day, device, service, amount, limit);
      if (!result.changes) throw new ApiError("BUDGET_EXCEEDED", 429);
      return;
    }
    let entry = this.budgets.get(key);
    if (!entry || entry.day !== day) {
      entry = { day, spent: 0 };
      this.budgets.set(key, entry);
    }
    if (entry.spent + amount > limit)
      throw new ApiError("BUDGET_EXCEEDED", 429);
    entry.spent += amount;
  }
  async search(device, query) {
    if (
      typeof query !== "string" ||
      !query.trim() ||
      Array.from(query).length > 512
    )
      throw new ApiError("INVALID_QUERY", 400);
    const cacheKey = JSON.stringify([device, query.trim()]);
    const cached = this.searchCache.get(cacheKey);
    if (cached?.expires > this.now()) return cached.result;
    if (this.searchBlockedUntil > this.now()) throw new ApiError("SEARCH_VERIFICATION_REQUIRED", 429);
    this.charge(device, "search", 1, 100);
    this.charge("global", "search", 1, 200);
    let data;
    try { data = await this.searchProvider(query.trim()); }
    catch (error) {
      if (error.code === "SEARCH_VERIFICATION_REQUIRED") this.searchBlockedUntil = this.now() + 60000;
      throw error;
    }
    for (const [id, page] of this.pages) {
      if (page.expires <= this.now()) this.pages.delete(id);
    }
    // A result capability is short-lived and bound to one registered watch.
    while (this.pages.size > 500)
      this.pages.delete(this.pages.keys().next().value);
    const results = [];
    for (const item of data.slice(0, 5)) {
      let url;
      try {
        url = new URL(item.url);
      } catch {
        continue;
      }
      if (url.protocol !== "https:" || url.username || url.password) continue;
      const id = opaque(16);
      this.pages.set(id, {
        device,
        url: url.href,
        expires: this.now() + 300000,
      });
      results.push({
        id,
        title: plainText(item.title || "").slice(0, 160),
        snippet: plainText(item.description || "").slice(0, 400),
        url: url.href.slice(0, 200),
      });
    }
    const result = { provider: "DuckDuckGo", results };
    for (const [key, value] of this.searchCache) {
      if (value.expires <= this.now()) this.searchCache.delete(key);
    }
    while (this.searchCache.size >= 100) this.searchCache.delete(this.searchCache.keys().next().value);
    this.searchCache.set(cacheKey, {result,expires:this.now()+60000});
    return result;
  }
  resultUrl(device, id) {
    const item = this.pages.get(id);
    if (!item || item.device !== device || item.expires <= this.now()) throw new ApiError("PAGE_EXPIRED", 410);
    return item.url;
  }
  async page(device, id, page) {
    if (!Number.isInteger(page) || page < 0 || page > 255)
      throw new ApiError("INVALID_PAGE", 400);
    const item = this.pages.get(id);
    if (!item || item.device !== device || item.expires <= this.now())
      throw new ApiError("PAGE_EXPIRED", 410);
    this.charge(device, "page", 1, 100);
    const text = await this.readPage(item.url);
    const chars = Array.from(text);
    return {
      id,
      body: chars.slice(page * 2048, (page + 1) * 2048).join(""),
      page,
      has_more: chars.length > (page + 1) * 2048,
    };
  }
  async translate(device, { text, source = "auto", target = "en" }) {
    if (!this.config.translationKey)
      throw new ApiError("PROVIDER_NOT_CONFIGURED", 503);
    const length = typeof text === "string" ? Array.from(text).length : 0;
    if (
      length < 1 ||
      length > 1024 ||
      typeof source !== "string" ||
      typeof target !== "string" ||
      !/^(auto|[a-z]{2,3}(-[A-Z]{2})?)$/.test(source) ||
      !/^[a-z]{2,3}(-[A-Z]{2})?$/.test(target)
    )
      throw new ApiError("INVALID_TRANSLATION", 400);
    this.charge(device, "translation", length, 5000);
    this.charge("global", "translation", length, 50000);
    const data = await json(
      "https://translation.googleapis.com/language/translate/v2",
      {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "X-Goog-Api-Key": this.config.translationKey,
        },
        body: JSON.stringify({
          q: text,
          target,
          format: "text",
          ...(source === "auto" ? {} : { source }),
        }),
      },
    );
    const translated = data.data?.translations?.[0];
    if (typeof translated?.translatedText !== "string")
      throw new ApiError("UPSTREAM_INVALID_RESPONSE");
    // Translation entities are decoded as text, never interpreted as UI/HTML.
    return {
      provider: "Google Cloud Translation",
      text: plainText(translated.translatedText).slice(0, 8192),
      source: translated.detectedSourceLanguage || source,
      target,
    };
  }
}
