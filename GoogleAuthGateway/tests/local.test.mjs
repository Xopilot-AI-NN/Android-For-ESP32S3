// Exercise the real TLS and loopback listeners with synthetic account data.
import test from "node:test";
import assert from "node:assert/strict";
import http from "node:http";
import https from "node:https";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createGateway } from "../src/server.mjs";
import { Store } from "../src/store.mjs";
import { SCOPES } from "../src/google.mjs";
import { config } from "../src/config.mjs";

test("local configuration preserves TLS and limits independent callbacks to LAN mode", () => {
  const env = { PUBLIC_BASE_URL: "https://192.168.1.2:8443", PORT: "8443", LOCAL_IP: "192.168.1.2",
    OAUTH_REDIRECT_URL: "http://localhost:8080/oauth/callback", TLS_CERT: "/fixture.pem", TLS_KEY: "/fixture.key",
    GOOGLE_CLIENT_ID: "fixture", GOOGLE_CLIENT_SECRET: "fixture", STORE_KEY: "ab".repeat(32) };
  assert.equal(config(env).dev, false);
  for (const change of [{LOCAL_IP:"8.8.8.8"}, {LOCAL_IP:""}, {TLS_KEY:""},
    {OAUTH_REDIRECT_URL:"http://remote.invalid/oauth/callback"}, {PORT:"8080"}]) {
    assert.throws(() => config({...env, ...change}));
  }
});

test("local browser OAuth keeps cookies, PKCE and watch approval; HTTP cannot use device API", async (t) => {
  const directory = mkdtempSync(join(tmpdir(), "wear-local-test-"));
  const key = join(directory, "key.pem"), cert = join(directory, "cert.pem");
  execFileSync("openssl", ["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
    "-subj", "/CN=local-fixture", "-addext", "subjectAltName=IP:127.0.0.1", "-keyout", key, "-out", cert], {stdio:"ignore"});
  const store = new Store(":memory:", "12".repeat(32)), device = store.register("Local fixture");
  let exchanges = 0;
  const settings = { base:"https://192.168.1.2:8443", localIp:"192.168.1.2", dev:false,
    oauthRedirect:"http://localhost:8080/oauth/callback", clientId:"fixture", clientSecret:"fixture" };
  const server = createGateway({config:settings, store,
    tls:{cert:readFileSync(cert), key:readFileSync(key)},
    google:{
      async exchange(code, verifier) {
        assert.equal(code, "fixture"); assert.ok(verifier.length >= 43); exchanges++;
        return {access_token:"fixture", refresh_token:"fixture", expires_in:3600, scope:SCOPES.calendar};
      },
      async profile() {return {sub:"fixture", email:"local@example.invalid", email_verified:true};},
    }});
  const browser = http.createServer(server.listeners("request")[0]);
  await Promise.all([server, browser].map(s => new Promise(resolve => s.listen(0,"127.0.0.1",resolve))));
  t.after(async () => {
    await Promise.all([server, browser].map(s => new Promise(resolve => {s.closeAllConnections();s.close(resolve);} )));
    store.close(); rmSync(directory,{recursive:true,force:true});
  });
  function request(secure, path, {method="GET", headers={}, payload}={}) {
    return new Promise((resolve,reject) => {
      const req = (secure ? https : http).request({hostname:"127.0.0.1", port:(secure?server:browser).address().port,
        path, method, ca:readFileSync(cert), headers:{Host:secure?"192.168.1.2:8443":"localhost:8080", ...headers}}, res => {
          let body="";res.setEncoding("utf8");res.on("data",v=>body+=v);res.on("end",()=>resolve({status:res.statusCode,headers:res.headers,body}));
        });
      req.on("error",reject);req.end(payload && JSON.stringify(payload));
    });
  }
  const auth = {"X-Device-ID":device.device_id, Authorization:"Device " + device.device_credential};
  assert.equal((await request(false,"/api/v1/google/pairings",{headers:auth})).status,403);
  assert.equal((await request(false,"/health",{headers:{Host:"evil.invalid","X-Forwarded-Proto":"https"}})).status,400);
  const created = await request(true,"/api/v1/google/pairings",{method:"POST",headers:auth,payload:{services:["calendar"]}});
  assert.equal(created.status,201);
  const pair=JSON.parse(created.body), path="/p/"+pair.pairing_id;
  assert.match((await request(true,path)).body,/localhost:8080/);
  const landing = await request(false,"/");
  assert.equal(landing.status,200);
  assert.ok(landing.body.includes(path));
  assert.equal((await request(false,path,{headers:{Host:"evil.invalid"}})).status,400);
  const login = await request(false,path+"/start",{method:"POST"});
  assert.equal(login.status,303);
  const location=new URL(login.headers.location), state=location.searchParams.get("state");
  assert.equal(location.searchParams.get("redirect_uri"),settings.oauthRedirect);
  assert.equal(location.searchParams.get("code_challenge_method"),"S256");
  const cookie=login.headers["set-cookie"][0];assert.doesNotMatch(cookie,/; Secure/);
  const callback="/oauth/callback?state="+state+"&code=fixture";
  assert.equal((await request(false,callback)).status,403);
  assert.equal(exchanges,0);
  assert.equal((await request(false,callback,{headers:{Cookie:cookie.split(";")[0]}})).status,200);
  assert.equal(exchanges,1);
  const status=JSON.parse((await request(true,"/api/v1/google/pairings/"+pair.pairing_id,{headers:auth})).body);
  assert.equal(status.status,"awaiting_watch_confirmation");
  assert.equal((await request(false,callback,{headers:{Cookie:cookie.split(";")[0]}})).status,403);
});
