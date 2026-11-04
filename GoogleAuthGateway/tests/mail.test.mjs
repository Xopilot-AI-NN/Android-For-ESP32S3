import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import test from 'node:test';
import {deflateSync, inflateSync} from 'node:zlib';

import {Google} from '../src/google.mjs';
import {mailImage, mailPages, mailWorker} from '../src/mail.mjs';
import {publicResource} from '../src/providers.mjs';

const image = () => execFileSync('python3', [
  '-c',
  'from PIL import Image; import io,sys; b=io.BytesIO(); Image.new(\'RGB\',(216,160),\'#3ddc84\').save(b,format=\'PNG\'); sys.stdout.buffer.write(b.getvalue())'
]);

test(
    'mail HTML preserves paragraphs, Unicode, links and image order without executing markup',
    async () => {
      const pages = await mailPages(
          '<head>private head</head><h1>Привет &amp; мир</h1><script>bad()</script><p hidden>hidden</p><ul><li>Первый</li></ul><a href="https://example.org">Ссылка</a><img src="cid:picture" alt="Фото"><p>После картинки</p>',
          'fallback');
      assert.equal(pages.length, 3);
      assert.match(pages[0].text, /Привет & мир/);
      assert.match(pages[0].text, /• Первый/);
      assert.match(pages[0].text, /https:\/\/example.org/);
      assert.doesNotMatch(pages[0].text, /hidden|bad|private head|fallback/);
      assert.equal(pages[1].source, 'cid:picture');
      assert.equal(pages[2].text, 'После картинки');
    });

test(
    'inline image is bounded, compressed RGB565; malformed images fail independently', async () => {
      const bytes = image();
      const preview = await mailImage(
          {source: 'cid:picture'},
          new Map([['picture', {body: {data: bytes.toString('base64url')}}]]),
          () => assert.fail('no attachment fetch'));
      assert.equal(preview.width, 108);
      assert.equal(preview.height, 80);
      const raw = inflateSync(Buffer.from(preview.data, 'base64'));
      assert.equal(raw.length, 108 * 80 * 2);
      assert.ok(
          Buffer.byteLength(JSON.stringify({image: preview, body: '😀'.repeat(120)})) < 24 * 1024);
      await assert.rejects(mailWorker({operation: 'image', data: 'not an image'}));
      await assert.rejects(
          mailImage({source: 'cid:missing'}, new Map(), () => {}), /MAIL_IMAGE_UNAVAILABLE/);
    });

test(
    'remote mail image rejects private DNS and non-HTTPS sources without forwarding OAuth',
    async () => {
      let connected = false;
      await assert.rejects(
          publicResource('https://private.example/image.png', 'image', {
            resolve: async () => [{address: '127.0.0.1'}],
            request: () => {
              connected = true;
            }
          }),
          /PAGE_NOT_ALLOWED/);
      assert.equal(connected, false);
      await assert.rejects(
          publicResource('http://example.org/image.png', 'image'), /PAGE_NOT_ALLOWED/);
    });

test(
    'Gmail HTML can read a referenced CID attachment without downloading unrelated attachments',
    async () => {
      const original = globalThis.fetch, calls = [], bytes = image();
      globalThis.fetch = async url => {
        calls.push(String(url));
        if (String(url).includes('/attachments/picture-part'))
          return Response.json({data: bytes.toString('base64url')});
        return Response.json({
          payload: {
            mimeType: 'multipart/related',
            parts: [
              {
                mimeType: 'text/html',
                body: {
                  data: Buffer.from('<p>До</p><img src="cid:picture" alt="Фото"><p>После</p>')
                            .toString('base64url')
                }
              },
              {
                mimeType: 'image/png',
                filename: 'image.png',
                headers: [{name: 'Content-ID', value: '<picture>'}],
                body: {attachmentId: 'picture-part'}
              },
              {
                mimeType: 'application/pdf',
                filename: 'private.pdf',
                body: {attachmentId: 'do-not-load'}
              }
            ]
          }
        });
      };
      try {
        const google = new Google({});
        assert.equal(
            (await google.message('synthetic', 'fixture', new URLSearchParams())).body, 'До');
        const middle =
            await google.message('synthetic', 'fixture', new URLSearchParams({page: '1'}));
        assert.ok(middle.image);
        assert.equal(middle.has_more, true);
        assert.equal(
            (await google.message('synthetic', 'fixture', new URLSearchParams({page: '2'}))).body,
            'После');
        assert.equal(calls.filter(url => url.includes('/attachments/')).length, 1);
        assert.ok(!calls.some(url => url.includes('do-not-load')));
      } finally {
        globalThis.fetch = original;
      }
    });
