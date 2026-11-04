// Import a private JSON file without exposing keys in arguments, logs or the checkout.
import {
  readFileSync,
  writeFileSync,
  renameSync,
  chmodSync,
  realpathSync,
} from "node:fs";
import { join, resolve, sep } from "node:path";
import { homedir } from "node:os";
import { randomBytes } from "node:crypto";
import { fileURLToPath } from "node:url";
try {
  const directory = join(
    process.env.XDG_CONFIG_HOME || join(homedir(), ".config"),
    "aosp-wear-gateway",
  );
  const repository = resolve(fileURLToPath(new URL("../..", import.meta.url)));
  const destination = realpathSync(directory);
  if (destination === repository || destination.startsWith(repository + sep))
    throw Error("Private config must be outside Git");
  const values = JSON.parse(readFileSync(process.argv[2], "utf8"));
  const allowed = {
    brave_search_api_key: "BRAVE_SEARCH_API_KEY",
    google_translate_api_key: "GOOGLE_TRANSLATE_API_KEY",
  };
  if (Object.keys(values).some((key) => !Object.hasOwn(allowed, key)))
    throw Error(
      "Use google_translate_api_key; DuckDuckGo search needs no key",
    );
  let contents = readFileSync(join(directory, "config.env"), "utf8");
  for (const [key, name] of Object.entries(allowed)) {
    if (!Object.hasOwn(values, key)) continue;
    // Accept old private JSON files without ever using or importing a paid search key.
    if (key === "brave_search_api_key") continue;
    const value = values[key];
    if (
      typeof value !== "string" ||
      value.length > 256 ||
      /[\r\n\0]/.test(value)
    )
      throw Error("Invalid API key");
    contents =
      contents
        .split("\n")
        .filter((line) => !line.startsWith(name + "="))
        .join("\n")
        .trimEnd() +
      "\n" +
      name +
      "=" +
      JSON.stringify(value) +
      "\n";
  }
  const temporary = join(
    directory,
    ".providers-" + randomBytes(8).toString("hex"),
  );
  writeFileSync(temporary, contents, { mode: 0o600, flag: "wx" });
  chmodSync(temporary, 0o600);
  renameSync(temporary, join(directory, "config.env"));
  console.log("Provider keys saved outside Git. A running Gateway reloads these keys automatically.");
} catch {
  console.error(
    "Provider import failed: check the private JSON path, fields and config directory. Values were not logged.",
  );
  process.exitCode = 1;
}
