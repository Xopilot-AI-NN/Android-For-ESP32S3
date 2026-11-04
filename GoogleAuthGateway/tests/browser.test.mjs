import assert from 'node:assert/strict';
import test from 'node:test';

import {Browser} from '../src/browser.mjs';
import {mailWorker} from '../src/mail.mjs';
import {Providers} from '../src/providers.mjs';

const html =
    `<html><head><title>Example &amp; test</title><base href="https://example.org/docs/"></head>
<body><h1>Hello мир</h1><script>secret()</script><style>bad</style><div hidden>hidden</div>
<p>A <strong>bold</strong> paragraph</p><ul><li>First</li><li>Second</li></ul>
<a href="next">Next page</a><a href="javascript:alert(1)">Unsafe</a><a>No href</a>
<img src="photo.png" alt="Photo"><img src="" alt="empty"><hr><h2 id="section">Section</h2><a href="#section">Jump</a></body></html>`;
test(
    'HTML reader keeps structure, resolves base/relative URLs and strips executable/hidden markup',
    async () => {
      const doc = await mailWorker({operation: 'browser', html, url: 'https://example.org/start'});
      assert.equal(doc.title, 'Example & test');
      assert.equal(doc.blocks[0].type, 'heading');
      assert.ok(doc.blocks.some(b => b.indent === 1 && b.text.includes('First')));
      assert.ok(
          doc.blocks.some(b => b.type === 'link' && b.target === 'https://example.org/docs/next'));
      assert.ok(doc.blocks.some(
          b => b.type === 'image' && b.source === 'https://example.org/docs/photo.png'));
      assert.equal(doc.blocks.filter(b => b.type === 'image').length, 1);
      assert.ok(doc.blocks.some(b => b.type === 'paragraph' && b.text === 'Unsafe'));
      assert.ok(doc.blocks.some(b => b.type === 'paragraph' && b.text === 'No href'));
      assert.ok(!JSON.stringify(doc.blocks).match(/secret\(\)|hidden|bad/));
      assert.ok(Number.isInteger(doc.anchors.section));
    });
test(
    'browser binds documents/images to device, follows capabilities and paginates without losing Unicode',
    async () => {
      let now = 0, reads = 0;
      const providers = new Providers({}, {
        now: () => now,
        search: async () => [{url: 'https://example.org/start', title: 'Result'}]
      });
      const browser = new Browser({
        providers,
        now: () => now,
        read: async url => {
          reads++;
          return {bytes: Buffer.from(html), url, contentType: 'text/html'};
        },
        image: async block => ({sourceTest: block.source})
      });
      const result = await providers.search('one', 'query');
      const first = await browser.open('one', {result_id: result.results[0].id});
      assert.equal(first.kind, 'browser-page');
      await assert.rejects(browser.open('two', {result_id: result.results[0].id}), /PAGE_EXPIRED/);
      assert.throws(() => browser.page('two', first.document), /BROWSER_PAGE_EXPIRED/);
      const link = first.blocks.find(b => b.type === 'link');
      assert.equal(
          (await browser.open('one', {document: first.document, link: link.index})).url,
          'https://example.org/docs/next');
      const img = first.blocks.find(b => b.type === 'image');
      assert.equal(
          (await browser.image('one', first.document, img.index)).image.sourceTest,
          'https://example.org/docs/photo.png');
      await assert.rejects(browser.image('two', first.document, img.index), /BROWSER_PAGE_EXPIRED/);
      await assert.rejects(browser.open('one', {url: 'http://example.org'}), /BROWSER_URL_INVALID/);
      now = 600001;
      assert.throws(() => browser.page('one', first.document), /BROWSER_PAGE_EXPIRED/);
      browser.read = async url =>
          ({bytes: Buffer.from('😀Привет '.repeat(5000)), url, contentType: 'text/plain'});
      const large = await browser.open('one', {url: 'example.org/plain'});
      let text = '';
      for (let i = 0; i < large.page_count; i++)
        text += browser.page('one', large.document, i).blocks.map(b => b.text).join('');
      assert.equal(text, '😀Привет '.repeat(5000));
    });
test(
    'document cache is bounded, anchors reuse pages, and script-only sites explain the limitation',
    async () => {
      const browser = new Browser({
        read: async url => ({
          bytes: Buffer.from('<h1 id="top">Title</h1><a href="#top">Up</a><script>app()</script>'),
          url
        })
      });
      const first = await browser.open('one', {url: 'https://example.org'});
      assert.equal(first.needs_js, true);
      const next = await browser.open('one', {document: first.document, link: 1});
      assert.equal(next.document, first.document);
      for (let i = 0; i < 20; i++) await browser.open('one', {url: `https://example.org/${i}`});
      assert.equal(browser.documents.size, 12);
      assert.throws(() => browser.page('one', first.document), /BROWSER_PAGE_EXPIRED/);
    });

test(
    'device HTTP contract opens and pages documents; credentials and ownership are checked',
    async t => {
      const {createGateway} = await import('../src/server.mjs');
      const {Store} = await import('../src/store.mjs');
      const store = new Store(':memory:', '12'.repeat(32)), one = store.register('One'),
            two = store.register('Two');
      const browser = new Browser(
          {read: async url => ({bytes: Buffer.from(html), url, contentType: 'text/html'})});
      const server =
          createGateway({config: {base: 'http://localhost:8080', dev: true}, store, browser});
      await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
      t.after(async () => {
        server.closeAllConnections();
        await new Promise(resolve => server.close(resolve));
        store.close();
      });
      const base = 'http://127.0.0.1:' + server.address().port;
      const headers = d => ({
        'X-Device-ID': d.device_id,
        Authorization: 'Device ' + d.device_credential,
        'Content-Type': 'application/json'
      });
      const open = await fetch(base + '/api/v1/browser/open', {
        method: 'POST',
        headers: headers(one),
        body: JSON.stringify({url: 'https://example.org/'})
      });
      assert.equal(open.status, 200);
      const page = await open.json();
      const path = base + '/api/v1/browser/documents/' + page.document;
      assert.equal((await fetch(path, {headers: headers(one)})).status, 200);
      assert.equal((await fetch(path, {headers: headers(two)})).status, 410);
      assert.equal((await fetch(path)).status, 401);
      assert.equal((await fetch(path + '?page=-1', {headers: headers(one)})).status, 400);
    });

test(
    'legacy Cyrillic charset and malformed optional attributes do not break the whole page',
    async () => {
      const browser = new Browser({
        read: async url => ({
          bytes: Buffer.from([0xcf, 0xf0, 0xe8, 0xe2, 0xe5, 0xf2]),
          url,
          contentType: 'text/plain;charset=windows-1251'
        })
      });
      const page = await browser.open('one', {url: 'example.org'});
      assert.equal(page.blocks[0].text, 'Привет');
      await assert.rejects(browser.open('one', null), /INVALID_PAGE/);
      const malformed = await mailWorker({
        operation: 'browser',
        html: '<p style>Visible<img src><a href="https://[">Broken</a></p>',
        url: 'https://example.org'
      });
      assert.ok(malformed.blocks.some(b => b.text === 'Visible'));
    });
