import assert from 'node:assert/strict';
import test from 'node:test';

import {duckSearch} from '../src/duckduckgo.mjs';
import {Providers} from '../src/providers.mjs';

const html = `<!doctype html><html><body>
<div class="result result--ad"><h2><a class="result__a" href="https://ad.example/">Advertisement</a></h2></div>
<div class="result results_links web-result"><h2><a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.org%2Fguide%3Fa%3D1%26b%3D2&amp;rut=fixture">Привет <b>мир</b></a></h2><div class="result__extras">example.org</div><a class="result__snippet">Текст &amp; <b>подробности</b>.</a></div>
<div class="result"><a class="result__a" href="javascript:alert(1)">Bad link</a></div></body></html>`;
test(
    'DuckDuckGo HTML search preserves Unicode, unwraps result links and needs no API key',
    async () => {
      const original = globalThis.fetch;
      globalThis.fetch = async (raw, options) => {
        const url = new URL(raw);
        assert.equal(url.origin, 'https://html.duckduckgo.com');
        assert.equal(url.searchParams.get('q'), 'привет & ESP32');
        assert.equal(options.redirect, 'error');
        assert.equal(options.headers.Authorization, undefined);
        assert.equal(options.headers['X-Subscription-Token'], undefined);
        return new Response(html, {headers: {'content-type': 'text/html; charset=utf-8'}});
      };
      try {
        const result = await duckSearch('привет & ESP32');
        assert.equal(result.length, 1);
        assert.equal(result[0].title, 'Привет мир');
        assert.equal(result[0].description, 'Текст & подробности.');
        assert.equal(result[0].url, 'https://example.org/guide?a=1&b=2');
      } finally {
        globalThis.fetch = original;
      }
    });
test(
    'DuckDuckGo challenges are reported without retrying or returning an empty success',
    async () => {
      const original = globalThis.fetch;
      let calls = 0;
      try {
        for (const status of [202, 403, 429]) {
          globalThis.fetch = async () => {
            calls++;
            return new Response('challenge', {status});
          };
          await assert.rejects(
              duckSearch('fixture'),
              error => error.code === 'SEARCH_VERIFICATION_REQUIRED' && error.status === 429);
        }
        globalThis.fetch = async () => {
          calls++;
          return new Response(
              '<form id="challenge-form" action="anomaly.js"></form>',
              {headers: {'content-type': 'text/html'}});
        };
        await assert.rejects(duckSearch('fixture'), /SEARCH_VERIFICATION_REQUIRED/);
        assert.equal(calls, 4);
      } finally {
        globalThis.fetch = original;
      }
    });
test('empty results are distinguished from an unknown upstream page', async () => {
  const original = globalThis.fetch;
  try {
    globalThis.fetch = async () => new Response(
        '<div class="no-results__message">No results</div>',
        {headers: {'content-type': 'text/html'}});
    assert.deepEqual(await duckSearch('fixture'), []);
    globalThis.fetch = async () =>
        new Response('<html>Unexpected layout</html>', {headers: {'content-type': 'text/html'}});
    await assert.rejects(duckSearch('fixture'), /UPSTREAM_INVALID_RESPONSE/);
  } finally {
    globalThis.fetch = original;
  }
});
test(
    'cached searches remain device-bound and challenge cooldown avoids repeated requests',
    async () => {
      let calls = 0, now = 1000;
      const provider = new Providers({}, {
        now: () => now,
        search: async () => {
          calls++;
          return [{url: 'https://example.org', title: 'Fixture', description: 'Text'}];
        }
      });
      const first = await provider.search('watch-one', 'fixture');
      assert.equal(first.provider, 'DuckDuckGo');
      assert.deepEqual(await provider.search('watch-one', 'fixture'), first);
      assert.equal(calls, 1);
      const other = await provider.search('watch-two', 'fixture');
      assert.equal(calls, 2);
      assert.notEqual(other.results[0].id, first.results[0].id);
      let blocked = 0;
      const limited = new Providers({}, {
        now: () => now,
        search: async () => {
          blocked++;
          throw Object.assign(
              new Error('SEARCH_VERIFICATION_REQUIRED'), {code: 'SEARCH_VERIFICATION_REQUIRED'});
        }
      });
      await assert.rejects(limited.search('watch-one', 'fixture'), /SEARCH_VERIFICATION_REQUIRED/);
      await assert.rejects(limited.search('watch-one', 'other'), /SEARCH_VERIFICATION_REQUIRED/);
      assert.equal(blocked, 1);
      now += 60001;
      await assert.rejects(limited.search('watch-one', 'other'), /SEARCH_VERIFICATION_REQUIRED/);
      assert.equal(blocked, 2);
    });
