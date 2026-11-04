import assert from 'node:assert/strict';
import test from 'node:test';

import {Google} from '../src/google.mjs';
import {Photos} from '../src/photos.mjs';
import {Store} from '../src/store.mjs';

test('People API requests only read-only name/contact fields and retains pagination', async () => {
  const original = globalThis.fetch;
  globalThis.fetch = async (raw, options) => {
    const url = new URL(raw);
    assert.equal(url.hostname, 'people.googleapis.com');
    assert.equal(url.searchParams.get('personFields'), 'names,emailAddresses,phoneNumbers');
    assert.equal(url.searchParams.get('pageToken'), 'page-two');
    assert.equal(options.method, undefined);
    return Response.json({
      connections: [{
        resourceName: 'people/fixture',
        names: [{displayName: 'Fixture'}],
        phoneNumbers: [{value: '+123'}],
        emailAddresses: [{value: 'fixture@example.invalid'}]
      }],
      nextPageToken: 'page-three'
    });
  };
  try {
    const result =
        await new Google({}).contacts('synthetic', new URLSearchParams({page: 'page-two'}));
    assert.equal(result.contacts[0].title, 'Fixture');
    assert.equal(result.contacts[0].subtitle, '+123');
    assert.match(result.contacts[0].body, /fixture@example.invalid/);
    assert.equal(result.next_page, 'page-three');
  } finally {
    globalThis.fetch = original;
  }
});

test(
    'Photos Picker exposes only explicitly selected items and keeps base URLs off the watch',
    async t => {
      const original = globalThis.fetch, store = new Store(':memory:', '12'.repeat(32));
      t.after(() => {
        globalThis.fetch = original;
        store.close();
      });
      const device = store.register('Fixture').device_id;
      store.saveAccount(device, {
        binding: 'fixture-binding',
        profile: {sub: 'fixture'},
        services: ['photos'],
        tokens: {access_token: 'fixture'}
      });
      const photos = new Photos({store, now: () => 1000});
      let ready = false;
      globalThis.fetch = async (raw, options) => {
        const url = new URL(raw);
        assert.equal(options.headers.Authorization, 'Bearer fixture');
        if (url.pathname === '/v1/sessions')
          return Response.json({
            id: 'fixture-session',
            pickerUri: 'https://photos.google.com/picker/fixture',
            expireTime: '2099-01-01T00:00:00Z'
          });
        if (url.pathname.startsWith('/v1/sessions/')) return Response.json({mediaItemsSet: ready});
        assert.equal(url.pathname, '/v1/mediaItems');
        return Response.json({
          mediaItems: [
            {
              id: 'photo-fixture',
              type: 'PHOTO',
              createTime: '2026-11-04',
              mediaFile: {
                filename: 'Fixture.png',
                mimeType: 'image/png',
                baseUrl: 'https://lh3.googleusercontent.com/fixture'
              }
            },
            {id: 'video-fixture', type: 'VIDEO', mediaFile: {mimeType: 'video/mp4'}}
          ]
        });
      };
      const choose = await photos.create(device, 'fixture');
      assert.equal(choose.status, 'choose_photos');
      assert.equal((await photos.list(device, 'fixture', new URLSearchParams())).photos.length, 0);
      ready = true;
      const selected = await photos.list(device, 'fixture', new URLSearchParams());
      assert.equal(selected.photos.length, 1);
      assert.equal(selected.photos[0].title, 'Fixture.png');
      assert.doesNotMatch(JSON.stringify(selected), /googleusercontent|Bearer|access_token/);
      await assert.rejects(photos.image(device, 'fixture', 'unknown'), /PAGE_EXPIRED/);
      const account = store.account(device);
      account.picker.items[0].base = 'https://evil.invalid/steal';
      store.saveAccount(device, account);
      await assert.rejects(
          photos.image(device, 'fixture', selected.photos[0].id), /UPSTREAM_INVALID_RESPONSE/);
    });

test('an OAuth replacement cannot inherit a previous account\'s picker', async t => {
  const original = globalThis.fetch, store = new Store(':memory:', '12'.repeat(32));
  t.after(() => {
    globalThis.fetch = original;
    store.close();
  });
  const device = store.register('Fixture').device_id;
  const account = binding => ({binding, profile: {sub: binding}, services: ['photos'], tokens: {}});
  store.saveAccount(device, account('old'));
  globalThis.fetch = async () => {
    store.saveAccount(device, account('new'));
    return Response.json({id: 'fixture', pickerUri: 'https://photos.google.com/picker/fixture'});
  };
  await assert.rejects(new Photos({store}).create(device, 'old-fixture'), /UNAUTHORIZED/);
  assert.equal(store.account(device).picker, undefined);
});
