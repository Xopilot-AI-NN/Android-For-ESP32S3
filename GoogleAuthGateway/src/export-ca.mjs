// Export a trust anchor ONLY after Node has verified the real HTTPS server.
// Downloading an unverified peer certificate and trusting it would permit MITM.
import tls from "node:tls";
import { X509Certificate, randomBytes } from "node:crypto";
import { readFileSync, writeFileSync, renameSync, realpathSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { config } from "./config.mjs";
async function main() {
  const cfg = config(),
    url = new URL(cfg.base);
  if (url.protocol !== "https:") throw Error("HTTPS required");
  const directory = realpathSync(
    join(
      process.env.XDG_CONFIG_HOME || join(homedir(), ".config"),
      "aosp-wear-gateway",
    ),
  );
  const repository = resolve(fileURLToPath(new URL("../..", import.meta.url)));
  if (directory === repository || directory.startsWith(repository + sep))
    throw Error("Private config directory must be outside Git");
  const peer = await new Promise((resolve, reject) => {
    const socket = tls.connect(
      {
        host: url.hostname,
        port: Number(url.port || 443),
        servername: url.hostname,
        rejectUnauthorized: true,
        minVersion: "TLSv1.2",
      },
      () => {
        if (!socket.authorized) {
          socket.destroy();
          reject(Error("Unverified server"));
          return;
        }
        const certificate = socket.getPeerCertificate(true);
        socket.destroy();
        resolve(certificate);
      },
    );
    socket.setTimeout(10000, () => socket.destroy(Error("TLS timeout")));
    socket.on("error", reject);
  });
  let last = peer;
  for (
    let depth = 0;
    depth < 10 &&
    last.issuerCertificate &&
    last.issuerCertificate.fingerprint256 !== last.fingerprint256;
    depth++
  )
    last = last.issuerCertificate;
  let root = new X509Certificate(last.raw);
  {
    const roots = [
      ...(typeof tls.getCACertificates === "function"
        ? tls.getCACertificates("default")
        : tls.rootCertificates),
    ];
    if (process.env.NODE_EXTRA_CA_CERTS)
      roots.push(
        ...(readFileSync(process.env.NODE_EXTRA_CA_CERTS, "utf8").match(
          /-----BEGIN CERTIFICATE-----[\s\S]+?-----END CERTIFICATE-----/g,
        ) || []),
      );
    const issuer = roots
      .map((pem) => new X509Certificate(pem))
      .find(
        (candidate) =>
          candidate.ca &&
          (candidate.fingerprint256 === root.fingerprint256 ||
            (root.checkIssued(candidate) && root.verify(candidate.publicKey))),
      );
    if (!issuer) throw Error("Trust anchor unavailable");
    root = issuer;
  }
  const pem = root.toString();
  if (Buffer.byteLength(pem) > 3000)
    throw Error("Trust anchor exceeds watch limit");
  const temporary = join(directory, ".ca-" + randomBytes(8).toString("hex"));
  writeFileSync(temporary, pem + "\n", { mode: 0o600, flag: "wx" });
  const target = join(directory, "gateway-ca.pem");
  renameSync(temporary, target);
  console.log("Verified public CA saved outside Git:", target);
  console.log("CA SHA-256:", root.fingerprint256);
}
main().catch(() => {
  console.error(
    "Cannot verify/export Gateway CA. Check the HTTPS address and trusted certificate chain; no insecure fallback was used.",
  );
  process.exitCode = 1;
});
