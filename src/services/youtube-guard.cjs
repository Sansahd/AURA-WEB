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
      patchedResponse: false
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

      if (!adShowing) {
        restorePlayback();
        return;
      }

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
    }

    function refresh() {
      scrubKnownGlobals();
      clearAdChrome();
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

    window[KEY] = { refresh };
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
