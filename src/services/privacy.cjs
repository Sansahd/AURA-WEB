// Small, predictable network filter. It deliberately avoids DOM injection and
// header rewriting so Chromium remains compatible and the hot request path stays cheap.
const BLOCKED_HOST_SUFFIXES = [
  'adnxs.com', 'criteo.com', 'criteo.net', 'taboola.com', 'outbrain.com',
  'scorecardresearch.com', 'adsrvr.org', 'amazon-adsystem.com',
  'hotjar.com', 'mouseflow.com', 'clarity.ms'
];

// Google's advertising hosts are intentionally handled separately. YouTube and
// other Google properties sometimes use the same infrastructure for player and
// consent flows. Blocking it unconditionally is not worth breaking the site.
const GOOGLE_AD_SUFFIXES = ['doubleclick.net', 'googlesyndication.com', 'googleadservices.com'];
const GOOGLE_COMPAT_SUFFIXES = ['youtube.com', 'youtu.be', 'google.com', 'google.fr', 'googleusercontent.com', 'ytimg.com'];

function parsedUrl(raw) {
  try { return new URL(raw); } catch { return null; }
}

function isCloudflareCritical(raw) {
  const u = parsedUrl(raw);
  if (!u) return false;
  const host = u.hostname.toLowerCase();
  return host === 'challenges.cloudflare.com' || host.endsWith('.challenges.cloudflare.com') || u.pathname.startsWith('/cdn-cgi/');
}

function hostMatches(host, suffix) {
  return host === suffix || host.endsWith(`.${suffix}`);
}

function initiatorHost(raw = '') {
  try { return new URL(raw).hostname.toLowerCase(); } catch { return ''; }
}

function shouldBlock(raw, initiator = '') {
  const u = parsedUrl(raw);
  if (!u) return false;
  const host = u.hostname.toLowerCase();
  if (host === 'challenges.cloudflare.com' || host.endsWith('.challenges.cloudflare.com') || u.pathname.startsWith('/cdn-cgi/')) return false;

  if (GOOGLE_AD_SUFFIXES.some((suffix) => hostMatches(host, suffix))) {
    const sourceHost = initiatorHost(initiator);
    if (GOOGLE_COMPAT_SUFFIXES.some((suffix) => hostMatches(sourceHost, suffix))) return false;
    return true;
  }

  for (const suffix of BLOCKED_HOST_SUFFIXES) {
    if (hostMatches(host, suffix)) return true;
  }
  return false;
}

function installPrivacyLayer(ses, onBlocked = () => {}) {
  ses.webRequest.onBeforeRequest((details, callback) => {
    const cancel = shouldBlock(details.url, details.initiator || details.referrer || '');
    if (cancel) onBlocked(details.url);
    callback({ cancel });
  });
}

module.exports = { installPrivacyLayer, shouldBlock, isCloudflareCritical };
