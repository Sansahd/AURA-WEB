'use strict';

const YOUTUBE_HOST_SUFFIXES = [
  'youtube.com',
  'youtu.be',
  'youtube-nocookie.com',
  'youtubei.googleapis.com'
];

function hostMatches(host, suffix) {
  return host === suffix || host.endsWith(`.${suffix}`);
}

function isYouTubeUrl(raw = '') {
  try {
    const host = new URL(raw).hostname.toLowerCase();
    return YOUTUBE_HOST_SUFFIXES.some((suffix) => hostMatches(host, suffix));
  } catch {
    return false;
  }
}

function youtubeGuardSource() {
  return String.raw`(() => {
    const KEY = '__gekkoYoutubeAdGuardV1';
    if (window[KEY]) {
      try { window[KEY].refresh(); } catch {}
      return true;
    }

    const PLAYER_ENDPOINT = /\\/youtubei\\/v1\\/player(?:[/?#]|$)/i;
    const AD_KEYS = new Set([
      'adPlacements',
      'playerAds',
      'adSlots',
      'adBreakParams',
      'adParams',
      'adPlacementConfig',
      'adSafetyReason',
      'adBreakHeartbeatParams',
      'adTrackingParams',
      'adSignalsInfo',
      'adServingDataEntry',
      'adPlaybackContext',
      'adInfoRenderer',
      'instreamAdPlayerOverlayRenderer',
      'promotedSparklesWebRenderer',
      'promotedVideoRenderer',
      'displayAdRenderer',
      'mastheadAdRenderer'
    ]);

    const state = {
      restore: null,
      scheduled: false,
      interval: null,
      observer: null,
      patchedResponse: false,
      patchedFetch: false,
      patchedJson: false,
      adSince: 0,
      directRequested: false,
      directCooldownUntil: 0,
      directFailedFor: '',
      directFallbackAfter: 0,
      lastVideoId: '',
      shield: null,
      shieldRestore: null,
      adCornerRestore: null,
      adCornerBadge: null
    };

    function scrub(value, seen = new WeakSet()) {
      if (!value || typeof value !== 'object') return value;
      if (seen.has(value)) return value;
      seen.add(value);

      if (Array.isArray(value)) {
        for (let i = value.length - 1; i >= 0; i -= 1) {
          const item = value[i];
          if (item && typeof item === 'object') scrub(item, seen);
        }
        return value;
      }

      for (const key of Object.keys(value)) {
        if (AD_KEYS.has(key)) {
          try { delete value[key]; } catch {}
          continue;
        }

        const lower = key.toLowerCase();
        if (
          lower === 'adplacements' ||
          lower === 'playerads' ||
          lower === 'adslots' ||
          lower === 'adbreakparams' ||
          lower === 'adparams'
        ) {
          try { delete value[key]; } catch {}
          continue;
        }

        const child = value[key];
        if (child && typeof child === 'object') scrub(child, seen);
      }
      return value;
    }

    function scrubKnownGlobals() {
      for (const key of ['ytInitialPlayerResponse', 'ytplayer']) {
        try {
          const value = window[key];
          if (value && typeof value === 'object') scrub(value);
        } catch {}
      }
    }

    function patchResponseReaders() {
      if (state.patchedResponse || typeof Response === 'undefined') return;
      state.patchedResponse = true;

      const nativeJson = Response.prototype.json;
      const nativeText = Response.prototype.text;

      if (typeof nativeJson === 'function') {
        Response.prototype.json = async function gekkoJson() {
          const data = await nativeJson.call(this);
          try {
            if (PLAYER_ENDPOINT.test(String(this.url || ''))) scrub(data);
          } catch {}
          return data;
        };
      }

      if (typeof nativeText === 'function') {
        Response.prototype.text = async function gekkoText() {
          const text = await nativeText.call(this);
          try {
            if (!PLAYER_ENDPOINT.test(String(this.url || ''))) return text;
            const parsed = JSON.parse(text);
            scrub(parsed);
            return JSON.stringify(parsed);
          } catch {
            return text;
          }
        };
      }
    }

    function patchFetch() {
      if (state.patchedFetch || typeof window.fetch !== 'function') return;
      state.patchedFetch = true;
      const nativeFetch = window.fetch.bind(window);
      window.fetch = async function gekkoFetch(...args) {
        const response = await nativeFetch(...args);
        let url = '';
        try { url = String(response?.url || args?.[0]?.url || args?.[0] || ''); } catch {}
        if (!PLAYER_ENDPOINT.test(url)) return response;
        try {
          const text = await response.clone().text();
          const parsed = JSON.parse(text);
          scrub(parsed);
          const headers = new Headers(response.headers);
          headers.delete('content-length');
          return new Response(JSON.stringify(parsed), {
            status: response.status,
            statusText: response.statusText,
            headers
          });
        } catch {
          return response;
        }
      };
    }

    function patchJsonParser() {
      if (state.patchedJson || typeof JSON?.parse !== 'function') return;
      state.patchedJson = true;
      const nativeParse = JSON.parse;
      JSON.parse = function gekkoJsonParse(text, ...rest) {
        const data = nativeParse.call(this, text, ...rest);
        try {
          if (
            typeof text === 'string' &&
            (text.includes('"adPlacements"') || text.includes('"playerAds"') || text.includes('"adSlots"')) &&
            (text.includes('"streamingData"') || text.includes('"playabilityStatus"'))
          ) scrub(data);
        } catch {}
        return data;
      };
    }

    function currentVideoId() {
      try {
        const url = new URL(location.href);
        if (url.pathname === '/watch') return url.searchParams.get('v') || '';
        const parts = url.pathname.split('/').filter(Boolean);
        if (['shorts', 'embed', 'live'].includes(parts[0])) return parts[1] || '';
      } catch {}
      return '';
    }

    function adIsShowing() {
      const player = document.querySelector('#movie_player');
      if (!player) return false;
      if (player.classList.contains('ad-showing') || player.classList.contains('ad-interrupting')) return true;
      return Boolean(document.querySelector(
        '.ytp-ad-player-overlay, .ytp-ad-message-container, .ytp-ad-preview-container, .ytp-ad-module:not(:empty)'
      ));
    }

    function removeDirectShield(restoreNative = false) {
      try { state.shield?.remove?.(); } catch {}
      state.shield = null;
      if (!restoreNative || !state.shieldRestore) return;
      const { video, muted, volume } = state.shieldRestore;
      state.shieldRestore = null;
      if (!video || !video.isConnected) return;
      try { video.muted = muted; } catch {}
      try { video.volume = volume; } catch {}
      try { video.play?.().catch?.(() => {}); } catch {}
    }

    function ensureDirectShield() {
      const player = document.querySelector('#movie_player');
      if (!player) return false;
      const nativeVideo = player.querySelector('video');
      if (nativeVideo && (!state.shieldRestore || state.shieldRestore.video !== nativeVideo)) {
        state.shieldRestore = {
          video: nativeVideo,
          muted: Boolean(nativeVideo.muted),
          volume: Number.isFinite(nativeVideo.volume) ? nativeVideo.volume : 1
        };
      }
      if (nativeVideo) {
        try { nativeVideo.muted = true; } catch {}
        try { nativeVideo.pause?.(); } catch {}
      }
      if (state.shield?.isConnected) return true;

      const shield = document.createElement('div');
      shield.id = 'gekko-youtube-shield';
      shield.setAttribute('aria-label', 'GEKKO prépare la vidéo sans publicité');
      shield.style.cssText = [
        'position:absolute','inset:0','z-index:2147482500',
        'display:flex','flex-direction:column','align-items:center','justify-content:center',
        'gap:10px','background:linear-gradient(145deg,#02070b,#06151c)',
        'color:#d9fff7','font:600 12px/1.2 system-ui,sans-serif',
        'letter-spacing:.06em','pointer-events:auto'
      ].join(';');
      const mark = document.createElement('div');
      mark.textContent = 'GEKKO DIRECT';
      mark.style.cssText = 'padding:8px 12px;border:1px solid rgba(122,255,225,.28);border-radius:999px;background:rgba(25,110,105,.12)';
      const status = document.createElement('small');
      status.textContent = 'Préparation de la vidéo sans publicité…';
      status.style.cssText = 'font:500 10px system-ui,sans-serif;letter-spacing:.02em;color:rgba(220,255,250,.55)';
      shield.append(mark, status);
      player.append(shield);
      state.shield = shield;
      return true;
    }

    function requestDirect() {
      if (Date.now() < state.directCooldownUntil) return false;
      const activeVideoId = currentVideoId();
      if (state.directFailedFor && state.directFailedFor === activeVideoId) return false;
      if (state.directRequested || (window.__gekkoDirectPlayerV2?.snapshot?.().active || window.__gekkoDirectPlayerV1?.snapshot?.().active)) return false;
      const videoId = currentVideoId();
      if (!/^[A-Za-z0-9_-]{6,32}$/.test(videoId)) return false;
      state.directRequested = true;
      try {
        location.href = 'gekko-direct://youtube/' + encodeURIComponent(videoId);
        return true;
      } catch {
        state.directRequested = false;
        return false;
      }
    }

    function ensureDirectButton() {
      if (document.getElementById('gekko-direct-button')) return;
      const controls = document.querySelector('.ytp-right-controls');
      if (!controls) return;

      const button = document.createElement('button');
      button.id = 'gekko-direct-button';
      button.type = 'button';
      button.title = 'Lire avec GEKKO Direct';
      button.textContent = 'G·DIRECT';
      button.style.cssText = [
        'height:32px',
        'margin:0 5px',
        'padding:0 9px',
        'border:1px solid rgba(122,255,225,.4)',
        'border-radius:999px',
        'background:rgba(3,20,24,.72)',
        'color:#d9fff7',
        'font:700 10px/30px system-ui,sans-serif',
        'letter-spacing:.06em',
        'cursor:pointer'
      ].join(';');
      button.addEventListener('click', (event) => {
        event.preventDefault();
        event.stopPropagation();
        requestDirect();
      });
      controls.prepend(button);
    }

    function clickSkipButtons() {
      const selectors = [
        '.ytp-ad-skip-button',
        '.ytp-ad-skip-button-modern',
        '.ytp-skip-ad-button',
        'button[class*="ytp-ad-skip"]'
      ];
      for (const selector of selectors) {
        const button = document.querySelector(selector);
        if (button && typeof button.click === 'function') {
          try { button.click(); } catch {}
        }
      }
    }

    function clearAdChrome() {
      const selectors = [
        '.ytp-ad-overlay-container',
        '.ytp-ad-player-overlay',
        '.ytp-ad-message-container',
        '.ytp-ad-image-overlay',
        '#player-ads',
        'ytd-display-ad-renderer',
        'ytd-promoted-sparkles-web-renderer',
        'ytd-promoted-video-renderer',
        'ytd-ad-slot-renderer'
      ];
      for (const selector of selectors) {
        for (const node of document.querySelectorAll(selector)) {
          try { node.remove(); } catch {}
        }
      }
    }

    function nativeVideoForPlayer(player = document.querySelector('#movie_player')) {
      if (!player) return null;
      return player.querySelector('.html5-video-container video.html5-main-video, video.html5-main-video, .html5-video-container video, video');
    }

    function hideAdCorner(restoreNative = false) {
      const saved = state.adCornerRestore;
      state.adCornerRestore = null;
      try { state.adCornerBadge?.remove?.(); } catch {}
      state.adCornerBadge = null;
      if (!saved) return;

      const { video, container, videoStyle, containerStyle, muted, volume, playbackRate } = saved;
      if (container?.isConnected) {
        try {
          if (containerStyle == null) container.removeAttribute('style');
          else container.setAttribute('style', containerStyle);
        } catch {}
      }
      if (video?.isConnected) {
        try {
          if (videoStyle == null) video.removeAttribute('style');
          else video.setAttribute('style', videoStyle);
        } catch {}
        if (restoreNative) {
          try { video.muted = muted; } catch {}
          try { video.volume = volume; } catch {}
          try { video.playbackRate = playbackRate; } catch {}
          try { video.play?.().catch?.(() => {}); } catch {}
        } else {
          try { video.muted = true; } catch {}
          try { video.pause?.(); } catch {}
        }
      }
    }

    function showAdCorner() {
      const player = document.querySelector('#movie_player');
      const video = nativeVideoForPlayer(player);
      const container = video?.closest?.('.html5-video-container') || video?.parentElement;
      if (!player || !video || !container) return false;

      if (!state.adCornerRestore || state.adCornerRestore.video !== video) {
        hideAdCorner(false);
        state.adCornerRestore = {
          video,
          container,
          videoStyle: video.getAttribute('style'),
          containerStyle: container.getAttribute('style'),
          muted: Boolean(video.muted),
          volume: Number.isFinite(video.volume) ? video.volume : 1,
          playbackRate: Number(video.playbackRate || 1)
        };
      }

      for (const [name, value] of [
        ['position', 'absolute'],
        ['inset', 'auto 14px auto auto'],
        ['top', '56px'],
        ['right', '14px'],
        ['width', 'clamp(180px, 26%, 320px)'],
        ['height', 'auto'],
        ['aspect-ratio', '16 / 9'],
        ['z-index', '2147483600'],
        ['overflow', 'hidden'],
        ['border-radius', '14px'],
        ['box-shadow', '0 12px 34px rgba(0,0,0,.52)'],
        ['border', '1px solid rgba(255,255,255,.28)'],
        ['background', '#000'],
        ['pointer-events', 'none']
      ]) {
        try { container.style.setProperty(name, value, 'important'); } catch {}
      }

      for (const [name, value] of [
        ['position', 'absolute'],
        ['inset', '0'],
        ['width', '100%'],
        ['height', '100%'],
        ['object-fit', 'cover'],
        ['transform', 'none'],
        ['pointer-events', 'none']
      ]) {
        try { video.style.setProperty(name, value, 'important'); } catch {}
      }

      try { video.muted = true; } catch {}
      try { video.volume = 0; } catch {}
      try { video.playbackRate = 1; } catch {}
      try { video.play?.().catch?.(() => {}); } catch {}

      if (!state.adCornerBadge?.isConnected) {
        const badge = document.createElement('div');
        badge.id = 'gekko-youtube-ad-corner-badge';
        badge.textContent = 'PUB · MUET';
        badge.style.cssText = [
          'position:absolute','top:8px','left:8px','z-index:2147483646',
          'padding:5px 8px','border-radius:999px',
          'background:rgba(0,0,0,.72)','color:#fff',
          'font:700 10px/1 system-ui,sans-serif','letter-spacing:.06em',
          'pointer-events:none','backdrop-filter:blur(10px)'
        ].join(';');
        try { container.appendChild(badge); } catch {}
        state.adCornerBadge = badge;
      }
      return true;
    }

    function restorePlayback() {
      if (!state.restore) return;
      const { video, muted, playbackRate, volume } = state.restore;
      state.restore = null;
      if (!video || !video.isConnected) return;
      try { video.playbackRate = playbackRate; } catch {}
      try { video.volume = volume; } catch {}
      try { video.muted = muted; } catch {}
    }

    function bypassAdPlayback() {
      const player = document.querySelector('#movie_player');
      const adShowing = adIsShowing();

      if ((window.__gekkoDirectPlayerV2?.snapshot?.().active || window.__gekkoDirectPlayerV1?.snapshot?.().active)) {
        state.adSince = 0;
        if (adShowing) showAdCorner();
        else hideAdCorner(false);
        return;
      }

      if (!adShowing) {
        state.adSince = 0;
        state.directRequested = false;
        restorePlayback();
        return;
      }

      if (!state.adSince) state.adSince = Date.now();

      clickSkipButtons();
      clearAdChrome();

      const video = player.querySelector('video');
      if (!video) return;

      if (!state.restore || state.restore.video !== video) {
        state.restore = {
          video,
          muted: Boolean(video.muted),
          playbackRate: Number(video.playbackRate || 1),
          volume: Number.isFinite(video.volume) ? video.volume : 1
        };
      }

      try { video.muted = true; } catch {}
      try { video.playbackRate = 16; } catch {}

      try {
        const duration = Number(video.duration);
        const current = Number(video.currentTime);
        if (Number.isFinite(duration) && duration > 0 && duration < 600 && duration - current > 0.35) {
          video.currentTime = Math.max(current, duration - 0.15);
        }
      } catch {}

      try { video.play?.().catch?.(() => {}); } catch {}

      if (!state.directRequested) {
        requestDirect();
      }
    }

    function refresh() {
      scrubKnownGlobals();
      clearAdChrome();
      ensureDirectButton();

      const videoId = currentVideoId();
      if (videoId !== state.lastVideoId) {
        hideAdCorner(true);
        state.lastVideoId = videoId;
        state.directRequested = false;
        state.directFailedFor = '';
        state.directFallbackAfter = 0;
        state.directCooldownUntil = 0;
        removeDirectShield(true);
      }

      const directActive = Boolean(
        window.__gekkoDirectPlayerV2?.snapshot?.().active ||
        window.__gekkoDirectPlayerV1?.snapshot?.().active
      );

      if (!videoId) {
        removeDirectShield(true);
        bypassAdPlayback();
        return;
      }

      if (directActive) {
        removeDirectShield(false);
        if (adIsShowing()) showAdCorner();
        else hideAdCorner(false);
        bypassAdPlayback();
        return;
      }

      if (state.directFailedFor === videoId) {
        if (adIsShowing()) {
          ensureDirectShield();
          bypassAdPlayback();
          return;
        }
        if (Date.now() >= state.directFallbackAfter) {
          removeDirectShield(true);
        } else {
          ensureDirectShield();
        }
        bypassAdPlayback();
        return;
      }

      // A watch/shorts/live page never reveals native playback before Direct has
      // had the first chance to resolve the content stream.
      ensureDirectShield();
      requestDirect();
      bypassAdPlayback();
    }

    function scheduleRefresh() {
      if (state.scheduled) return;
      state.scheduled = true;
      queueMicrotask(() => {
        state.scheduled = false;
        refresh();
      });
    }

    patchResponseReaders();
    patchFetch();
    patchJsonParser();
    scrubKnownGlobals();

    state.observer = new MutationObserver(scheduleRefresh);
    try {
      state.observer.observe(document.documentElement || document, {
        childList: true,
        subtree: true,
        attributes: true,
        attributeFilter: ['class']
      });
    } catch {}

    state.interval = setInterval(refresh, 250);
    window.addEventListener('pagehide', () => {
      try { clearInterval(state.interval); } catch {}
      try { state.observer?.disconnect(); } catch {}
      hideAdCorner(true);
      restorePlayback();
    }, { once: true });

    window[KEY] = {
      refresh,
      directResult(result = {}) {
        state.directRequested = Boolean(result.ok);
        if (result.ok) {
          state.directFailedFor = '';
          state.directFallbackAfter = 0;
          removeDirectShield(false);
        }
        if (!result.ok) {
          hideAdCorner(true);
          state.directFailedFor = currentVideoId();
          state.directFallbackAfter = Date.now() + 1400;
          state.directCooldownUntil = Date.now() + 15000;
          state.adSince = 0;
          const button = document.getElementById('gekko-direct-button');
          if (button) {
            const original = button.textContent;
            button.textContent = 'DIRECT ✕';
            button.title = String(result.error || 'Flux direct indisponible');
            setTimeout(() => {
              if (!button.isConnected) return;
              button.textContent = original || 'G·DIRECT';
              button.title = 'Lire avec GEKKO Direct';
              state.directRequested = false;
            }, 2600);
          } else {
            state.directRequested = false;
          }
        }
      },
      resetDirect(cooldownMs = 0) {
        hideAdCorner(true);
        state.directRequested = false;
        state.adSince = 0;
        state.directFailedFor = '';
        state.directFallbackAfter = 0;
        state.directCooldownUntil = Date.now() + Math.max(0, Number(cooldownMs || 0));
      }
    };
    refresh();
    return true;
  })()`;
}

async function installYouTubeGuard(webContents) {
  if (!webContents || webContents.isDestroyed?.()) return false;

  let current = '';
  try { current = webContents.getURL?.() || ''; } catch {}
  if (!isYouTubeUrl(current)) return false;

  try {
    await webContents.executeJavaScript(youtubeGuardSource(), true);
    return true;
  } catch {
    return false;
  }
}

module.exports = {
  YOUTUBE_HOST_SUFFIXES,
  isYouTubeUrl,
  youtubeGuardSource,
  installYouTubeGuard
};
