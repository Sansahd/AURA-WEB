// Small, predictable network filter. It deliberately avoids header rewriting so
// the hot request path stays cheap and normal media delivery remains compatible.
const BLOCKED_HOST_SUFFIXES = [
  'adnxs.com', 'criteo.com', 'criteo.net', 'taboola.com', 'outbrain.com',
  'scorecardresearch.com', 'adsrvr.org', 'amazon-adsystem.com',
  'hotjar.com', 'mouseflow.com', 'clarity.ms'
];

const GOOGLE_AD_SUFFIXES = [
  'doubleclick.net',
  'googlesyndication.com',
  'googleadservices.com'
];

const GOOGLE_COMPAT_SUFFIXES = [
  'youtube.com', 'youtu.be', 'google.com', 'google.fr',
  'googleusercontent.com', 'ytimg.com'
];

const YOUTUBE_SUFFIXES = ['youtube.com', 'youtu.be', 'youtube-nocookie.com'];
const YOUTUBE_MEDIA_SUFFIXES = ['googlevideo.com', 'ytimg.com', 'ggpht.com'];

function parsedUrl(raw) {
  try { return new URL(raw); } catch { return null; }
}

function isCloudflareCritical(raw) {
  const u = parsedUrl(raw);
  if (!u) return false;
  const host = u.hostname.toLowerCase();
  return host === 'challenges.cloudflare.com' ||
    host.endsWith('.challenges.cloudflare.com') ||
    u.pathname.startsWith('/cdn-cgi/');
}

function hostMatches(host, suffix) {
  return host === suffix || host.endsWith(`.${suffix}`);
}

function initiatorHost(raw = '') {
  try { return new URL(raw).hostname.toLowerCase(); } catch { return ''; }
}

function isYouTubeHost(host = '') {
  return YOUTUBE_SUFFIXES.some((suffix) => hostMatches(host, suffix));
}

function isYouTubeInitiator(raw = '') {
  return isYouTubeHost(initiatorHost(raw));
}

function isYouTubeMediaHost(host = '') {
  return YOUTUBE_MEDIA_SUFFIXES.some((suffix) => hostMatches(host, suffix));
}

function isYouTubeAdRequest(u, initiator = '') {
  const host = u.hostname.toLowerCase();
  const pathname = u.pathname.toLowerCase();

  // Never interfere with the actual audio/video/image delivery path.
  if (isYouTubeMediaHost(host)) return false;

  // YouTube still calls Google's dedicated advertising infrastructure. Those
  // requests are safe to cancel when the caller is a YouTube document.
  if (
    GOOGLE_AD_SUFFIXES.some((suffix) => hostMatches(host, suffix)) &&
    isYouTubeInitiator(initiator)
  ) return true;

  if (!isYouTubeHost(host)) return false;

  // Explicit advertising/tracking endpoints used by the website player.
  if (
    pathname.startsWith('/pagead/') ||
    pathname === '/api/stats/ads' ||
    pathname.startsWith('/api/stats/ads/') ||
    pathname === '/ptracking' ||
    pathname.startsWith('/ptracking/')
  ) return true;

  // Common ad ping variants. Restrict them to unmistakably ad-labelled URLs so
  // normal watch history, playback telemetry and recommendations are untouched.
  if (
    pathname.includes('/pagead/') ||
    /(?:^|[?&])(ad_type|adformat|adunit|ad_id|adid)=/i.test(u.search)
  ) return true;

  return false;
}

function shouldBlock(raw, initiator = '') {
  const u = parsedUrl(raw);
  if (!u) return false;
  const host = u.hostname.toLowerCase();

  if (
    host === 'challenges.cloudflare.com' ||
    host.endsWith('.challenges.cloudflare.com') ||
    u.pathname.startsWith('/cdn-cgi/')
  ) return false;

  if (isYouTubeAdRequest(u, initiator)) return true;

  if (GOOGLE_AD_SUFFIXES.some((suffix) => hostMatches(host, suffix))) {
    const sourceHost = initiatorHost(initiator);

    // Outside YouTube, keep the compatibility exception for Google-owned pages.
    // YouTube is handled above by the stricter ad-only policy.
    if (
      GOOGLE_COMPAT_SUFFIXES.some((suffix) => hostMatches(sourceHost, suffix)) &&
      !isYouTubeHost(sourceHost)
    ) return false;

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

module.exports = {
  installPrivacyLayer,
  shouldBlock,
  isCloudflareCritical,
  isYouTubeAdRequest,
  isYouTubeHost,
  isYouTubeMediaHost
};
