// Local operator tool. Device/session credentials are written to mode-0600 files, not printed.
import { writeFileSync, readFileSync, mkdirSync, unlinkSync } from "node:fs";
import { join, dirname } from "node:path";
import { config } from "./config.mjs";
import { Store } from "./store.mjs";
const cfg = config(),
  [action, ...args] = process.argv.slice(2);
function save(file, value) {
  mkdirSync(dirname(file), { recursive: true, mode: 0o700 });
  writeFileSync(file, JSON.stringify(value, null, 2) + "\n", {
    mode: 0o600,
    flag: "wx",
  });
}
async function request(route, credential, method = "GET", payload) {
  const response = await fetch(cfg.base + route, {
    method,
    redirect: "error",
    signal: AbortSignal.timeout(15000),
    headers: {
      "X-Device-ID": credential.device_id,
      Authorization: credential.session
        ? "Bearer " + credential.session
        : "Device " + credential.device_credential,
      "Content-Type": "application/json",
    },
    ...(payload ? { body: JSON.stringify(payload) } : {}),
  });
  const result = await response.json();
  if (!response.ok) throw new Error(result.error || "Request rejected");
  return result;
}
try {
  if (action === "register") {
    const store = new Store(join(cfg.data, "gateway.sqlite"), cfg.key);
    try {
      const credential = store.register(args[0] || "AOSP Watch");
      const file = join(
        cfg.data,
        "devices",
        credential.device_id + ".device.json",
      );
      save(file, credential);
      console.log("Private device file (keep outside Git):", file);
    } finally {
      store.close();
    }
  } else if (["pair", "poll", "confirm", "renew", "logout"].includes(action)) {
    if (!args[0])
      throw new Error("Specify the private device/session JSON path");
    const credential = JSON.parse(readFileSync(args[0], "utf8"));
    if (action === "pair") {
      const services = (args[1] || "calendar").split(",");
      const result = await request(
        "/api/v1/google/pairings",
        credential,
        "POST",
        { device_id: credential.device_id, services },
      );
      console.log("Open this one-time URL in your browser:", cfg.localIp ? "http://localhost:8080/p/" + result.pairing_id : result.qr_url);
      console.log("Pairing ID:", result.pairing_id);
      console.log(
        "Expires in 5 minutes. Localhost works only on the phone or PC running Gateway; another device needs HTTPS.",
      );
    }
    if (action === "poll") {
      if (!args[1]) throw new Error("Specify pairing ID");
      const result = await request(
        "/api/v1/google/pairings/" + encodeURIComponent(args[1]),
        credential,
      );
      console.log("Status:", result.status);
      if (result.account) {
        console.log(
          "Check account on this local terminal:",
          result.account.email,
        );
        console.log("Services:", result.granted_services.join(", "));
      }
    }
    if (action === "confirm" || action === "renew") {
      if (action === "confirm" && !args[1])
        throw new Error("Specify the pairing ID after checking the account");
      const result = await request(
        action === "confirm"
          ? "/api/v1/google/pairings/" +
              encodeURIComponent(args[1]) +
              "/confirm"
          : "/api/v1/google/session",
        credential,
        "POST",
        {},
      );
      const file = join(
        cfg.data,
        "devices",
        credential.device_id + ".session." + Date.now() + ".json",
      );
      save(file, { device_id: credential.device_id, session: result.session });
      console.log("Private session file:", file);
    }
    if (action === "logout") {
      await request("/api/v1/google/session", credential, "DELETE");
      unlinkSync(args[0]);
      console.log("Server account unlinked; local session file removed");
    }
  } else {
    console.log(
      "Usage: register [name] | pair DEVICE_JSON [calendar,gmail] | poll DEVICE_JSON PAIR_ID | confirm DEVICE_JSON PAIR_ID | renew DEVICE_JSON | logout SESSION_JSON",
    );
    process.exitCode = 2;
  }
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
