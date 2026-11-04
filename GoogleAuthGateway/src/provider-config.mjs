// Only provider keys are reloaded. OAuth identity, STORE_KEY and listeners need an explicit restart.
import { watch, readFileSync } from "node:fs";
import { basename, dirname } from "node:path";
export function watchProviderKeys(file, config) {
  function reload() {
    try {
      const text = readFileSync(file, "utf8");
      if (Buffer.byteLength(text) > 16384) return;
      const next = { translationKey: "" };
      const names = { GOOGLE_TRANSLATE_API_KEY: "translationKey" };
      for (const line of text.split("\n")) {
        const split = line.indexOf("=");
        const name = line.slice(0, split);
        if (split < 1 || !Object.hasOwn(names, name)) continue;
        const field = names[name];
        const value = JSON.parse(line.slice(split + 1));
        if (typeof value !== "string" || value.length > 256 || /[\r\n\0]/.test(value)) return;
        next[field] = value;
      }
      Object.assign(config, next);
    } catch {
      // Atomic imports are retried on the next directory event. Never log a malformed secret.
    }
  }
  const watcher = watch(dirname(file), (_event, name) => {
    if (!name || String(name) === basename(file)) reload();
  });
  watcher.on("error", () => {});
  watcher.unref();
  return watcher;
}
