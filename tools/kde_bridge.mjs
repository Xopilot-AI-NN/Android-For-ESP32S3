#!/usr/bin/env node
import {readSms,publishSms} from "./kde_sms.mjs";
// Модуль локального моста к установленному KDE Connect.
// Optional KDE Connect desktop adapter. Uses KDE's existing paired-device transport.
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {randomBytes} from 'node:crypto';
import {pathToFileURL} from 'node:url';
const exec = promisify(execFile);
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
// Вызывает установленный kdeconnect-cli с таймаутом и ограниченным выводом.
export async function kde(args) {
  const {stdout} = await exec('kdeconnect-cli', args, {timeout: 8000, maxBuffer: 65536});
  return stdout.trim();
}
// Выполняет ограниченную команду через существующий Unix-сокет PC-моста.
export function pcShell(socketPath, command) {
  if (Buffer.byteLength(command) > 80) throw new Error('PC shell limit: 80 bytes');
  return new Promise((resolve, reject) => {
    const socket = net.createConnection(socketPath);
    let data = '';
    socket.setTimeout(8000, () => socket.destroy(new Error('Watch bridge timeout')));
    socket.on('connect', () => socket.write(`@ZADB|CMD|${Buffer.from(command).toString('hex')}\n`));
    socket.on('data', bytes => {
      data += bytes.toString();
      if (data.length > 8192) return socket.destroy(new Error('Watch response too large'));
      for (const line of data.split('\n')) {
        const prefix = '@ZADB|OUT|shell|';
        if (line.startsWith(prefix)) { resolve(line.slice(prefix.length).trim()); socket.end(); return; }
      }
    });
    socket.on('error', reject);
    socket.on('end', () => reject(new Error('Watch disconnected before response')));
  });
}
// Проверяет запрос часов, подтверждает получение и вызывает соответствующее действие KDE.
export async function handleAction(packet, session, watch, runKde, device) {
  if (!Number.isSafeInteger(packet.id) || packet.id < 1 || !['ring', 'ping', 'share'].includes(packet.action)) return false;
  const args = ['--device', device];
  if (packet.action === 'ring') args.push('--ring');
  if (packet.action === 'ping') args.push('--ping-msg', 'Сигнал с часов AOSP Wear');
  if (packet.action === 'share') {
    if (!Number.isSafeInteger(packet.bytes) || packet.bytes < 1 || packet.bytes > 3072) throw new Error('Invalid text length');
    const chunks = [];
    for (let offset = 0; offset < packet.bytes; offset += 64) {
      const raw = await watch(`companion text ${session} ${offset}`);
      if (!/^(?:[a-fA-F0-9]{2}){1,64}$/.test(raw)) throw new Error('Text expired or watch locked');
      chunks.push(Buffer.from(raw, 'hex'));
    }
    const bytes = Buffer.concat(chunks);
    if (bytes.length !== packet.bytes) throw new Error('Truncated text');
    const text = new TextDecoder('utf-8', {fatal: true}).decode(bytes);
    args.push('--share-text', text);
  }
  // Consume the watch request BEFORE invoking KDE: a reconnect must not ring/send twice.
  // A process crash after this acknowledgement loses the action; the user may retry.
  const ack = await watch(`companion ack ${session} ${packet.id} 2`);
  if (ack !== 'OK') throw new Error('Watch did not accept acknowledgement');
  try {
    await runKde(args);
    await watch(`companion ack ${session} ${packet.id} 1`);
  } catch (error) {
    await watch(`companion ack ${session} ${packet.id} 0`).catch(() => {});
    throw error;
  }
  return true;
}
// Разбирает разрешённые параметры запуска моста.
function options(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i++) {
    const key = argv[i];
    if (['--list', '--self-test', '--help'].includes(key)) out[key.slice(2)] = true;
    else if (['--device', '--socket', '--watch', '--adb'].includes(key) && argv[i + 1]) out[key.slice(2)] = argv[++i];
    else throw new Error(`Unknown/missing option: ${key}`);
  }
  return out;
}
// Проверяет протокол моста на искусственных ответах, не обращаясь к устройствам.
async function selfTest() {
  const assert = await import('node:assert/strict');
  const calls = []; const sent = [];
  const text = 'Привет\nтекст'; const bytes = Buffer.from(text);
  const watch = async cmd => { calls.push(cmd); return cmd.includes(' text ') ? bytes.toString('hex') : 'OK'; };
  assert.equal(await handleAction({id: 1, action: 'none'}, 'abc', watch, async a => sent.push(a), 'phone'), false);
  assert.equal(sent.length, 0);
  await handleAction({id: 7, action: 'share', bytes: bytes.length}, 'abc', watch, async a => sent.push(a), 'phone');
  assert.deepEqual(sent[0], ['--device', 'phone', '--share-text', text]);
  assert.ok(calls.at(-1).includes(' ack '));
  await assert.rejects(handleAction({id: 8, action: 'share', bytes: 999999}, 'abc', watch, async () => {}, 'phone'));
  const failingCalls = [];
  await assert.rejects(handleAction({id: 9, action: 'ring'}, 'abc', async cmd => { failingCalls.push(cmd); return 'OK'; }, async () => { throw new Error('offline'); }, 'phone'));
  assert.deepEqual(failingCalls, ['companion ack abc 9 2', 'companion ack abc 9 0']);
  const denied = [];
  await assert.rejects(handleAction({id: 10, action: 'ring'}, 'abc', async () => 'denied', async a => denied.push(a), 'phone'));
  assert.equal(denied.length, 0);
  console.log('KDE bridge self-tests passed (no devices contacted)');
}
// Разбирает параметры запуска и выполняет основной сценарий утилиты.
export async function main(argv = process.argv.slice(2)) {
  const opts = options(argv);
  if (opts['self-test']) return selfTest();
  if (opts.help) { console.log('node Bootloader/tools/kde_bridge.mjs --list\nnode Bootloader/tools/kde_bridge.mjs --device KDE_DEVICE_ID [--socket PATH | --watch IP:5555] [--adb /usr/bin/adb]'); return; }
  if (opts.list) { console.log(await kde(['--list-available', '--id-name-only'])); return; }
  if (!/^[A-Za-z0-9_-]{32,64}$/.test(opts.device ?? '')) throw new Error('Choose an already paired device with --list, then pass --device ID');
  const defaultSocket = process.env.XDG_RUNTIME_DIR ? path.join(process.env.XDG_RUNTIME_DIR, 'aosp-wear-viewer.sock') : `/tmp/aosp-wear-viewer-${os.userInfo().uid}.sock`;
  const watch = opts.watch ? async command => {
    const {stdout} = await exec(opts.adb ?? '/usr/bin/adb', ['-s', opts.watch, 'shell', command], {timeout: 8000, maxBuffer: 8192});
    return stdout.trim();
  } : command => pcShell(opts.socket ?? defaultSocket, command);
  const session = randomBytes(8).toString('hex');
  console.log(`Open Телефон on the watch and approve code ${session.slice(-4)}. KDE device: ${opts.device}`);
  let reachable = false;
  let nextReachabilityCheck = 0;
  let nextSmsCheck = 0;
  let lastStatus = '';
  const status = message => { if (message !== lastStatus) { console.log(message); lastStatus = message; } };
  while (true) {
    try {
      const hello = await watch(`companion hello ${session} 4b444520436f6e6e656374`);
      if (hello !== 'approved') { status(hello); nextReachabilityCheck = 0; await sleep(2000); continue; }
      // KDE discovery is comparatively expensive. Keep the watch heartbeat at
      // two seconds, but query desktop device discovery only every ten seconds.
      if (Date.now() >= nextReachabilityCheck) {
        reachable = (await kde(['--list-available', '--id-only'])).split(/\s+/).includes(opts.device);
        nextReachabilityCheck = Date.now() + 10000;
      }
      status(reachable ? 'KDE device connected' : 'KDE device offline');
      await watch(`companion update ${session} ${reachable ? 1 : 0}`);
      if (reachable && Date.now() >= nextSmsCheck) {
        nextSmsCheck = Date.now() + 30000;
        const sms = await readSms(opts.device).catch(() => ({available:false,messages:[]}));
        await publishSms(sms,session,watch);
      }
      if (reachable) {
        const raw = await watch(`companion poll ${session}`);
        if (raw.startsWith('{')) await handleAction(JSON.parse(raw), session, watch, kde, opts.device);
      }
    } catch (error) {
      reachable = false;
      nextReachabilityCheck = 0;
      await watch(`companion update ${session} 0`).catch(() => {});
      status(`Bridge error: ${error.message}`);
    }
    await sleep(2000);
  }
}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) main().catch(error => { console.error(error.message); process.exitCode = 1; });
