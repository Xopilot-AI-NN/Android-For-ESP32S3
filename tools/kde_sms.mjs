// Read the local KDE D-Bus adapter. This is SMS/MMS text, not the Google Messages web protocol.
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
const exec = promisify(execFile);
function unwrap(value) {
  if (value && typeof value === 'object' && 'type' in value && 'data' in value)
    return unwrap(value.data);
  if (Array.isArray(value)) return value.map(unwrap);
  if (value && typeof value === 'object')
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, unwrap(item)]));
  return value;
}
function clipped(value, bytes) {
  let result = '';
  for (const character of String(value || '').replace(/[\x00-\x08\x0b\x0c\x0e-\x1f]/g, '')) {
    if (Buffer.byteLength(result + character) > bytes) break;
    result += character;
  }
  return result;
}
export function normalizeSms(packet) {
  const rows = packet?.data?.[0] || [];
  const messages =
      rows.map(value => {
            let row = unwrap(value);
            while (Array.isArray(row) && row.length === 1 && Array.isArray(row[0])) row = row[0];
            // Current KDE uses a D-Bus structure. Older builds exported a QVariantMap.
            const structured = Array.isArray(row);
            const body = structured ? row[1] : row.body;
            const addresses = structured ? row[2] : row.addresses;
            const timestamp = Number(structured ? row[3] : row.date);
            const sender = structured ?
                (addresses || []).flat(Infinity).join(', ') :
                (addresses || []).map(address => address.address || address).join(', ');
            const date = new Date(timestamp < 1e11 ? timestamp * 1000 : timestamp);
            return {
              sender: clipped(sender || 'Неизвестный номер', 64),
              body: clipped(body, 512),
              date: Number.isFinite(date.getTime()) ?
                  date.toISOString().slice(0, 16).replace('T', ' ') :
                  '',
              timestamp
            };
          })
          .filter(row => row.body)
          .sort((a, b) => b.timestamp - a.timestamp)
          .slice(0, 12)
          .map(({timestamp, ...row}) => row);
  while (messages.length && Buffer.byteLength(JSON.stringify({available: true, messages})) > 8192)
    messages.pop();
  return {available: true, messages};
}
export async function readSms(
    device,
    run = async args =>
        (await exec('busctl', args, {timeout: 5000, maxBuffer: 256 * 1024})).stdout) {
  if (!/^[A-Za-z0-9_-]{32,64}$/.test(device)) throw new Error('Invalid KDE device');
  const base = [
    '--user', '--json=short', 'call', 'org.kde.kdeconnect', `/modules/kdeconnect/devices/${device}`,
    'org.kde.kdeconnect.device.conversations'
  ];
  await run([...base, 'requestAllConversationThreads']);
  return normalizeSms(JSON.parse(await run([...base, 'activeConversations'])));
}
export async function publishSms(payload, session, watch) {
  const bytes = Buffer.from(JSON.stringify(payload));
  if (bytes.length > 8192) throw new Error('SMS batch too large');
  if (await watch(`companion sms-begin ${session} ${bytes.length}`) !== 'OK')
    throw new Error('SMS bridge not authorized');
  for (let offset = 0; offset < bytes.length; offset += 16) {
    if (offset && offset % 512 === 0) await watch(`companion update ${session} 1`);
    const reply = await watch(`companion sms-data ${session} ${offset} ${
        bytes.subarray(offset, offset + 16).toString('hex')}`);
    if (reply !== 'OK') throw new Error('SMS batch interrupted');
  }
  if (await watch(`companion sms-end ${session}`) !== 'OK') throw new Error('SMS batch rejected');
}
