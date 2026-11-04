import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync, renameSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { watchProviderKeys } from "../src/provider-config.mjs";
test("atomic provider imports update running keys without replacing OAuth/store configuration", async t => {
  const directory = mkdtempSync(join(tmpdir(),"wear-provider-config-")), file = join(directory,"config.env");
  writeFileSync(file,'BRAVE_SEARCH_API_KEY="old"\n');
  const config={translationKey:"",clientSecret:"fixture-only",key:"fixture-store"};
  const watcher=watchProviderKeys(file,config);
  t.after(()=>{watcher.close();rmSync(directory,{recursive:true,force:true});});
  writeFileSync(join(directory,"temporary"),'BRAVE_SEARCH_API_KEY="new"\nGOOGLE_TRANSLATE_API_KEY="translate"\nGOOGLE_CLIENT_SECRET="must-not-replace"\nSTORE_KEY="must-not-replace"\n');
  renameSync(join(directory,"temporary"),file);
  for(let tries=0;tries<50 && config.translationKey!=="translate";tries++)await delay(10);
  assert.equal(config.searchKey,undefined);assert.equal(config.translationKey,"translate");
  assert.equal(config.clientSecret,"fixture-only");assert.equal(config.key,"fixture-store");
});
