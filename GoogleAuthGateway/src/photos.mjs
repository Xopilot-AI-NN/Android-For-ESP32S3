// Picker access covers only photos deliberately selected by the user in Google's UI.
import {createHash} from 'node:crypto';

import {ApiError, json} from './google.mjs';
import {mailWorker} from './mail.mjs';

const root = 'https://photospicker.googleapis.com/v1/';
const headers = token => ({Authorization: `Bearer ${token}`, 'Content-Type': 'application/json'});
export class Photos {
  constructor({store, now = () => Date.now()}) {
    this.store = store;
    this.now = now;
  }
  account(device) {
    const account = this.store.account(device);
    if (!account) throw new ApiError('REAUTH_REQUIRED', 401);
    return account;
  }
  save(device, account, previousPicker) {
    const current = this.store.account(device);
    if (current?.binding !== account.binding || current.picker?.id !== previousPicker)
      throw new ApiError('UNAUTHORIZED', 401);
    // Refreshing tokens or reading another service may finish while Picker is awaiting Google.
    // Merge only the selection, retaining the current account tokens and profile.
    current.picker = account.picker;
    this.store.saveAccount(device, current);
  }
  async create(device, token) {
    const account = this.account(device);
    const previousPicker = account.picker?.id;
    if (account.picker?.id) {
      // Best effort cleanup of the old selection; failures never hide a fresh session.
      await fetch(root + 'sessions/' + encodeURIComponent(account.picker.id), {
        method: 'DELETE',
        headers: headers(token),
        redirect: 'error',
        signal: AbortSignal.timeout(5000)
      }).catch(() => {});
    }
    const picked =
        await json(root + 'sessions', {method: 'POST', headers: headers(token), body: '{}'});
    const uri = new URL(picked.pickerUri);
    if (uri.protocol !== 'https:' || uri.hostname !== 'photos.google.com' || uri.username ||
        uri.password)
      throw new ApiError('UPSTREAM_INVALID_RESPONSE');
    account.picker = {
      id: picked.id,
      uri: uri.href,
      expires: Date.parse(picked.expireTime) || this.now() + 3600000,
      items: []
    };
    this.save(device, account, previousPicker);
    return {status: 'choose_photos', picker_url: uri.href, photos: []};
  }
  async list(device, token, query) {
    const account = this.account(device), picker = account.picker;
    if (!picker || picker.expires <= this.now()) return {status: 'choose_photos', photos: []};
    const session =
        await json(root + 'sessions/' + encodeURIComponent(picker.id), {headers: headers(token)});
    if (!session.mediaItemsSet)
      return {status: 'choose_photos', photos: [], picker_url: picker.uri};
    const page = query.get('page') || '';
    if (page.length > 512 || !/^[\w+/=-]*$/.test(page)) throw new ApiError('INVALID_PAGE', 400);
    const params = new URLSearchParams({sessionId: picker.id, pageSize: '10'});
    if (page) params.set('pageToken', page);
    const data = await json(root + 'mediaItems?' + params, {headers: headers(token)});
    picker.selected = true;
    picker.items =
        (data.mediaItems || [])
            .filter(
                item => item.type === 'PHOTO' && /^image\//.test(item.mediaFile?.mimeType || ''))
            .slice(0, 10)
            .map(item => ({
                   id: createHash('sha256').update(String(item.id)).digest('hex').slice(0, 32),
                   title: String(item.mediaFile.filename || 'Фото').slice(0, 160),
                   subtitle: String(item.createTime || '').slice(0, 40),
                   base: item.mediaFile.baseUrl
                 }));
    this.save(device, account, picker.id);
    return {
      status: 'selected',
      photos: picker.items.map(({base, ...item}) => item),
      page_token: page,
      next_page: data.nextPageToken || null
    };
  }
  async image(device, token, id) {
    const account = this.account(device), picker = account.picker;
    const item = picker?.expires > this.now() && picker.items.find(item => item.id === id);
    if (!item) throw new ApiError('PAGE_EXPIRED', 410);
    const url = new URL(item.base);
    // Never send an account token to a URL supplied by a watch or a third-party image host.
    if (url.protocol !== 'https:' || !url.hostname.endsWith('.googleusercontent.com') ||
        url.username || url.password || (url.port && url.port !== '443'))
      throw new ApiError('UPSTREAM_INVALID_RESPONSE');
    const response = await fetch(url.href + '=w216-h160', {
      headers: {Authorization: `Bearer ${token}`},
      redirect: 'error',
      signal: AbortSignal.timeout(10000)
    });
    if (!response.ok ||
        !/^image\/(jpeg|png|webp)/i.test(response.headers.get('content-type') || ''))
      throw new ApiError('UPSTREAM_REJECTED');
    let size = 0;
    const chunks = [];
    for await (const chunk of response.body) {
      size += chunk.length;
      if (size > 2 * 1024 * 1024) throw new ApiError('UPSTREAM_TOO_LARGE');
      chunks.push(chunk);
    }
    const image =
        await mailWorker({operation: 'image', data: Buffer.concat(chunks).toString('base64')});
    return {id, body: item.title, image, has_more: false, page: 0};
  }
}
