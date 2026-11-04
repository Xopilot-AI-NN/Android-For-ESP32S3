// QR pairing, browser OAuth and watch sessions share one state machine.
// Google refresh/access tokens are encrypted in SQLite and never returned to a watch.
import http from "node:http";
import https from "node:https";
import { readFileSync } from "node:fs";
import { timingSafeEqual } from "node:crypto";
import { pathToFileURL } from "node:url";
import { join } from "node:path";
import { homedir } from "node:os";
import { watchProviderKeys } from "./provider-config.mjs";
import { Store, opaque, hash } from "./store.mjs";
import { Google, ApiError, SCOPES } from "./google.mjs";
import { Browser } from "./browser.mjs";
import { Photos } from "./photos.mjs";
import { Providers } from "./providers.mjs";
import { config as loadConfig } from "./config.mjs";
const equal = (a, b) =>
  typeof a === "string" &&
  typeof b === "string" &&
  a.length === b.length &&
  timingSafeEqual(Buffer.from(a), Buffer.from(b));
const escape = (value) =>
  String(value).replace(
    /[&<>"']/g,
    (c) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[
        c
      ],
  );
function send(res, status, data, headers = {}) {
  const payload = typeof data === "string" ? data : JSON.stringify(data);
  res.writeHead(status, {
    "Content-Type": "application/json; charset=utf-8",
    "Content-Length": Buffer.byteLength(payload),
    "Cache-Control": "no-store",
    "Referrer-Policy": "no-referrer",
    "X-Content-Type-Options": "nosniff",
    "Content-Security-Policy":
      "default-src 'none'; form-action 'self' https://accounts.google.com; frame-ancestors 'none'; base-uri 'none'",
    ...headers,
  });
  res.end(payload);
}
function page(res, title, body, status = 200) {
  send(
    res,
    status,
    `<!doctype html><html lang="ru"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>${escape(title)}</title><h1>${escape(title)}</h1>${body}</html>`,
    { "Content-Type": "text/html; charset=utf-8" },
  );
}
async function body(req) {
  let size = 0;
  const parts = [];
  for await (const chunk of req) {
    size += chunk.length;
    if (size > 8192) throw new ApiError("BODY_TOO_LARGE", 413);
    parts.push(chunk);
  }
  try {
    return size ? JSON.parse(Buffer.concat(parts).toString("utf8")) : {};
  } catch {
    throw new ApiError("INVALID_JSON", 400);
  }
}
export function createGateway({
  config,
  store,
  google = new Google(config),
  providers,
  browser,
  now = () => Date.now(),
  tls,
}) {
  providers ||= new Providers(config, { now, store });
  browser ||= new Browser({providers,now});
  const photos = new Photos({store, google, now});
  const limits = new Map(),
    refreshing = new Map();
  function rate(key, max) {
    const at = now();
    let item = limits.get(key);
    if (!item || at - item.at > 60000) {
      item = { at, count: 0 };
      limits.set(key, item);
    }
    if (++item.count > max) throw new ApiError("RATE_LIMITED", 429);
    if (limits.size > 1000) {
      for (const [k, v] of limits) {
        if (at - v.at > 60000) limits.delete(k);
      }
      if (limits.size > 1000) throw new ApiError("RATE_LIMITED", 429);
    }
  }
  function device(req) {
    const id = req.headers["x-device-id"];
    const token = /^Device ([A-Za-z0-9_-]{32,128})$/.exec(
      req.headers.authorization || "",
    )?.[1];
    const row = typeof id === "string" ? store.device(id) : null;
    if (!row || !token || !equal(row.credential_hash, hash(token)))
      throw new ApiError("UNAUTHORIZED", 401);
    rate("device:" + id, 30);
    return row;
  }
  function session(req) {
    const id = req.headers["x-device-id"];
    const token = /^Bearer ([A-Za-z0-9_-]{32,128})$/.exec(
      req.headers.authorization || "",
    )?.[1];
    const row = token
      ? store.db
          .prepare("SELECT * FROM sessions WHERE token_hash=? AND expires>?")
          .get(hash(token), now())
      : null;
    if (!row || row.device !== id || !store.account(id))
      throw new ApiError("UNAUTHORIZED", 401);
    return row;
  }
  function pairing(id, owner) {
    const row = store.db.prepare("SELECT * FROM pairings WHERE id=?").get(id);
    if (!row) throw new ApiError("PAIRING_NOT_FOUND", 404);
    if (owner && row.device !== owner)
      throw new ApiError("PAIRING_NOT_FOUND", 404);
    if (row.expires <= now()) throw new ApiError("PAIRING_EXPIRED", 410);
    return row;
  }
  function cookie(value) {
    return `watch_oauth=${value}; HttpOnly; SameSite=Lax; Path=/oauth; Max-Age=300${config.dev || config.localIp ? "" : "; Secure"}`;
  }
  function issue(id) {
    const token = opaque();
    const expires = now() + 3600000;
    store.db
      .prepare("INSERT INTO sessions VALUES(?,?,?)")
      .run(hash(token), id, expires);
    return { session: token, expires_in: 3600 };
  }
  async function access(id, service) {
    const account = store.account(id);
    if (!account) throw new ApiError("REAUTH_REQUIRED", 401);
    if (!account.services.includes(service))
      throw new ApiError("SCOPE_MISSING", 403);
    if (account.tokens.expires_at > now() + 60000)
      return account.tokens.access_token;
    const refreshKey = id + ":" + (account.binding || account.profile.sub);
    if (!refreshing.has(refreshKey)) {
      refreshing.set(
        refreshKey,
        (async () => {
          try {
            const tokens = await google.refresh(account.tokens.refresh_token);
            if (typeof tokens.access_token !== "string")
              throw new ApiError("UPSTREAM_INVALID_RESPONSE");
            const current = store.account(id);
            if (
              !current ||
              current.profile.sub !== account.profile.sub ||
              current.binding !== account.binding ||
              current.tokens.refresh_token !== account.tokens.refresh_token
            )
              throw new ApiError("UNAUTHORIZED", 401);
            current.tokens = {
              ...current.tokens,
              ...tokens,
              refresh_token:
                tokens.refresh_token || current.tokens.refresh_token,
              expires_at:
                now() +
                Math.min(Number(tokens.expires_in) || 3600, 86400) * 1000,
            };
            if (typeof tokens.scope === "string") {
              const granted = new Set(tokens.scope.split(" "));
              current.services = current.services.filter((service) =>
                granted.has(SCOPES[service]),
              );
            }
            store.saveAccount(id, current);
            return current.tokens.access_token;
          } catch (error) {
            const current = store.account(id);
            if (
              error.code === "REAUTH_REQUIRED" &&
              current?.binding === account.binding &&
              current?.profile.sub === account.profile.sub &&
              current?.tokens.refresh_token === account.tokens.refresh_token
            )
              store.unlink(id);
            throw error;
          } finally {
            refreshing.delete(refreshKey);
          }
        })(),
      );
    }
    const token = await refreshing.get(refreshKey);
    if (!store.account(id)?.services.includes(service))
      throw new ApiError("SCOPE_MISSING", 403);
    return token;
  }
  async function readService(id, service, read) {
    const binding = store.account(id)?.binding;
    try {
      return await read(await access(id, service));
    } catch (error) {
      if (error.code === "SCOPE_MISSING") {
        const current = store.account(id);
        if (current && current.binding === binding) {
          current.services = current.services.filter(
            (value) => value !== service,
          );
          store.saveAccount(id, current);
        }
      }
      throw error;
    }
  }
  const handler = async (req, res) => {
    const requestId = opaque(6);
    try {
      // Only Caddy on loopback may terminate HTTPS. Never trust a remote Forwarded header.
      // Loopback HTTP serves browser login only. Device credentials always use HTTPS.
      const browserLocal = config.localIp && !req.socket.encrypted &&
        req.socket.remoteAddress === "127.0.0.1" && req.headers.host === "localhost:8080";
      if (config.localIp && !req.socket.encrypted && !browserLocal)
        throw new ApiError("HTTPS_REQUIRED", 400);
      if (browserLocal && req.url !== "/" && !/^\/(?:p\/[A-Za-z0-9_-]{16,64}(?:\/start)?|oauth\/callback)(?:\?|$)/.test(req.url))
        throw new ApiError("LOCAL_BROWSER_ONLY", 403);
      if (!config.dev && !browserLocal) {
        const local = ["127.0.0.1", "::1", "::ffff:127.0.0.1"].includes(
          req.socket.remoteAddress,
        );
        if (
          !req.socket.encrypted &&
          !(local && req.headers["x-forwarded-proto"] === "https")
        )
          throw new ApiError("HTTPS_REQUIRED", 400);
      }
      rate("ip:" + req.socket.remoteAddress, 120);
      store.db
        .prepare(
          "UPDATE pairings SET status='expired',pending=NULL,verifier=NULL,state_hash=NULL,cookie_hash=NULL WHERE expires<=? AND status NOT IN ('expired','linked')",
        )
        .run(now());
      store.db
        .prepare("DELETE FROM pairings WHERE expires<?")
        .run(now() - 86400000);
      store.db.prepare("DELETE FROM sessions WHERE expires<=?").run(now());
      const url = new URL(req.url, config.base),
        path = url.pathname,
        method = req.method;
      if (path === "/health" && method === "GET") {
        send(res, 200, { status: "ok", version: 1 });
        return;
      }
      if (path === "/" && method === "GET" && browserLocal) {
        const pending = store.db.prepare(
          "SELECT p.id,d.name FROM pairings p JOIN devices d ON d.id=p.device WHERE p.status='waiting_for_phone' AND p.expires>?",
        ).all(now());
        const pickers = store.db.prepare("SELECT id,name FROM devices WHERE account IS NOT NULL").all()
          .map(device => ({...device, picker:store.account(device.id)?.picker}))
          .filter(item => item.picker && !item.picker.selected && item.picker.expires > now());
        const pickerLinks = pickers.map(item => `<p><a href="${escape(item.picker.uri)}">Выбрать Google Фото: ${escape(item.name)}</a></p>`).join("");
        page(res, "Локальный вход Google", pickerLinks + (pending.length
          ? "<p>Выберите запрос с ваших часов:</p><ul>" + pending.map(row =>
            `<li><a href="/p/${escape(row.id)}">${escape(row.name)}</a></li>`).join("") + "</ul>"
          : "<p>Откройте Google на часах и начните подключение аккаунта. Затем обновите эту страницу.</p>"));
        return;
      }
      if (path === "/" && method === "GET") {
        page(
          res,
          "AOSP Wear Gateway",
          "<p>Сервис готов. Создайте привязку с часов или через локальный admin.mjs. Пароль Google вводится только на accounts.google.com.</p>",
        );
        return;
      }
      if (path === "/api/v1/google/pairings" && method === "POST") {
        const owner = device(req),
          data = await body(req);
        if (data.device_id && data.device_id !== owner.id)
          throw new ApiError("UNAUTHORIZED", 401);
        const services = data.services ?? ["calendar"];
        if (
          !Array.isArray(services) ||
          services.length > 4 ||
          !services.length ||
          services.some((s) => !Object.hasOwn(SCOPES, s))
        )
          throw new ApiError("INVALID_SERVICES", 400);
        const id = opaque(16);
        store.db
          .prepare(
            "INSERT INTO pairings(id,device,expires,status,services) VALUES(?,?,?,?,?)",
          )
          .run(
            id,
            owner.id,
            now() + 300000,
            "waiting_for_phone",
            JSON.stringify([...new Set(services)]),
          );
        store.audit("pairing.created", owner.id, now());
        send(res, 201, {
          pairing_id: id,
          qr_url: config.base + "/p/" + id,
          expires_in: 300,
          poll_after: 3,
        });
        return;
      }
      const publicPair = /^\/p\/([A-Za-z0-9_-]{16,64})(\/start)?$/.exec(path);
      if (publicPair) {
        if (config.localIp && !browserLocal) {
          page(res, "Локальный вход Google",
            `<p>Откройте эту ссылку в браузере на компьютере с Gateway:</p><p>${escape("http://localhost:8080" + path)}</p><p>После входа подтвердите аккаунт на часах.</p>`);
          return;
        }
        const pair = pairing(publicPair[1]);
        if (method === "GET" && !publicPair[2]) {
          if (pair.status !== "waiting_for_phone") {
            page(
              res,
              "Подтверждение на часах",
              "<p>Ожидается подтверждение аккаунта на часах. Эта ссылка не выполняет привязку автоматически.</p>",
            );
            return;
          }
          const owner = store.device(pair.device);
          page(
            res,
            "Подключить Google",
            `<p>Устройство: ${escape(owner.name)}</p><form method="post" action="/p/${escape(pair.id)}/start"><button>Продолжить в Google</button></form><p>После входа подтвердите адрес аккаунта на часах.</p>`,
          );
          return;
        }
        if (method === "POST" && publicPair[2]) {
          if (pair.status !== "waiting_for_phone")
            throw new ApiError("PAIRING_ALREADY_USED", 409);
          const state = opaque(),
            browser = opaque(),
            verifier = opaque(48);
          store.db
            .prepare(
              "UPDATE pairings SET state_hash=?,cookie_hash=?,verifier=? WHERE id=?",
            )
            .run(
              hash(state),
              hash(browser),
              store.seal(pair.device, { verifier }),
              pair.id,
            );
          const challenge = Buffer.from(hash(verifier), "hex").toString(
            "base64url",
          );
          const auth = new URL("https://accounts.google.com/o/oauth2/v2/auth");
          auth.search = new URLSearchParams({
            client_id: config.clientId,
            redirect_uri: config.oauthRedirect || config.base + "/oauth/callback",
            response_type: "code",
            scope:
              "openid email profile " +
              JSON.parse(pair.services)
                .map((s) => SCOPES[s])
                .join(" "),
            access_type: "offline",
            prompt: "consent",
            state,
            code_challenge: challenge,
            code_challenge_method: "S256",
          }).toString();
          send(res, 303, "", {
            Location: auth.href,
            "Set-Cookie": cookie(browser),
          });
          return;
        }
      }
      if (path === "/oauth/callback" && method === "GET") {
        const state = url.searchParams.get("state") || "",
          browser = /(?:^|;\s*)watch_oauth=([A-Za-z0-9_-]+)/.exec(
            req.headers.cookie || "",
          )?.[1];
        const row = store.db
          .prepare("SELECT * FROM pairings WHERE state_hash=?")
          .get(hash(state));
        if (!row || !browser || !equal(row.cookie_hash, hash(browser)))
          throw new ApiError("INVALID_OAUTH_STATE", 403);
        const pair = pairing(row.id);
        if (pair.status !== "waiting_for_phone")
          throw new ApiError("OAUTH_STATE_ALREADY_USED", 409);
        // Consume state before awaiting the network: a repeated callback cannot exchange twice.
        store.db
          .prepare(
            "UPDATE pairings SET status='exchanging',state_hash=NULL,cookie_hash=NULL WHERE id=?",
          )
          .run(pair.id);
        if (url.searchParams.has("error")) {
          store.db
            .prepare(
              "UPDATE pairings SET status='denied',verifier=NULL WHERE id=?",
            )
            .run(pair.id);
          page(
            res,
            "Вход отменён",
            "<p>Аккаунт не подключён. Повторите вход с часов.</p>",
          );
          return;
        }
        const code = url.searchParams.get("code");
        if (!code || code.length > 4096)
          throw new ApiError("INVALID_CODE", 400);
        const { verifier } = store.open(pair.device, pair.verifier);
        const tokens = await google.exchange(code, verifier);
        if (
          typeof tokens.access_token !== "string" ||
          tokens.access_token.length > 8192
        )
          throw new ApiError("UPSTREAM_INVALID_RESPONSE");
        const profile = await google.profile(tokens.access_token);
        if (
          typeof profile.sub !== "string" ||
          profile.sub.length > 255 ||
          !profile.email ||
          profile.email_verified !== true
        )
          throw new ApiError("UNVERIFIED_IDENTITY", 403);
        const old = store.account(pair.device);
        tokens.refresh_token ||=
          old?.profile.sub === profile.sub ? old.tokens.refresh_token : null;
        if (!tokens.refresh_token) throw new ApiError("REAUTH_REQUIRED", 401);
        const granted = new Set(String(tokens.scope || "").split(" "));
        const services = JSON.parse(pair.services).filter((s) =>
          granted.has(SCOPES[s]),
        );
        const account = {
          binding: opaque(16),
          profile: {
            sub: profile.sub,
            email: String(profile.email).slice(0, 254),
            display_name: String(profile.name || "").slice(0, 100),
          },
          tokens: {
            ...tokens,
            expires_at:
              now() + Math.min(Number(tokens.expires_in) || 3600, 86400) * 1000,
          },
          services,
        };
        delete account.tokens.id_token;
        // A valid Google login is still only pending. Device confirmation is mandatory.
        if (pairing(pair.id).status !== "exchanging")
          throw new ApiError("PAIRING_ALREADY_USED", 409);
        store.db
          .prepare(
            "UPDATE pairings SET status='awaiting_watch_confirmation',pending=?,verifier=NULL WHERE id=?",
          )
          .run(store.seal(pair.device, account), pair.id);
        store.audit("oauth.pending_confirmation", pair.device, now());
        page(
          res,
          "Подтвердите на часах",
          "<p>Вход выполнен. Проверьте адрес аккаунта на часах и подтвердите привязку там.</p>",
        );
        return;
      }
      const pairRoute =
        /^\/api\/v1\/google\/pairings\/([A-Za-z0-9_-]{16,64})(\/confirm)?$/.exec(
          path,
        );
      if (pairRoute) {
        const owner = device(req),
          pair = pairing(pairRoute[1], owner.id);
        if (method === "DELETE" && !pairRoute[2]) {
          if (pair.status === "linked")
            throw new ApiError("PAIRING_ALREADY_USED", 409);
          store.db
            .prepare(
              "UPDATE pairings SET status='cancelled',pending=NULL,verifier=NULL,state_hash=NULL,cookie_hash=NULL WHERE id=?",
            )
            .run(pair.id);
          send(res, 200, { status: "cancelled" });
          return;
        }
        if (method === "GET" && !pairRoute[2]) {
          const pending = pair.pending
            ? store.open(owner.id, pair.pending)
            : null;
          send(res, 200, {
            status: pair.status,
            ...(pending
              ? { account: pending.profile, granted_services: pending.services }
              : {}),
          });
          return;
        }
        if (method === "POST" && pairRoute[2]) {
          if (pair.status !== "awaiting_watch_confirmation" || !pair.pending)
            throw new ApiError("PAIRING_NOT_CONFIRMABLE", 409);
          const account = store.open(owner.id, pair.pending);
          store.db.exec("BEGIN IMMEDIATE");
          let session;
          try {
            // Replacing an account invalidates sessions issued for the previous identity.
            store.db
              .prepare("DELETE FROM sessions WHERE device=?")
              .run(owner.id);
            store.db
              .prepare(
                "UPDATE pairings SET status='cancelled',pending=NULL,verifier=NULL,state_hash=NULL,cookie_hash=NULL WHERE device=? AND id<>? AND status<>'linked'",
              )
              .run(owner.id, pair.id);
            store.saveAccount(owner.id, account);
            session = issue(owner.id);
            store.db
              .prepare(
                "UPDATE pairings SET status='linked',pending=NULL,verifier=NULL,state_hash=NULL,cookie_hash=NULL WHERE id=?",
              )
              .run(pair.id);
            store.db.exec("COMMIT");
          } catch (error) {
            store.db.exec("ROLLBACK");
            throw error;
          }
          store.audit("account.linked", owner.id, now());
          send(res, 200, { status: "linked", ...session });
          return;
        }
      }
      if (path === "/api/v1/browser/open" && method === "POST") {
        const owner=device(req);rate("browser:"+owner.id,30);
        send(res,200,await browser.open(owner.id,await body(req)));return;
      }
      const documentRoute=/^\/api\/v1\/browser\/documents\/([A-Za-z0-9_-]{16,64})(?:\/images\/(\d+))?$/.exec(path);
      if (documentRoute && method === "GET") {
        const owner=device(req);rate("browser:"+owner.id,30);
        send(res,200,documentRoute[2]!==undefined
          ? await browser.image(owner.id,documentRoute[1],Number(documentRoute[2]))
          : browser.page(owner.id,documentRoute[1],Number(url.searchParams.get("page")||0)));
        return;
      }
      // Independent search/translation require device auth, not a personal Google login.
      if (path === "/api/v1/search" && method === "GET") {
        const owner = device(req);
        rate("provider:" + owner.id, 20);
        send(
          res,
          200,
          await providers.search(owner.id, url.searchParams.get("q")),
        );
        return;
      }
      const searchPage =
        /^\/api\/v1\/search\/pages\/([A-Za-z0-9_-]{16,64})$/.exec(path);
      if (searchPage && method === "GET") {
        const owner = device(req);
        rate("provider:" + owner.id, 20);
        send(
          res,
          200,
          await providers.page(
            owner.id,
            searchPage[1],
            Number(url.searchParams.get("page") || 0),
          ),
        );
        return;
      }
      if (path === "/api/v1/translate" && method === "POST") {
        const owner = device(req);
        rate("provider:" + owner.id, 20);
        send(res, 200, await providers.translate(owner.id, await body(req)));
        return;
      }
      // Logout by the private device credential works even after the short session expires.
      if (path === "/api/v1/google/account" && method === "DELETE") {
        const owner = device(req);
        const account = store.account(owner.id);
        store.unlink(owner.id);
        if (account) await google.revoke(account.tokens.refresh_token);
        store.audit("account.unlinked", owner.id, now());
        send(res, 200, { status: "not_connected" });
        return;
      }
      // A provisioned device can renew its short session without logging in on a phone again.
      if (path === "/api/v1/google/session" && method === "POST") {
        const owner = device(req);
        if (!store.account(owner.id))
          throw new ApiError("REAUTH_REQUIRED", 401);
        send(res, 200, issue(owner.id));
        return;
      }
      if (path.startsWith("/api/v1/google/")) {
        const active = session(req);
        rate("session:" + active.device, 30);
        const account = store.account(active.device);
        if (path === "/api/v1/google/me" && method === "GET") {
          send(res, 200, {
            account: account.profile,
            granted_services: account.services,
            last_sync: account.last_sync || null,
          });
          return;
        }
        if (path === "/api/v1/google/session" && method === "DELETE") {
          store.unlink(active.device);
          await google.revoke(account.tokens.refresh_token);
          store.audit("account.unlinked", active.device, now());
          send(res, 200, { status: "not_connected" });
          return;
        }
        let result;
        if (path === "/api/v1/google/calendar/events" && method === "GET")
          result = await readService(active.device, "calendar", (token) =>
            google.events(token, url.searchParams),
          );
        else if (path === "/api/v1/google/contacts" && method === "GET")
          result = await readService(active.device, "contacts", token => google.contacts(token, url.searchParams));
        else if (path === "/api/v1/google/photos/picker" && method === "POST")
          result = await readService(active.device, "photos", token => photos.create(active.device, token));
        else if (path === "/api/v1/google/photos" && method === "GET")
          result = await readService(active.device, "photos", token => photos.list(active.device, token, url.searchParams));
        else if (/^\/api\/v1\/google\/photos\/items\/[A-Za-z0-9_-]{32}$/.test(path) && method === "GET")
          result = await readService(active.device, "photos", token => photos.image(active.device, token, path.split("/").at(-1)));
        else if (path === "/api/v1/google/gmail/messages" && method === "GET")
          result = await readService(active.device, "gmail", (token) =>
            google.messages(token, url.searchParams),
          );
        else if (
          /^\/api\/v1\/google\/gmail\/messages\/[\w-]+$/.test(path) &&
          method === "GET"
        )
          result = await readService(active.device, "gmail", (token) =>
            google.message(token, path.split("/").at(-1), url.searchParams),
          );
        else if (path === "/api/v1/google/sync" && method === "POST") {
          result = {};
          if (account.services.includes("calendar"))
            result.calendar = await readService(
              active.device,
              "calendar",
              (token) =>
                google.events(token, new URLSearchParams({ limit: "3" })),
            );
          if (account.services.includes("gmail"))
            result.gmail = await readService(active.device, "gmail", (token) =>
              google.messages(token, new URLSearchParams({ limit: "3" })),
            );
        } else throw new ApiError("NOT_FOUND", 404);
        session(req);
        const current = store.account(active.device);
        current.last_sync = new Date(now()).toISOString();
        store.saveAccount(active.device, current);
        send(res, 200, { ...result, synced_at: current.last_sync });
        return;
      }
      throw new ApiError("NOT_FOUND", 404);
    } catch (error) {
      const known = error instanceof ApiError;
      send(res, known ? error.status : 500, {
        error: known ? error.code : "INTERNAL_ERROR",
        request_id: requestId,
      }); /* No URLs, bodies, tokens, emails or raw upstream errors in console logs. */
    }
  };
  const server = tls ? https.createServer(tls, handler) : http.createServer(handler);
  server.requestTimeout = 15000;
  server.headersTimeout = 10000;
  return server;
}
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  const cfg = loadConfig(),
    store = new Store(join(cfg.data, "gateway.sqlite"), cfg.key);
  const providerFile = process.env.GATEWAY_CONFIG_FILE || join(process.env.XDG_CONFIG_HOME || join(homedir(), ".config"), "aosp-wear-gateway", "config.env");
  watchProviderKeys(providerFile, cfg);
  const tls = cfg.localIp ? { cert: readFileSync(cfg.tlsCert), key: readFileSync(cfg.tlsKey) } : undefined;
  const server = createGateway({ config: cfg, store, tls });
  if (cfg.localIp) {
    // Reuse the state machine, but expose only the browser routes over loopback HTTP.
    const browser = http.createServer(server.listeners("request")[0]);
    browser.requestTimeout = 15000;
    browser.headersTimeout = 10000;
    browser.listen(8080, "127.0.0.1");
  }
  server.listen(cfg.port, cfg.localIp || "127.0.0.1", () =>
    console.log(
      `Gateway listening on ${cfg.localIp || "127.0.0.1"}:${cfg.port}; provider key reload enabled; request bodies and tokens are never logged`,
    ),
  );
}
