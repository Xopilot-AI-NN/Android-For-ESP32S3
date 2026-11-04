// DuckDuckGo's public HTML search needs no subscription. CAPTCHA is returned to the user, not
// solved.
import {ApiError} from './google.mjs';
import {mailWorker} from './mail.mjs';
export async function duckSearch(query) {
  const params = new URLSearchParams({q: query, kl: 'ru-ru', kp: '1'});
  let response;
  try {
    response = await fetch('https://html.duckduckgo.com/html/?' + params, {
      redirect: 'error',
      signal: AbortSignal.timeout(10000),
      headers: {Accept: 'text/html', 'User-Agent': 'AOSP-Wear-Gateway/1.0'}
    });
  } catch {
    throw new ApiError('UPSTREAM_UNAVAILABLE');
  }
  if ([202, 403, 429].includes(response.status)) {
    await response.body?.cancel();
    throw new ApiError('SEARCH_VERIFICATION_REQUIRED', 429);
  }
  if (!response.ok || !/^text\/html(?:;|$)/i.test(response.headers.get('content-type') || ''))
    throw new ApiError('UPSTREAM_INVALID_RESPONSE');
  let size = 0;
  const chunks = [];
  for await (const chunk of response.body) {
    size += chunk.length;
    if (size > 400000) throw new ApiError('UPSTREAM_TOO_LARGE');
    chunks.push(chunk);
  }
  const html = Buffer.concat(chunks).toString('utf8');
  if (/challenge-form|anomaly\.js/i.test(html))
    throw new ApiError('SEARCH_VERIFICATION_REQUIRED', 429);
  const parsed = await mailWorker({operation: 'search', html});
  if (!parsed.results.length && !parsed.no_results) throw new ApiError('UPSTREAM_INVALID_RESPONSE');
  const results = [];
  for (const row of parsed.results) {
    try {
      let url = new URL(row.url, 'https://duckduckgo.com');
      if (url.hostname === 'duckduckgo.com' && url.pathname === '/l/')
        url = new URL(url.searchParams.get('uddg'));
      if (url.protocol !== 'https:' || url.username || url.password ||
          (url.port && url.port !== '443'))
        continue;
      results.push({...row, url: url.href});
    } catch { /* A malformed result never becomes a capability on a watch. */
    }
  }
  return results;
}
