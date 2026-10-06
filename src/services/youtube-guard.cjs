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
      adSince: 0,
      directRequested: false,
      directCooldownUntil: 0
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

    function currentVideoId() {
      try {
        const url = new URL(location.href);
        if (url.pathname === '/watch') return url.searchParams.get('v') || '';
        const parts = url.pathname.split('/').filter(Boolean);
        if (['shorts', 'embed', 'live'].includes(parts[0])) return parts[1] || '';
      } catch {}
      return '';
    }

    function requestDirect() {
      if (Date.now() < state.directCooldownUntil) return false;
      if (state.directRequested || window.__gekkoDirectPlayerV1?.snapshot?.().active) return false;
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
      const adShowing = Boolean(player && player.classList.contains('ad-showing'));

      if (window.__gekkoDirectPlayerV1?.snapshot?.().active) {
        state.adSince = 0;
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

      if (!state.directRequested && Date.now() - state.adSince >= 1800) {
        requestDirect();
      }
    }

    function refresh() {
      scrubKnownGlobals();
      clearAdChrome();
      ensureDirectButton();
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

    state.interval = setInterval(refresh, 500);
    window.addEventListener('pagehide', () => {
      try { clearInterval(state.interval); } catch {}
      try { state.observer?.disconnect(); } catch {}
      restorePlayback();
    }, { once: true });

    window[KEY] = {
      refresh,
      directResult(result = {}) {
        state.directRequested = Boolean(result.ok);
        if (!result.ok) {
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
        state.directRequested = false;
        state.adSince = 0;
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
