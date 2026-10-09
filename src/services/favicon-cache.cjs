'use strict';
/* Favicons are fetched using the tab's OWN Electron session, not the
 * privileged GEKKO renderer. Never expose remote favicon URLs to the shell.
 * Only same-site icons, png, ico, jpeg, webp and gif are allowed; the
 * downloaded bytes are bounded and rasterized into a small, inert PNG. */
const MAX_SOURCE_BYTES = 128 * 1024;
const MAX_ICON_BYTES = 48 * 1024;
const IMAGE_TYPES = new Set(['image/png','image/x-icon','image/vnd.microsoft.icon','image/jpeg','image/webp','image/gif']);
function isAllowedIconSource(raw, pageUrl) {
  try {
    const page = new URL(pageUrl);
    if (!['http:', 'https:'].includes(page.protocol)) return false;
    if (/^data:image\/(png|x-icon|vnd\.microsoft\.icon);base64,/i.test(raw)) {
      return raw.length <= MAX_SOURCE_BYTES * 2;
    }
    const icon = new URL(raw);
    if (icon.protocol !== 'https:' && icon.protocol !== 'http:') return false;
    if (icon.username || icon.password || icon.hostname === 'localhost' || icon.hostname.endsWith('.local')) return false;
    if (icon.protocol === 'http:' && page.protocol === 'https:') return false;
    const from = icon.hostname.toLowerCase();
    const on = page.hostname.toLowerCase();
    // A page may use its own subdomains for its icon. Do not contact an
    // arbitrary third-party URL suggested by an untrusted page.
    return from === on || from.endsWith('.' + on) || on.endsWith('.' + from);
  } catch {
    return false;
  }
}
async function fetchSmallIcon({ candidates, pageUrl, fetch, nativeImage, signal }) {
  if (typeof fetch !== 'function' || !nativeImage) return '';
  for (const raw of (Array.isArray(candidates) ? candidates : []).slice(0, 8)) {
    if (typeof raw !== 'string' || !isAllowedIconSource(raw, pageUrl)) continue;
    try {
      let bytes;
      if (raw.startsWith('data:')) {
        bytes = Buffer.from(raw.slice(raw.indexOf(',') + 1), 'base64');
        if (bytes.length > MAX_SOURCE_BYTES) continue;
      } else {
        const res = await fetch(raw, { redirect: 'error', signal });
        if (!res?.ok) continue;
        const type = String(res.headers?.get?.('content-type') || '').split(';')[0].trim().toLowerCase();
        if (type && !IMAGE_TYPES.has(type) && type !== 'application/octet-stream') continue;
        const declared = Number(res.headers?.get?.('content-length') || 0);
        if (declared > MAX_SOURCE_BYTES || !res.body?.getReader) continue;
        const reader = res.body.getReader();
        const chunks = []; let total = 0;
        try {
          while (true) {
            const { done, value } = await reader.read();
            if (done) break;
            total += value.byteLength;
            if (total > MAX_SOURCE_BYTES) { await reader.cancel(); throw new Error('favicon_too_large'); }
            chunks.push(Buffer.from(value));
          }
        } finally { try { reader.releaseLock(); } catch {} }
        bytes = Buffer.concat(chunks, total);
      }
      if (!bytes || !bytes.length) continue;
      const source = nativeImage.createFromBuffer(bytes);
      if (source.isEmpty()) continue;
      const png = source.resize({ width: 32, height: 32, quality: 'best' }).toPNG();
      if (png.length > MAX_ICON_BYTES || !png.length) continue;
      return 'data:image/png;base64,' + png.toString('base64');
    } catch { /* A missing favicon never blocks page rendering. */ }
  }
  return '';
}
module.exports = { isAllowedIconSource, fetchSmallIcon };
