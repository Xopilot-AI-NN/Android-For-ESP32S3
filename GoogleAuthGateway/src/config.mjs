import { homedir } from "node:os";
import { join, resolve, dirname, sep } from "node:path";
import { existsSync, realpathSync } from "node:fs";
import { fileURLToPath } from "node:url";
// Refuse runtime state inside the checkout, even if a custom env path bypasses .gitignore.
function canonical(path) {
  const tail = [];
  let at = resolve(path);
  while (!existsSync(at)) {
    tail.unshift(at.split(sep).at(-1));
    const parent = dirname(at);
    if (parent === at) break;
    at = parent;
  }
  return resolve(realpathSync(at), ...tail);
}
function repository() {
  let at = dirname(fileURLToPath(import.meta.url));
  while (dirname(at) !== at) {
    if (existsSync(join(at, ".git"))) return realpathSync(at);
    at = dirname(at);
  }
  return resolve(dirname(fileURLToPath(import.meta.url)), "..");
}
export function config(env = process.env) {
  if (env.NODE_TLS_REJECT_UNAUTHORIZED === "0")
    throw new Error("Insecure Node TLS configuration is not supported");
  const base = (env.PUBLIC_BASE_URL || "").replace(/\/$/, "");
  let url;
  try {
    url = new URL(base);
  } catch {
    throw new Error("Set PUBLIC_BASE_URL using configure.py");
  }
  // A stale development flag must never disable HTTPS checks on a public origin.
  const dev =
    env.DEV_LOCALHOST === "1" &&
    url.protocol === "http:" &&
    url.hostname === "localhost";
  if (
    url.username ||
    url.password ||
    url.search ||
    url.hash ||
    url.pathname !== "/" ||
    !(url.protocol === "https:" || dev)
  )
    throw new Error(
      "HTTPS origin required; HTTP is restricted to explicit localhost tests",
    );
  if (!env.GOOGLE_CLIENT_ID || !env.GOOGLE_CLIENT_SECRET)
    throw new Error("Import Google Web OAuth credentials with configure.py");
  // Validate here too: the check command must fail before an unusable store is opened.
  if (!/^[a-fA-F0-9]{64}$/.test(env.STORE_KEY || ""))
    throw new Error(
      "STORE_KEY must contain 32 bytes encoded as 64 hexadecimal characters",
    );
  const port = Number(env.PORT || 8080);
  if (!Number.isInteger(port) || port < 1 || port > 65535)
    throw new Error("Invalid PORT");
  const data =
    env.GATEWAY_DATA_DIR ||
    join(
      env.XDG_STATE_HOME || join(homedir(), ".local/state"),
      "aosp-wear-gateway",
    );
  const destination = canonical(data),
    repo = repository();
  if (destination === repo || destination.startsWith(repo + sep))
    throw new Error("Runtime account data must be outside the Git checkout");
  return {
    base,
    dev,
    port,
    clientId: env.GOOGLE_CLIENT_ID,
    clientSecret: env.GOOGLE_CLIENT_SECRET,
    key: env.STORE_KEY || "",
    data: destination,
    searchKey: env.BRAVE_SEARCH_API_KEY || "",
    translationKey: env.GOOGLE_TRANSLATE_API_KEY || "",
  };
}
