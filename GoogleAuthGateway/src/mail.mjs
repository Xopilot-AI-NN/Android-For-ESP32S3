// HTML becomes bounded native pages, never executable markup on the watch.
import {spawn} from 'node:child_process';
import {fileURLToPath} from 'node:url';

import {ApiError} from './google.mjs';
import {publicResource} from './providers.mjs';

const workerPath = fileURLToPath(new URL('../mail_content.py', import.meta.url));
export function mailWorker(input) {
  return new Promise((resolve, reject) => {
    const child = spawn(
        process.env.GATEWAY_PYTHON || 'python3', [workerPath], {stdio: ['pipe', 'pipe', 'ignore']});
    let size = 0;
    const parts = [];
    const timer = setTimeout(() => child.kill('SIGKILL'), 5000);
    child.stdout.on('data', chunk => {
      size += chunk.length;
      if (size > 3 * 1024 * 1024)
        child.kill('SIGKILL');
      else
        parts.push(chunk);
    });
    child.stdin.on('error', () => {});
    child.on('error', () => {
      clearTimeout(timer);
      reject(new ApiError('MAIL_RENDER_UNAVAILABLE', 503));
    });
    child.on('close', code => {
      clearTimeout(timer);
      if (code !== 0) {
        reject(new ApiError('MAIL_RENDER_UNAVAILABLE', 503));
        return;
      }
      try {
        resolve(JSON.parse(Buffer.concat(parts).toString('utf8')));
      } catch {
        reject(new ApiError('MAIL_RENDER_UNAVAILABLE', 503));
      }
    });
    child.stdin.end(JSON.stringify(input));
  });
}
export async function mailPages(html, plain) {
  const blocks = html ? await mailWorker({operation: 'html', html}) : [{type: 'text', text: plain}];
  const pages = [];
  for (const block of blocks) {
    if (block.type === 'image')
      pages.push(block);
    else {
      const chars = Array.from(block.text || '');
      for (let at = 0; at < chars.length; at += 4096)
        pages.push({type: 'text', text: chars.slice(at, at + 4096).join('')});
    }
    if (pages.length >= 256) break;
  }
  return pages.slice(0, 256);
}
export async function mailImage(block, inline, attachment, dependencies = {}) {
  const source = block.source || '';
  let bytes;
  if (source.startsWith('cid:')) {
    const part = inline.get(source.slice(4));
    if (!part) throw new ApiError('MAIL_IMAGE_UNAVAILABLE', 422);
    if (part.body?.data)
      bytes = Buffer.from(part.body.data, 'base64url');
    else if (part.body?.attachmentId)
      bytes = await attachment(part.body.attachmentId);
  } else if (/^data:image\/(?:png|jpeg|gif|webp);base64,/i.test(source)) {
    bytes = Buffer.from(source.slice(source.indexOf(',') + 1), 'base64');
  } else {
    // No Google Authorization, cookies or Referer are sent to external image hosts.
    bytes = await publicResource(source, 'image', dependencies);
  }
  if (!bytes?.length || bytes.length > 2 * 1024 * 1024)
    throw new ApiError('MAIL_IMAGE_UNAVAILABLE', 422);
  return mailWorker({operation: 'image', data: bytes.toString('base64')});
}
