import { config } from "./config.mjs";
import { DatabaseSync } from "node:sqlite";
config();
const database = new DatabaseSync(":memory:");
database.close();
console.log("Node/SQLite/config OK; credentials stay outside Git");
