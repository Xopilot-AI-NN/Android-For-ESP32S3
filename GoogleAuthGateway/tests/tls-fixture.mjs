// Used by the Rust host harness only: ephemeral certificates/keys stay in a private temp dir.
import https from "node:https";
import { readFileSync } from "node:fs";
const server = https.createServer(
  { cert: readFileSync(process.argv[2]), key: readFileSync(process.argv[3]) },
  (req, res) => {
    if (req.url === "/browser") {
      if (req.headers.authorization || req.headers.cookie || req.headers["x-device-id"]) {
        res.writeHead(403); res.end("Credential leak"); return;
      }
      res.writeHead(200, {"Content-Type": "text/html; charset=utf-8"});
      res.end("<h1>Direct browser</h1>"); return;
    }
    const body = JSON.stringify({ ok: true });
    res.writeHead(200, {
      "Content-Type": "application/json",
      "Content-Length": Buffer.byteLength(body),
    });
    res.end(body);
  },
);
server.on("tlsClientError", () => {});
server.listen(0, "127.0.0.1", () => console.log(server.address().port));
