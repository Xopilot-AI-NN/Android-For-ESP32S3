// Device-bound document capabilities keep links/resources off the watch's network stack.
import {ApiError} from './google.mjs';
import {mailImage, mailWorker} from './mail.mjs';
import {publicResource} from './providers.mjs';
import {opaque} from './store.mjs';

export class Browser {
  constructor({
    now = () => Date.now(),
    providers,
    read = raw => publicResource(raw, 'text', {}, 0, true),
    render = mailWorker,
    image = mailImage
  } = {}) {
    this.now = now;
    this.providers = providers;
    this.read = read;
    this.render = render;
    this.readImage = image;
    this.documents = new Map();
  }
  document(device, id) {
    const doc = this.documents.get(id);
    if (!doc || doc.device !== device || doc.expires <= this.now())
      throw new ApiError('BROWSER_PAGE_EXPIRED', 410);
    return doc;
  }
  page(device, id, page = 0) {
    const doc = this.document(device, id);
    if (!Number.isInteger(page) || page < 0 || page >= doc.pages.length)
      throw new ApiError('INVALID_PAGE', 400);
    return {
      kind: 'browser-page',
      document: id,
      url: doc.url,
      title: doc.title,
      page,
      page_count: doc.pages.length,
      has_more: page + 1 < doc.pages.length,
      needs_js: doc.needs_js,
      blocks: doc.pages[page]
    };
  }
  async open(device, request) {
    if (!request || typeof request !== 'object' || Array.isArray(request))
      throw new ApiError('INVALID_PAGE', 400);
    let raw = request.url;
    if (request.result_id) raw = this.providers.resultUrl(device, request.result_id);
    if (request.document) {
      const previous = this.document(device, request.document);
      raw = previous.url;
      if (request.link !== undefined) {
        const block = previous.blocks[Number(request.link)];
        if (!Number.isInteger(request.link) || block?.type !== 'link' || !block.target)
          throw new ApiError('INVALID_PAGE', 400);
        raw = block.target;
        const target = new URL(raw), base = new URL(previous.url);
        if (target.origin === base.origin && target.pathname === base.pathname &&
            target.search === base.search && target.hash) {
          let anchor;
          try {
            anchor = decodeURIComponent(target.hash.slice(1));
          } catch {
            throw new ApiError('INVALID_PAGE', 400);
          }
          const index = previous.anchors[anchor];
          if (Number.isInteger(index)) {
            const page = previous.pages.findIndex(page => page.some(block => block.index >= index));
            return this.page(device, request.document, Math.max(0, page));
          }
        }
      }
    }
    if (typeof raw !== 'string' || raw.length > 2048)
      throw new ApiError('BROWSER_URL_INVALID', 400);
    let url;
    try {
      url = new URL(raw.includes('://') ? raw : 'https://' + raw);
    } catch {
      throw new ApiError('BROWSER_URL_INVALID', 400);
    }
    if (url.protocol !== 'https:' || url.username || url.password)
      throw new ApiError('BROWSER_URL_INVALID', 400);
    const resource = await this.read(url.href);
    const head = resource.bytes.subarray(0, 2048).toString('ascii');
    const charset = /charset\s*=\s*["']?([\w-]+)/i.exec(resource.contentType || '')?.[1] ||
        /charset\s*=\s*["']?([\w-]+)/i.exec(head)?.[1] || 'utf-8';
    let text;
    try {
      text = new TextDecoder(charset).decode(resource.bytes);
    } catch {
      text = resource.bytes.toString('utf8');
    }
    const plain = Array.from(text);
    const parsed = /^text\/plain/i.test(resource.contentType || '') ?
        {
          blocks: Array.from(
              {length: Math.ceil(plain.length / 900)},
              (_, i) => ({type: 'paragraph', text: plain.slice(i * 900, (i + 1) * 900).join('')})),
          title: new URL(resource.url).hostname,
          anchors: {}
        } :
        await this.render({operation: 'browser', html: text, url: resource.url});
    const pages = [];
    let chunk = [], chars = 0;
    for (const [index, block] of parsed.blocks.entries()) {
      const item = {
        index,
        type: block.type,
        text: Array.from(String(block.text || '')).slice(0, 900).join(''),
        level: block.level || 0,
        bold: Boolean(block.bold),
        indent: block.indent || 0
      };
      const length = Array.from(item.text).length;
      if (chunk.length && (chars + length > 2048 || chunk.length >= 24)) {
        pages.push(chunk);
        chunk = [];
        chars = 0;
      }
      chunk.push(item);
      chars += length;
    }
    if (chunk.length || !pages.length) pages.push(chunk);
    for (const [id, doc] of this.documents)
      if (doc.expires <= this.now()) this.documents.delete(id);
    while (this.documents.size >= 12) this.documents.delete(this.documents.keys().next().value);
    const id = opaque(16);
    this.documents.set(id, {
      device,
      url: resource.url,
      title: parsed.title || new URL(resource.url).hostname,
      blocks: parsed.blocks,
      anchors: parsed.anchors || {},
      needs_js: Boolean(parsed.needs_js),
      pages,
      expires: this.now() + 600000,
      images: new Map()
    });
    return this.page(device, id, 0);
  }
  async image(device, id, index) {
    const doc = this.document(device, id), block = doc.blocks[index];
    if (!Number.isInteger(index) || block?.type !== 'image')
      throw new ApiError('INVALID_PAGE', 400);
    let image = doc.images.get(index);
    if (!image) {
      image = await this.readImage({source: block.source}, new Map(), () => {
        throw new ApiError('PAGE_NOT_ALLOWED', 400);
      });
      if (doc.images.size >= 3) doc.images.delete(doc.images.keys().next().value);
      doc.images.set(index, image);
    }
    return {
      kind: 'browser-image',
      document: id,
      index,
      body: String(block.text || 'Изображение').slice(0, 120),
      image
    };
  }
}
