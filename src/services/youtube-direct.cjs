'use strict';

const DIRECT_SCHEME_PREFIX = 'gekko-direct://youtube/';
const GOOGLEVIDEO_SUFFIX = 'googlevideo.com';
const DIRECT_CACHE_MS = 20 * 60_000;
const MAX_DIRECT_HEIGHT = 2160;
const SYNC_HARD_DRIFT_SEC = 0.24;
const SYNC_SOFT_DRIFT_SEC = 0.08;

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

function codecFromMime(mime = '') {
  const match = String(mime).match(/codecs?="([^"]+)"/i);
  return match?.[1] || '';
}

function mimeOf(format = {}) {
  return String(format.mime_type || format.mimeType || '');
}

function codecRank(format = {}, kind = 'video') {
  const codec = codecFromMime(mimeOf(format)).toLowerCase();
  if (kind === 'audio') {
    if (codec.includes('opus')) return 4;
    if (codec.includes('mp4a')) return 3;
    if (codec.includes('vorbis')) return 2;
    return 1;
  }

  // VP9 is the most conservative Chromium choice for high-resolution YouTube.
  // AV1 is excellent when available but can be more hardware-sensitive.
  if (codec.includes('vp9') || codec.includes('vp09')) return 5;
  if (codec.includes('av01')) return 4;
  if (codec.includes('avc1') || codec.includes('avc')) return 3;
  if (codec.includes('hev1') || codec.includes('hvc1')) return 2;
  return 1;
}

function isVideoOnly(format = {}) {
  return Boolean(format.has_video || format.hasVideo) &&
    !Boolean(format.has_audio || format.hasAudio) &&
    mimeOf(format).toLowerCase().startsWith('video/');
}

function isAudioOnly(format = {}) {
  return Boolean(format.has_audio || format.hasAudio) &&
    !Boolean(format.has_video || format.hasVideo) &&
    mimeOf(format).toLowerCase().startsWith('audio/');
}

function videoScore(format = {}) {
  const height = Number(format.height || 0);
  const fps = Number(format.fps || 0);
  const bitrate = Number(format.bitrate || format.average_bitrate || 0);
  return [height, Math.min(fps, 60), codecRank(format, 'video'), bitrate];
}

function audioScore(format = {}) {
  const bitrate = Number(format.bitrate || format.average_bitrate || 0);
  const channels = Number(format.audio_channels || format.audioChannels || 0);
  const sampleRate = Number(format.audio_sample_rate || format.audioSampleRate || 0);
  return [codecRank(format, 'audio'), bitrate, channels, sampleRate];
}

function compareScoreDesc(a, b, scorer) {
  const left = scorer(a);
  const right = scorer(b);
  for (let i = 0; i < Math.max(left.length, right.length); i += 1) {
    const diff = Number(right[i] || 0) - Number(left[i] || 0);
    if (diff) return diff;
  }
  return 0;
}

function selectAdaptiveFormats(formats = [], maxHeight = MAX_DIRECT_HEIGHT) {
  const usable = Array.isArray(formats) ? formats.filter((format) => !isDrmFormat(format)) : [];

  let videos = usable.filter((format) => {
    const height = Number(format.height || 0);
    return isVideoOnly(format) && height > 0 && height <= maxHeight;
  });

  // If metadata omitted height but still exposes a video track, keep it as a
  // last resort rather than throwing away an otherwise valid adaptive stream.
  if (!videos.length) videos = usable.filter((format) => isVideoOnly(format));

  const audios = usable.filter((format) => isAudioOnly(format));

  videos.sort((a, b) => compareScoreDesc(a, b, videoScore));
  audios.sort((a, b) => compareScoreDesc(a, b, audioScore));

  return {
    video: videos[0] || null,
    audio: audios[0] || null,
    availableHeights: [...new Set(videos.map((f) => Number(f.height || 0)).filter(Boolean))].sort((a, b) => b - a)
  };
}

function streamMetadata(format = {}, videoId = '') {
  const mime = mimeOf(format);
  const quality = String(format.quality_label || format.qualityLabel || format.quality || '');
  return {
    videoId,
    url: String(format.url || ''),
    mime,
    codec: codecFromMime(mime),
    quality,
    width: Number(format.width || 0),
    height: Number(format.height || 0),
    fps: Number(format.fps || 0),
    bitrate: Number(format.bitrate || format.average_bitrate || 0),
    audioChannels: Number(format.audio_channels || format.audioChannels || 0),
    audioSampleRate: Number(format.audio_sample_rate || format.audioSampleRate || 0)
  };
}

function streamIsUsable(stream = {}) {
  if (stream.mode === 'adaptive') {
    return isAllowedDirectStreamUrl(stream.video?.url) && isAllowedDirectStreamUrl(stream.audio?.url);
  }
  return isAllowedDirectStreamUrl(stream.url);
}

async function decipherFormat(format, player, videoId) {
  if (!format || isDrmFormat(format)) return null;
  const deciphered = await format.decipher(player);
  const copy = { ...streamMetadata(format, videoId), url: String(deciphered || format.url || '') };
  return isAllowedDirectStreamUrl(copy.url) ? copy : null;
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
    if (cached && Date.now() - cached.resolvedAt < DIRECT_CACHE_MS && streamIsUsable(cached)) {
      return cached;
    }

    const youtube = await this.client();
    const info = await youtube.getBasicInfo(videoId);
    const status = String(info?.playability_status?.status || '');
    if (status && status !== 'OK') {
      throw new Error(`Flux direct indisponible: ${status}`);
    }

    const player = youtube.session?.player;
    const adaptiveFormats = info?.streaming_data?.adaptive_formats || [];
    const selected = selectAdaptiveFormats(adaptiveFormats, MAX_DIRECT_HEIGHT);

    let fallback = null;
    try {
      const progressive = info?.chooseFormat?.({
        type: 'video+audio',
        quality: 'best',
        format: 'mp4'
      });
      if (progressive && !isDrmFormat(progressive)) {
        fallback = await decipherFormat(progressive, player, videoId);
      }
    } catch {}

    if (selected.video && selected.audio) {
      const [video, audio] = await Promise.all([
        decipherFormat(selected.video, player, videoId),
        decipherFormat(selected.audio, player, videoId)
      ]);

      if (video && audio) {
        const quality = video.quality || (video.height ? `${video.height}p${video.fps >= 50 ? video.fps : ''}` : 'HQ');
        const result = {
          mode: 'adaptive',
          videoId,
          video,
          audio,
          fallback,
          quality,
          mime: video.mime,
          availableHeights: selected.availableHeights,
          resolvedAt: Date.now()
        };
        this.cache.set(videoId, result);
        return result;
      }
    }

    if (!fallback) throw new Error('Aucun flux direct vidéo+audio utilisable');

    const result = {
      mode: 'progressive',
      ...fallback,
      fallback: null,
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
    mode: stream?.mode === 'adaptive' ? 'adaptive' : 'progressive',
    videoId: String(stream?.videoId || ''),
    quality: String(stream?.quality || ''),
    availableHeights: Array.isArray(stream?.availableHeights) ? stream.availableHeights : [],
    video: stream?.mode === 'adaptive' ? {
      url: String(stream?.video?.url || ''),
      mime: String(stream?.video?.mime || ''),
      codec: String(stream?.video?.codec || ''),
      quality: String(stream?.video?.quality || stream?.quality || ''),
      height: Number(stream?.video?.height || 0),
      fps: Number(stream?.video?.fps || 0)
    } : {
      url: String(stream?.url || ''),
      mime: String(stream?.mime || ''),
      codec: String(stream?.codec || ''),
      quality: String(stream?.quality || ''),
      height: Number(stream?.height || 0),
      fps: Number(stream?.fps || 0)
    },
    audio: stream?.mode === 'adaptive' ? {
      url: String(stream?.audio?.url || ''),
      mime: String(stream?.audio?.mime || ''),
      codec: String(stream?.audio?.codec || ''),
      bitrate: Number(stream?.audio?.bitrate || 0)
    } : null,
    fallback: stream?.fallback ? {
      url: String(stream.fallback.url || ''),
      mime: String(stream.fallback.mime || ''),
      quality: String(stream.fallback.quality || ''),
      height: Number(stream.fallback.height || 0)
    } : null
  }).replace(/</g, '\\u003c');

  return String.raw`(() => {
    const payload = ${payload};
    const KEY = '__gekkoDirectPlayerV2';
    const LEGACY_KEY = '__gekkoDirectPlayerV1';
    const HARD_DRIFT = ${SYNC_HARD_DRIFT_SEC};
    const SOFT_DRIFT = ${SYNC_SOFT_DRIFT_SEC};

    try { window[KEY]?.destroy?.(false); } catch {}
    try { window[LEGACY_KEY]?.destroy?.(false); } catch {}

    const moviePlayer = document.querySelector('#movie_player');
    if (!moviePlayer || !payload.video?.url) return { ok: false, error: 'player-unavailable' };
    if (payload.mode === 'adaptive' && !payload.audio?.url) return { ok: false, error: 'audio-unavailable' };

    const nativeVideo = moviePlayer.querySelector('video');
    const startTime = Number(nativeVideo?.currentTime || 0);
    const volume = Number.isFinite(nativeVideo?.volume) ? nativeVideo.volume : 1;
    const muted = Boolean(nativeVideo?.muted);
    const rate = Number(nativeVideo?.playbackRate || 1);

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
    video.src = payload.video.url;
    video.controls = true;
    video.autoplay = true;
    video.playsInline = true;
    video.preload = 'auto';
    video.style.cssText = 'width:100%;height:100%;object-fit:contain;background:#000';
    video.volume = volume;
    video.muted = muted;
    video.playbackRate = rate;

    const audio = payload.mode === 'adaptive' ? document.createElement('audio') : null;
    if (audio) {
      audio.src = payload.audio.url;
      audio.preload = 'auto';
      audio.volume = volume;
      audio.muted = muted;
      audio.playbackRate = rate;
      audio.style.display = 'none';
    }

    let adaptiveActive = Boolean(audio);
    let syncTimer = null;
    let destroyed = false;
    let fallbackUsed = false;

    const badge = document.createElement('div');
    const qualityLabel = payload.video.quality || payload.quality || (payload.video.height ? payload.video.height + 'p' : 'HQ');
    badge.textContent = 'GEKKO DIRECT · ' + qualityLabel + (adaptiveActive ? ' · A/V SYNC' : '');
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

    function syncAudio(force = false) {
      if (!adaptiveActive || !audio || !Number.isFinite(video.currentTime)) return;
      const drift = Number(audio.currentTime || 0) - Number(video.currentTime || 0);

      try {
        audio.volume = video.volume;
        audio.muted = video.muted;
      } catch {}

      if (force || !Number.isFinite(drift) || Math.abs(drift) >= HARD_DRIFT) {
        try { audio.currentTime = video.currentTime; } catch {}
        try { audio.playbackRate = video.playbackRate; } catch {}
        return;
      }

      const targetRate = Number(video.playbackRate || 1);
      if (Math.abs(drift) >= SOFT_DRIFT) {
        const correction = drift > 0 ? -0.025 : 0.025;
        try { audio.playbackRate = Math.max(0.25, Math.min(4, targetRate + correction)); } catch {}
      } else if (Math.abs(Number(audio.playbackRate || 1) - targetRate) > 0.001) {
        try { audio.playbackRate = targetRate; } catch {}
      }
    }

    function playAudio() {
      if (!adaptiveActive || !audio) return;
      syncAudio(true);
      try { audio.play?.().catch?.(() => {}); } catch {}
    }

    function pauseAudio() {
      if (!audio) return;
      try { audio.pause(); } catch {}
    }

    function beginSyncLoop() {
      clearInterval(syncTimer);
      if (!adaptiveActive) return;
      syncTimer = setInterval(() => {
        if (destroyed || video.paused || video.ended) return;
        syncAudio(false);
      }, 250);
    }

    function useProgressiveFallback(reason = '') {
      if (fallbackUsed || !payload.fallback?.url) return false;
      fallbackUsed = true;
      adaptiveActive = false;
      clearInterval(syncTimer);
      pauseAudio();

      const at = Number(video.currentTime || startTime || 0);
      try {
        video.pause();
        video.removeAttribute('src');
        video.load();
        video.src = payload.fallback.url;
        video.volume = volume;
        video.muted = muted;
        video.playbackRate = rate;
      } catch {
        return false;
      }

      badge.textContent = 'GEKKO DIRECT · ' + (payload.fallback.quality || 'fallback') + ' · FALLBACK';

      video.addEventListener('loadedmetadata', () => {
        try {
          if (at > 0 && Number.isFinite(video.duration) && at < video.duration - 1) video.currentTime = at;
          video.play?.().catch?.(() => {});
        } catch {}
      }, { once: true });
      return true;
    }

    function destroy(resume = true, cooldownMs = 0) {
      if (destroyed) return;
      destroyed = true;
      clearInterval(syncTimer);
      try { video.pause(); } catch {}
      try { audio?.pause(); } catch {}
      try { overlay.remove(); } catch {}
      try { window.__gekkoYoutubeAdGuardV1?.resetDirect?.(cooldownMs); } catch {}

      if (resume && nativeVideo?.isConnected) {
        try {
          nativeVideo.currentTime = Number(video.currentTime || startTime || 0);
          nativeVideo.volume = video.volume;
          nativeVideo.muted = video.muted;
          nativeVideo.playbackRate = video.playbackRate;
          nativeVideo.play?.().catch?.(() => {});
        } catch {}
      }

      try { delete window[KEY]; } catch {}
      try { delete window[LEGACY_KEY]; } catch {}
    }

    back.addEventListener('click', () => destroy(true, 30000));

    video.addEventListener('play', playAudio);
    video.addEventListener('pause', pauseAudio);
    video.addEventListener('seeking', () => syncAudio(true));
    video.addEventListener('seeked', () => syncAudio(true));
    video.addEventListener('ratechange', () => {
      if (audio && adaptiveActive) {
        try { audio.playbackRate = video.playbackRate; } catch {}
      }
    });
    video.addEventListener('volumechange', () => {
      if (audio && adaptiveActive) {
        try {
          audio.volume = video.volume;
          audio.muted = video.muted;
        } catch {}
      }
    });
    video.addEventListener('ended', pauseAudio);

    video.addEventListener('loadedmetadata', () => {
      if (startTime > 0 && Number.isFinite(video.duration) && startTime < video.duration - 1) {
        try { video.currentTime = startTime; } catch {}
      }
      if (adaptiveActive) syncAudio(true);
      try { video.play?.().catch?.(() => {}); } catch {}
    }, { once: true });

    video.addEventListener('error', () => {
      if (!useProgressiveFallback('video-error')) destroy(true, 10000);
    });

    if (audio) {
      audio.addEventListener('loadedmetadata', () => syncAudio(true), { once: true });
      audio.addEventListener('error', () => {
        if (!useProgressiveFallback('audio-error')) destroy(true, 10000);
      });
      audio.addEventListener('stalled', () => syncAudio(true));
    }

    overlay.addEventListener('pointerdown', () => {
      if (!video.paused && adaptiveActive) playAudio();
    }, { passive: true });

    overlay.append(video);
    if (audio) overlay.append(audio);
    overlay.append(badge, back);
    moviePlayer.appendChild(overlay);

    beginSyncLoop();

    const api = {
      destroy,
      videoId: payload.videoId,
      snapshot: () => ({
        active: true,
        mode: adaptiveActive ? 'adaptive' : 'progressive',
        videoId: payload.videoId,
        quality: badge.textContent,
        currentTime: Number(video.currentTime || 0),
        audioTime: adaptiveActive ? Number(audio?.currentTime || 0) : null,
        drift: adaptiveActive ? Number((Number(audio?.currentTime || 0) - Number(video.currentTime || 0)).toFixed(3)) : 0,
        paused: Boolean(video.paused)
      })
    };

    window[KEY] = api;
    window[LEGACY_KEY] = api;

    return {
      ok: true,
      mode: adaptiveActive ? 'adaptive' : 'progressive',
      videoId: payload.videoId,
      quality: qualityLabel,
      height: Number(payload.video.height || 0),
      fps: Number(payload.video.fps || 0)
    };
  })()`;
}

function destroyDirectPlayerSource() {
  return String.raw`(() => {
    try {
      const player = window.__gekkoDirectPlayerV2 || window.__gekkoDirectPlayerV1;
      player?.destroy?.(false);
      return true;
    } catch {
      return false;
    }
  })()`;
}

async function installYouTubeDirectPlayer(webContents, stream) {
  if (!webContents || webContents.isDestroyed?.()) return { ok: false, error: 'webcontents-unavailable' };
  if (!streamIsUsable(stream)) return { ok: false, error: 'stream-origin-rejected' };
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
  MAX_DIRECT_HEIGHT,
  SYNC_HARD_DRIFT_SEC,
  SYNC_SOFT_DRIFT_SEC,
  extractYouTubeVideoId,
  directRequestUrl,
  parseDirectRequest,
  isAllowedDirectStreamUrl,
  isDrmFormat,
  codecFromMime,
  isVideoOnly,
  isAudioOnly,
  selectAdaptiveFormats,
  streamIsUsable,
  YouTubeDirectResolver,
  directPlayerSource,
  installYouTubeDirectPlayer,
  removeYouTubeDirectPlayer
};
