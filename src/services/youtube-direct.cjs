'use strict';

const DIRECT_SCHEME_PREFIX = 'gekko-direct://youtube/';
const GOOGLEVIDEO_SUFFIX = 'googlevideo.com';
const DIRECT_CACHE_MS = 20 * 60_000;

function hostMatches(host, suffix) {
  return host === suffix || host.endsWith(`.${suffix}`);
}

function extractYouTubeVideoId(raw = '') {
  try {
    const url = new URL(raw);
    const host = url.hostname.toLowerCase();

    if (hostMatches(host, 'youtu.be')) {
      return url.pathname.split('/').filter(Boolean)[0] || '';
    }

    if (!hostMatches(host, 'youtube.com') && !hostMatches(host, 'youtube-nocookie.com')) return '';

    if (url.pathname === '/watch') return url.searchParams.get('v') || '';

    const parts = url.pathname.split('/').filter(Boolean);
    if (['shorts', 'embed', 'live'].includes(parts[0])) return parts[1] || '';
    return '';
  } catch {
    return '';
  }
}

function directRequestUrl(videoId) {
  const id = String(videoId || '').trim();
  if (!/^[A-Za-z0-9_-]{6,32}$/.test(id)) return '';
  return `${DIRECT_SCHEME_PREFIX}${encodeURIComponent(id)}`;
}

function parseDirectRequest(raw = '') {
  const value = String(raw || '');
  if (!value.startsWith(DIRECT_SCHEME_PREFIX)) return '';
  try {
    const id = decodeURIComponent(value.slice(DIRECT_SCHEME_PREFIX.length).split(/[?#]/, 1)[0] || '');
    return /^[A-Za-z0-9_-]{6,32}$/.test(id) ? id : '';
  } catch {
    return '';
  }
}

function isAllowedDirectStreamUrl(raw = '') {
  try {
    const url = new URL(raw);
    return url.protocol === 'https:' && hostMatches(url.hostname.toLowerCase(), GOOGLEVIDEO_SUFFIX);
  } catch {
    return false;
  }
}

function isDrmFormat(format) {
  return Boolean(
    format?.is_drm ||
    format?.isDrm ||
    format?.drm ||
    format?.drm_track_type ||
    (Array.isArray(format?.drm_families) && format.drm_families.length)
  );
}

function streamMetadata(format = {}, videoId = '') {
  const mime = String(format.mime_type || format.mimeType || '');
  const quality = String(format.quality_label || format.qualityLabel || format.quality || '');
  return {
    videoId,
    url: String(format.url || ''),
    mime,
    quality,
    width: Number(format.width || 0),
    height: Number(format.height || 0),
    bitrate: Number(format.bitrate || 0)
  };
}

class YouTubeDirectResolver {
  constructor() {
    this.clientPromise = null;
    this.cache = new Map();
  }

  async client() {
    if (!this.clientPromise) {
      this.clientPromise = import('youtubei.js')
        .then(async (mod) => {
          const Innertube = mod.Innertube || mod.default?.Innertube || mod.default;
          if (!Innertube?.create) throw new Error('youtubei.js Innertube indisponible');
          return Innertube.create();
        })
        .catch((error) => {
          this.clientPromise = null;
          throw error;
        });
    }
    return this.clientPromise;
  }

  async resolve(input) {
    const videoId = extractYouTubeVideoId(input) || (
      /^[A-Za-z0-9_-]{6,32}$/.test(String(input || '')) ? String(input) : ''
    );
    if (!videoId) throw new Error('Identifiant YouTube invalide');

    const cached = this.cache.get(videoId);
    if (cached && Date.now() - cached.resolvedAt < DIRECT_CACHE_MS && isAllowedDirectStreamUrl(cached.url)) {
      return cached;
    }

    const youtube = await this.client();
    const info = await youtube.getBasicInfo(videoId);
    const status = String(info?.playability_status?.status || '');
    if (status && status !== 'OK') {
      throw new Error(`Flux direct indisponible: ${status}`);
    }

    const format = info?.chooseFormat?.({
      type: 'video+audio',
      quality: 'best',
      format: 'mp4'
    });

    if (!format) throw new Error('Aucun flux vidéo+audio direct');
    if (isDrmFormat(format)) throw new Error('Contenu DRM: mode direct désactivé');

    const deciphered = await format.decipher(youtube.session?.player);
    format.url = String(deciphered || format.url || '');

    if (!isAllowedDirectStreamUrl(format.url)) {
      throw new Error('Origine du flux direct refusée');
    }

    const result = {
      ...streamMetadata(format, videoId),
      resolvedAt: Date.now()
    };

    this.cache.set(videoId, result);
    return result;
  }

  clear(videoId = '') {
    if (videoId) this.cache.delete(videoId);
    else this.cache.clear();
  }
}

function directPlayerSource(stream) {
  const payload = JSON.stringify({
    videoId: String(stream?.videoId || ''),
    url: String(stream?.url || ''),
    quality: String(stream?.quality || ''),
    mime: String(stream?.mime || '')
  }).replace(/</g, '\\u003c');

  return String.raw`(() => {
    const payload = ${payload};
    const KEY = '__gekkoDirectPlayerV1';

    try { window[KEY]?.destroy?.(); } catch {}

    const moviePlayer = document.querySelector('#movie_player');
    if (!moviePlayer || !payload.url) return { ok: false, error: 'player-unavailable' };

    const nativeVideo = moviePlayer.querySelector('video');
    const startTime = Number(nativeVideo?.currentTime || 0);
    const volume = Number.isFinite(nativeVideo?.volume) ? nativeVideo.volume : 1;
    const muted = Boolean(nativeVideo?.muted);

    try { nativeVideo?.pause?.(); } catch {}

    const overlay = document.createElement('div');
    overlay.id = 'gekko-direct-player';
    overlay.style.cssText = [
      'position:absolute',
      'inset:0',
      'z-index:2147483000',
      'background:#02070b',
      'display:flex',
      'align-items:center',
      'justify-content:center',
      'overflow:hidden'
    ].join(';');

    const video = document.createElement('video');
    video.src = payload.url;
    video.controls = true;
    video.autoplay = true;
    video.playsInline = true;
    video.preload = 'auto';
    video.style.cssText = 'width:100%;height:100%;object-fit:contain;background:#000';
    video.volume = volume;
    video.muted = muted;

    const badge = document.createElement('div');
    badge.textContent = 'GEKKO DIRECT' + (payload.quality ? ' · ' + payload.quality : '');
    badge.style.cssText = [
      'position:absolute',
      'top:12px',
      'left:12px',
      'z-index:3',
      'font:600 11px/1.2 system-ui,sans-serif',
      'letter-spacing:.08em',
      'padding:7px 10px',
      'border:1px solid rgba(122,255,225,.35)',
      'border-radius:999px',
      'color:#d9fff7',
      'background:rgba(3,20,24,.72)',
      'backdrop-filter:blur(12px)',
      'pointer-events:none'
    ].join(';');

    const back = document.createElement('button');
    back.type = 'button';
    back.textContent = 'Player YouTube';
    back.style.cssText = [
      'position:absolute',
      'top:10px',
      'right:12px',
      'z-index:4',
      'font:600 12px system-ui,sans-serif',
      'padding:8px 12px',
      'border:1px solid rgba(255,255,255,.22)',
      'border-radius:999px',
      'color:#fff',
      'background:rgba(10,14,18,.72)',
      'cursor:pointer',
      'backdrop-filter:blur(12px)'
    ].join(';');

    function destroy(resume = true, cooldownMs = 0) {
      try { video.pause(); } catch {}
      try { overlay.remove(); } catch {}
      try { window.__gekkoYoutubeAdGuardV1?.resetDirect?.(cooldownMs); } catch {}
      if (resume && nativeVideo?.isConnected) {
        try {
          nativeVideo.currentTime = Number(video.currentTime || startTime || 0);
          nativeVideo.volume = video.volume;
          nativeVideo.muted = video.muted;
          nativeVideo.play?.().catch?.(() => {});
        } catch {}
      }
      try { delete window[KEY]; } catch {}
    }

    back.addEventListener('click', () => destroy(true, 30000));

    video.addEventListener('loadedmetadata', () => {
      if (startTime > 0 && Number.isFinite(video.duration) && startTime < video.duration - 1) {
        try { video.currentTime = startTime; } catch {}
      }
      try { video.play?.().catch?.(() => {}); } catch {}
    }, { once: true });

    video.addEventListener('error', () => destroy(true, 10000), { once: true });

    overlay.append(video, badge, back);
    moviePlayer.appendChild(overlay);

    window[KEY] = {
      destroy,
      videoId: payload.videoId,
      snapshot: () => ({
        active: true,
        videoId: payload.videoId,
        currentTime: Number(video.currentTime || 0),
        paused: Boolean(video.paused)
      })
    };

    return { ok: true, videoId: payload.videoId, quality: payload.quality };
  })()`;
}

function destroyDirectPlayerSource() {
  return String.raw`(() => {
    try {
      window.__gekkoDirectPlayerV1?.destroy?.(false);
      return true;
    } catch {
      return false;
    }
  })()`;
}

async function installYouTubeDirectPlayer(webContents, stream) {
  if (!webContents || webContents.isDestroyed?.()) return { ok: false, error: 'webcontents-unavailable' };
  if (!isAllowedDirectStreamUrl(stream?.url)) return { ok: false, error: 'stream-origin-rejected' };
  try {
    const result = await webContents.executeJavaScript(directPlayerSource(stream), true);
    return result || { ok: false, error: 'inject-failed' };
  } catch (error) {
    return { ok: false, error: error?.message || String(error) };
  }
}

async function removeYouTubeDirectPlayer(webContents) {
  if (!webContents || webContents.isDestroyed?.()) return false;
  try {
    return Boolean(await webContents.executeJavaScript(destroyDirectPlayerSource(), true));
  } catch {
    return false;
  }
}

module.exports = {
  DIRECT_SCHEME_PREFIX,
  GOOGLEVIDEO_SUFFIX,
  extractYouTubeVideoId,
  directRequestUrl,
  parseDirectRequest,
  isAllowedDirectStreamUrl,
  isDrmFormat,
  YouTubeDirectResolver,
  directPlayerSource,
  installYouTubeDirectPlayer,
  removeYouTubeDirectPlayer
};
