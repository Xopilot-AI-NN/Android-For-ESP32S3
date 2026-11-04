import { execFileSync } from "node:child_process";
import { config } from "./config.mjs";
import { DatabaseSync } from "node:sqlite";
config();
const database = new DatabaseSync(":memory:");
database.close();
try {
  execFileSync(process.env.GATEWAY_PYTHON || "python3", ["-c", "from PIL import Image; import html.parser"], {stdio:"ignore",timeout:5000});
} catch { throw new Error("Install Python 3 and Pillow for mail HTML and compressed previews"); }
console.log("Node/SQLite/config/Python/Pillow OK; credentials stay outside Git");
