'use strict';

const DIRECT_SCHEME_PREFIX = 'gekko-direct://youtube/';
const GOOGLEVIDEO_SUFFIX = 'googlevideo.com';
const DIRECT_CACHE_MS = 20 * 60_000;
const MAX_DIRECT_HEIGHT = 2160;
const MAX_VIDEO_CANDIDATES = 6;
const MAX_AUDIO_CANDIDATES = 4;

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

function mimeBase(format = {}) {
  return String(format.mime_type || format.mimeType || '').split(';', 1)[0].trim().toLowerCase();
}

function codecName(format = {}) {
  const mime = String(format.mime_type || format.mimeType || '');
  const match = mime.match(/codecs?="([^"]+)"/i);
  return match ? match[1].toLowerCase() : '';
}

function videoCodecRank(format = {}) {
  const codec = codecName(format);
  if (/av01|av1/.test(codec)) return 4;
  if (/vp9|vp09/.test(codec)) return 3;
  if (/avc1|h264/.test(codec)) return 2;
  if (/hev1|hvc1|hevc/.test(codec)) return 1;
  return 0;
}

function audioCodecRank(format = {}) {
  const codec = codecName(format);
  if (/opus/.test(codec)) return 4;
  if (/mp4a|aac/.test(codec)) return 3;
  if (/vorbis/.test(codec)) return 2;
  return 0;
}

function isAdaptiveVideo(format = {}) {
  return Boolean(
    format.has_video &&
    !format.has_audio &&
    !format.has_text &&
    !format.is_type_otf &&
    !isDrmFormat(format) &&
    Number(format.height || 0) > 0 &&
    Number(format.height || 0) <= MAX_DIRECT_HEIGHT
  );
}

function isAdaptiveAudio(format = {}) {
  return Boolean(
    format.has_audio &&
    !format.has_video &&
    !format.has_text &&
    !format.is_type_otf &&
    !isDrmFormat(format)
  );
}

function videoScore(format = {}) {
  const height = Number(format.height || 0);
  const fps = Math.min(120, Number(format.fps || 0));
  const bitrate = Math.min(250_000_000, Number(format.bitrate || format.average_bitrate || 0));
  const codec = videoCodecRank(format);
  const standardDynamicRange = format.color_info?.transfer_characteristics === 'PQ' ||
    format.color_info?.transfer_characteristics === 'HLG' ? 0 : 1;

  return (
    height * 1_000_000_000 +
    fps * 1_000_000 +
    codec * 100_000 +
    standardDynamicRange * 10_000 +
    Math.floor(bitrate / 10_000)
  );
}

function audioScore(format = {}) {
  const bitrate = Math.min(2_000_000, Number(format.bitrate || format.average_bitrate || 0));
  const channels = Math.min(8, Number(format.audio_channels || 0));
  const codec = audioCodecRank(format);
  const defaultTrack = format.audio_track?.audio_is_default ? 1 : 0;
  const original = format.is_original ? 1 : 0;
  const nonDubbed = format.is_dubbed || format.is_auto_dubbed ? 0 : 1;
  const nonDrc = format.is_drc ? 0 : 1;

  return (
    defaultTrack * 1_000_000_000 +
    original * 100_000_000 +
    nonDubbed * 10_000_000 +
    nonDrc * 1_000_000 +
    codec * 100_000 +
    channels * 10_000 +
    Math.floor(bitrate / 100)
  );
}

function selectAdaptiveFormats(formats = []) {
  const list = Array.isArray(formats) ? formats : [];
  const video = list
    .filter(isAdaptiveVideo)
    .sort((a, b) => videoScore(b) - videoScore(a))
    .slice(0, MAX_VIDEO_CANDIDATES);

  const audio = list
    .filter(isAdaptiveAudio)
    .sort((a, b) => audioScore(b) - audioScore(a))
    .slice(0, MAX_AUDIO_CANDIDATES);

  return { video, audio };
}

function streamMetadata(format = {}, videoId = '') {
  const mime = String(format.mime_type || format.mimeType || '');
  const quality = String(format.quality_label || format.qualityLabel || format.quality || '');
  return {
    videoId,
    url: String(format.url || ''),
    mime,
    mimeBase: mimeBase(format),
    codecs: codecName(format),
    quality,
    width: Number(format.width || 0),
    height: Number(format.height || 0),
    fps: Number(format.fps || 0),
    bitrate: Number(format.bitrate || format.average_bitrate || 0),
    audioChannels: Number(format.audio_channels || 0),
    audioSampleRate: Number(format.audio_sample_rate || 0)
  };
}

async function decipherFormat(format, player, videoId) {
  if (!format || isDrmFormat(format)) return null;
  const url = String(await format.decipher(player) || format.url || '');
  if (!isAllowedDirectStreamUrl(url)) return null;
  format.url = url;
  return streamMetadata(format, videoId);
}

function validCachedStream(stream) {
  if (!stream || Date.now() - Number(stream.resolvedAt || 0) >= DIRECT_CACHE_MS) return false;
  if (stream.mode === 'adaptive') {
    return Array.isArray(stream.videoCandidates) &&
      stream.videoCandidates.some((item) => isAllowedDirectStreamUrl(item.url)) &&
      Array.isArray(stream.audioCandidates) &&
      stream.audioCandidates.some((item) => isAllowedDirectStreamUrl(item.url));
  }
  return isAllowedDirectStreamUrl(stream.url);
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
    if (validCachedStream(cached)) return cached;

    const youtube = await this.client();
    const info = await youtube.getBasicInfo(videoId);
    const status = String(info?.playability_status?.status || '');
    if (status && status !== 'OK') {
      throw new Error(`Flux direct indisponible: ${status}`);
    }

    const player = youtube.session?.player;
    const adaptive = selectAdaptiveFormats(info?.streaming_data?.adaptive_formats || []);

    let progressiveFallback = null;
    try {
      const progressiveFormat = info?.chooseFormat?.({
        type: 'video+audio',
        quality: 'best',
        format: 'mp4'
      });
      if (progressiveFormat && !isDrmFormat(progressiveFormat)) {
        progressiveFallback = await decipherFormat(progressiveFormat, player, videoId);
      }
    } catch {}

    if (adaptive.video.length && adaptive.audio.length) {
      const videoCandidates = (await Promise.all(
        adaptive.video.map((format) => decipherFormat(format, player, videoId).catch(() => null))
      )).filter(Boolean);

      const audioCandidates = (await Promise.all(
        adaptive.audio.map((format) => decipherFormat(format, player, videoId).catch(() => null))
      )).filter(Boolean);

      if (videoCandidates.length && audioCandidates.length) {
        const top = videoCandidates[0];
        const result = {
          mode: 'adaptive',
          videoId,
          quality: top.quality || (top.height ? `${top.height}p` : ''),
          width: top.width,
          height: top.height,
          fps: top.fps,
          videoCandidates,
          audioCandidates,
          progressiveFallback,
          resolvedAt: Date.now()
        };
        this.cache.set(videoId, result);
        return result;
      }
    }

    const fallback = progressiveFallback;
    if (!fallback) throw new Error('Aucun flux direct compatible');

    const result = {
      mode: 'progressive',
      ...fallback,
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
    width: Number(stream?.width || 0),
    height: Number(stream?.height || 0),
    fps: Number(stream?.fps || 0),
    url: String(stream?.url || ''),
    mime: String(stream?.mime || ''),
    videoCandidates: Array.isArray(stream?.videoCandidates) ? stream.videoCandidates : [],
    audioCandidates: Array.isArray(stream?.audioCandidates) ? stream.audioCandidates : [],
    progressiveFallback: stream?.progressiveFallback || null
  }).replace(/</g, '\\u003c');

  return String.raw`(() => {
    const payload = ${payload};
    const KEY = '__gekkoDirectPlayerV2';

    try { window.__gekkoDirectPlayerV1?.destroy?.(false); } catch {}
    try { window[KEY]?.destroy?.(false); } catch {}

    const moviePlayer = document.querySelector('#movie_player');
    if (!moviePlayer) return { ok: false, error: 'player-unavailable' };

    const nativeVideo = moviePlayer.querySelector('video');
    const startTime = Number(nativeVideo?.currentTime || 0);
    const initialVolume = Number.isFinite(nativeVideo?.volume) ? nativeVideo.volume : 1;
    const initialMuted = Boolean(nativeVideo?.muted);
    const initialRate = Number(nativeVideo?.playbackRate || 1);

    const canPlay = (candidate, kind) => {
      if (!candidate?.url) return false;
      const probe = document.createElement(kind === 'audio' ? 'audio' : 'video');
      const mime = String(candidate.mime || '');
      if (!mime || typeof probe.canPlayType !== 'function') return true;
      return probe.canPlayType(mime) !== '';
    };

    const videoCandidates = (payload.videoCandidates || []).filter((item) => canPlay(item, 'video'));
    const audioCandidates = (payload.audioCandidates || []).filter((item) => canPlay(item, 'audio'));
    const selectedVideo = videoCandidates[0] || null;
    const selectedAudio = audioCandidates[0] || null;
    const progressive = payload.progressiveFallback?.url
      ? payload.progressiveFallback
      : (payload.url ? {
          url: payload.url,
          mime: payload.mime,
          quality: payload.quality,
          height: payload.height,
          fps: payload.fps
        } : null);
    let adaptiveActive = Boolean(payload.mode === 'adaptive' && selectedVideo?.url && selectedAudio?.url);
    const primary = adaptiveActive ? selectedVideo : progressive;
    const primaryUrl = primary?.url || '';
    if (!primaryUrl) return { ok: false, error: 'stream-unavailable' };

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
    video.src = primaryUrl;
    video.controls = true;
    video.autoplay = true;
    video.playsInline = true;
    video.preload = 'auto';
    video.style.cssText = 'width:100%;height:100%;object-fit:contain;background:#000';
    video.volume = initialVolume;
    video.muted = initialMuted;
    video.playbackRate = initialRate;

    const audio = adaptiveActive ? document.createElement('audio') : null;
    if (audio) {
      audio.src = selectedAudio.url;
      audio.preload = 'auto';
      audio.volume = initialVolume;
      audio.muted = initialMuted;
      audio.playbackRate = initialRate;
      audio.style.display = 'none';
    }

    let quality = adaptiveActive
      ? String(selectedVideo.quality || (selectedVideo.height ? selectedVideo.height + 'p' : payload.quality || 'HQ'))
      : String(primary?.quality || payload.quality || 'Direct');
    let fps = adaptiveActive ? Number(selectedVideo.fps || 0) : Number(primary?.fps || payload.fps || 0);

    const badge = document.createElement('div');
    const updateBadge = () => {
      badge.textContent = 'GEKKO DIRECT · ' + quality + (fps > 30 ? ' · ' + fps + ' FPS' : '') +
        (adaptiveActive ? ' · A/V' : '');
    };
    updateBadge();
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

    let syncTimer = null;
    let destroyed = false;
    let audioFailed = false;

    const safeTime = (value) => Number.isFinite(Number(value)) ? Number(value) : 0;

    function syncAudio(force = false) {
      if (!audio || audioFailed || destroyed) return;
      const target = safeTime(video.currentTime);
      const actual = safeTime(audio.currentTime);
      const drift = Math.abs(target - actual);
      if (force || drift > 0.22) {
        try { audio.currentTime = target; } catch {}
      }
      if (audio.playbackRate !== video.playbackRate) {
        try { audio.playbackRate = video.playbackRate; } catch {}
      }
      if (audio.volume !== video.volume) audio.volume = video.volume;
      if (audio.muted !== video.muted) audio.muted = video.muted;
    }

    function startAudio() {
      if (!audio || audioFailed || video.paused || destroyed) return;
      syncAudio(true);
      try {
        const promise = audio.play();
        promise?.catch?.(() => {
          if (!downgradeToProgressive()) {
            audioFailed = true;
            destroy(true, 10000);
          }
        });
      } catch {
        if (!downgradeToProgressive()) {
          audioFailed = true;
          destroy(true, 10000);
        }
      }
    }

    function stopAudio() {
      if (!audio) return;
      try { audio.pause(); } catch {}
    }

    function downgradeToProgressive() {
      if (!adaptiveActive || !progressive?.url || destroyed) return false;
      const resumeAt = safeTime(video.currentTime);
      adaptiveActive = false;
      audioFailed = true;
      stopAudio();
      try { audio?.removeAttribute?.('src'); audio?.load?.(); } catch {}
      quality = String(progressive.quality || (progressive.height ? progressive.height + 'p' : 'Direct'));
      fps = Number(progressive.fps || 0);
      updateBadge();
      try {
        video.src = progressive.url;
        video.load();
        video.addEventListener('loadedmetadata', () => {
          if (resumeAt > 0 && Number.isFinite(video.duration) && resumeAt < video.duration - 1) {
            try { video.currentTime = resumeAt; } catch {}
          }
          try { video.play?.().catch?.(() => {}); } catch {}
        }, { once: true });
        return true;
      } catch {
        return false;
      }
    }

    function destroy(resume = true, cooldownMs = 0) {
      if (destroyed) return;
      destroyed = true;
      if (syncTimer) clearInterval(syncTimer);
      try { video.pause(); } catch {}
      try { audio?.pause?.(); } catch {}
      try { overlay.remove(); } catch {}
      try { window.__gekkoYoutubeAdGuardV1?.resetDirect?.(cooldownMs); } catch {}
      if (resume && nativeVideo?.isConnected) {
        try {
          nativeVideo.currentTime = safeTime(video.currentTime || startTime);
          nativeVideo.volume = video.volume;
          nativeVideo.muted = video.muted;
          nativeVideo.playbackRate = video.playbackRate;
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
      if (audio) {
        try { audio.currentTime = startTime; } catch {}
      }
      try { video.play?.().catch?.(() => {}); } catch {}
    }, { once: true });

    video.addEventListener('play', startAudio);
    video.addEventListener('playing', () => {
      syncAudio(true);
      startAudio();
    });
    video.addEventListener('pause', stopAudio);
    video.addEventListener('waiting', stopAudio);
    video.addEventListener('stalled', stopAudio);
    video.addEventListener('seeking', () => syncAudio(true));
    video.addEventListener('seeked', () => {
      syncAudio(true);
      startAudio();
    });
    video.addEventListener('ratechange', () => syncAudio(false));
    video.addEventListener('volumechange', () => syncAudio(false));
    video.addEventListener('ended', stopAudio);
    video.addEventListener('error', () => {
      if (!downgradeToProgressive()) destroy(true, 10000);
    });

    if (audio) {
      audio.addEventListener('error', () => {
        if (!downgradeToProgressive()) {
          audioFailed = true;
          destroy(true, 10000);
        }
      });
    }

    syncTimer = setInterval(() => {
      if (!video.paused && !video.seeking) syncAudio(false);
    }, 250);

    overlay.append(video, badge, back);
    if (audio) overlay.appendChild(audio);
    moviePlayer.appendChild(overlay);

    window[KEY] = {
      destroy,
      videoId: payload.videoId,
      snapshot: () => ({
        active: !destroyed,
        mode: adaptiveActive ? 'adaptive' : 'progressive',
        videoId: payload.videoId,
        quality,
        fps,
        currentTime: safeTime(video.currentTime),
        paused: Boolean(video.paused),
        drift: audio ? Math.abs(safeTime(video.currentTime) - safeTime(audio.currentTime)) : 0
      })
    };

    return {
      ok: true,
      mode: adaptiveActive ? 'adaptive' : 'progressive',
      videoId: payload.videoId,
      quality,
      fps,
      videoMime: adaptiveActive ? selectedVideo.mime : (progressive?.mime || payload.mime),
      audioMime: adaptiveActive ? selectedAudio.mime : ''
    };
  })()`;
}

function destroyDirectPlayerSource() {
  return String.raw`(() => {
    try {
      window.__gekkoDirectPlayerV2?.destroy?.(false);
      window.__gekkoDirectPlayerV1?.destroy?.(false);
      return true;
    } catch {
      return false;
    }
  })()`;
}

function streamPayloadIsAllowed(stream) {
  if (!stream) return false;
  if (stream.mode === 'adaptive') {
    const video = Array.isArray(stream.videoCandidates) ? stream.videoCandidates : [];
    const audio = Array.isArray(stream.audioCandidates) ? stream.audioCandidates : [];
    const fallbackOk = !stream.progressiveFallback ||
      isAllowedDirectStreamUrl(stream.progressiveFallback.url);
    return video.length > 0 &&
      audio.length > 0 &&
      video.every((item) => isAllowedDirectStreamUrl(item.url)) &&
      audio.every((item) => isAllowedDirectStreamUrl(item.url)) &&
      fallbackOk;
  }
  return isAllowedDirectStreamUrl(stream.url);
}

async function installYouTubeDirectPlayer(webContents, stream) {
  if (!webContents || webContents.isDestroyed?.()) return { ok: false, error: 'webcontents-unavailable' };
  if (!streamPayloadIsAllowed(stream)) return { ok: false, error: 'stream-origin-rejected' };
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
  extractYouTubeVideoId,
  directRequestUrl,
  parseDirectRequest,
  isAllowedDirectStreamUrl,
  isDrmFormat,
  isAdaptiveVideo,
  isAdaptiveAudio,
  videoScore,
  audioScore,
  selectAdaptiveFormats,
  YouTubeDirectResolver,
  directPlayerSource,
  streamPayloadIsAllowed,
  installYouTubeDirectPlayer,
  removeYouTubeDirectPlayer
};
