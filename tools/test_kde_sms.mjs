import assert from 'node:assert/strict';
import test from 'node:test';

import {normalizeSms, publishSms, readSms} from './kde_sms.mjs';

test('KDE structure and map rows become bounded newest-first SMS previews', () => {
  const packet = {
    type: 'av',
    data: [[
      {
        type: '(isa(s)xiixixav)',
        data: [1, 'Привет', [['+123']], 1700000000000, 1, 0, 7, 1, -1, []]
      },
      {
        type: 'a{sv}',
        data: {body: {type: 's', data: 'New'}, addresses: [{address: '+456'}], date: 1700000001000}
      },
    ]]
  };
  const result = normalizeSms(packet);
  assert.equal(result.messages[0].body, 'New');
  assert.equal(result.messages[1].sender, '+123');
  assert.equal(result.messages[1].body, 'Привет');
  assert.ok(Buffer.byteLength(JSON.stringify(result)) <= 8192);
});
test('SMS transport stays within PC shell limit and sends complete byte offsets', async () => {
  const commands = [];
  await publishSms(
      {available: true, messages: [{sender: 'Fixture', body: 'Привет 😀', date: '2026-11-04'}]},
      '1234567890abcdef', async command => {
        commands.push(command);
        return 'OK';
      });
  assert.ok(commands.every(command => Buffer.byteLength(command) <= 80));
  const chunks =
      commands.filter(command => command.includes(' sms-data ')).map(command => command.split(' '));
  const bytes = Buffer.concat(chunks.map(command => Buffer.from(command[4], 'hex')));
  assert.equal(JSON.parse(bytes).messages[0].body, 'Привет 😀');
  await assert.rejects(publishSms({messages: []}, '1234567890abcdef', async () => 'unpaired'));
});
test(
    'SMS adapter uses the device conversations interface without shell interpolation', async () => {
      const calls = [];
      const result = await readSms('d'.repeat(32), async args => {
        calls.push(args);
        return JSON.stringify({type: 'av', data: [[]]});
      });
      assert.equal(result.messages.length, 0);
      assert.equal(calls[0].at(-1), 'requestAllConversationThreads');
      assert.equal(calls[1].at(-1), 'activeConversations');
      assert.ok(calls[1].includes('org.kde.kdeconnect.device.conversations'));
    });
