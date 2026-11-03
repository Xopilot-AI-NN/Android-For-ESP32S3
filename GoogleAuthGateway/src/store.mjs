// Runtime data lives outside the checkout. Only source and schema may be published.
import { DatabaseSync } from "node:sqlite";
import { mkdirSync, chmodSync } from "node:fs";
import { dirname } from "node:path";
import {
  randomBytes,
  createHash,
  createCipheriv,
  createDecipheriv,
} from "node:crypto";
export const opaque = (size = 32) => randomBytes(size).toString("base64url");
export const hash = (value) => createHash("sha256").update(value).digest("hex");
export class Store {
  constructor(file, key) {
    if (!/^[0-9a-f]{64}$/i.test(key))
      throw new Error("STORE_KEY must contain 32 random bytes in hex");
    this.key = Buffer.from(key, "hex");
    process.umask(0o077);
    if (file !== ":memory:") {
      mkdirSync(dirname(file), { recursive: true, mode: 0o700 });
      chmodSync(dirname(file), 0o700);
    }
    this.db = new DatabaseSync(file);
    if (file !== ":memory:") chmodSync(file, 0o600);
    this.db.exec(`PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;
      CREATE TABLE IF NOT EXISTS devices(id TEXT PRIMARY KEY,name TEXT NOT NULL,credential_hash TEXT NOT NULL,account TEXT);
      CREATE TABLE IF NOT EXISTS pairings(id TEXT PRIMARY KEY,device TEXT NOT NULL REFERENCES devices(id),expires INTEGER NOT NULL,status TEXT NOT NULL,services TEXT NOT NULL,state_hash TEXT,cookie_hash TEXT,verifier TEXT,pending TEXT);
      CREATE TABLE IF NOT EXISTS sessions(token_hash TEXT PRIMARY KEY,device TEXT NOT NULL REFERENCES devices(id),expires INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS audit(time INTEGER NOT NULL,event TEXT NOT NULL,device TEXT);
      CREATE TABLE IF NOT EXISTS provider_usage(day INTEGER NOT NULL,device TEXT NOT NULL,service TEXT NOT NULL,spent INTEGER NOT NULL,PRIMARY KEY(day,device,service));
    `);
  }
  // AEAD binds ciphertext to its device. Moving a DB cell cannot change account ownership.
  seal(device, value) {
    const nonce = randomBytes(12),
      cipher = createCipheriv("aes-256-gcm", this.key, nonce);
    cipher.setAAD(Buffer.from(device));
    const bytes = Buffer.concat([
      cipher.update(JSON.stringify(value), "utf8"),
      cipher.final(),
    ]);
    return Buffer.concat([nonce, cipher.getAuthTag(), bytes]).toString(
      "base64",
    );
  }
  open(device, value) {
    const data = Buffer.from(value, "base64");
    if (data.length < 28) throw new Error("Invalid encrypted record");
    const cipher = createDecipheriv(
      "aes-256-gcm",
      this.key,
      data.subarray(0, 12),
    );
    cipher.setAAD(Buffer.from(device));
    cipher.setAuthTag(data.subarray(12, 28));
    return JSON.parse(
      Buffer.concat([
        cipher.update(data.subarray(28)),
        cipher.final(),
      ]).toString("utf8"),
    );
  }
  register(name) {
    if (typeof name !== "string" || name.length < 1 || name.length > 48)
      throw new Error("Invalid device name");
    const id = opaque(16),
      secret = opaque();
    this.db
      .prepare("INSERT INTO devices(id,name,credential_hash) VALUES(?,?,?)")
      .run(id, name, hash(secret));
    return { device_id: id, device_credential: secret };
  }
  device(id) {
    return this.db.prepare("SELECT * FROM devices WHERE id=?").get(id);
  }
  account(id) {
    const device = this.device(id);
    return device?.account ? this.open(id, device.account) : null;
  }
  saveAccount(id, account) {
    this.db
      .prepare("UPDATE devices SET account=? WHERE id=?")
      .run(this.seal(id, account), id);
  }
  unlink(id) {
    this.db.exec("BEGIN IMMEDIATE");
    try {
      this.db.prepare("UPDATE devices SET account=NULL WHERE id=?").run(id);
      this.db.prepare("DELETE FROM sessions WHERE device=?").run(id);
      this.db.prepare("DELETE FROM pairings WHERE device=?").run(id);
      this.db.exec("COMMIT");
    } catch (error) {
      this.db.exec("ROLLBACK");
      throw error;
    }
  }
  audit(event, id, time) {
    this.db
      .prepare("INSERT INTO audit(time,event,device) VALUES(?,?,?)")
      .run(time, event, id);
    this.db.prepare("DELETE FROM audit WHERE time<?").run(time - 30 * 86400000);
  }
  close() {
    this.db.close();
  }
}
