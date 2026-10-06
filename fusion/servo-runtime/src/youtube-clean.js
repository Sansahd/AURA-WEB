
(() => {
  const host = location.hostname.toLowerCase();
  const onYouTube =
    host === "youtube.com" ||
    host.endsWith(".youtube.com") ||
    host === "youtube-nocookie.com" ||
    host.endsWith(".youtube-nocookie.com");

  if (!onYouTube || window.__gekkoYouTubeCleanPlayback) return;
  window.__gekkoYouTubeCleanPlayback = true;

  const adKeys = new Set([
    "adPlacements",
    "playerAds",
    "adSlots",
    "adBreakHeartbeatParams",
    "adSafetyReason",
    "adSurvey",
    "adSignalsInfo",
    "instreamVideoAdRenderer",
    "playerLegacyDesktopWatchAdsRenderer"
  ]);

  const isAdKey = (key) => {
    if (adKeys.has(key)) return true;
    const lower = String(key).toLowerCase();
    return lower === "adplacements" ||
      lower === "playerads" ||
      lower === "adslots" ||
      lower.includes("instreamvideoadrenderer") ||
      lower.includes("promotedsparkles") ||
      lower.includes("companionad");
  };

  const sanitize = (value, depth = 0) => {
    if (!value || depth > 24) return value;
    if (Array.isArray(value)) {
      for (let i = value.length - 1; i >= 0; i--) {
        const item = value[i];
        if (item && typeof item === "object") sanitize(item, depth + 1);
      }
      return value;
    }
    if (typeof value !== "object") return value;

    for (const key of Object.keys(value)) {
      if (isAdKey(key)) {
        try { delete value[key]; } catch (_) { value[key] = Array.isArray(value[key]) ? [] : null; }
        continue;
      }
      sanitize(value[key], depth + 1);
    }
    return value;
  };

  let initialPlayerResponse;
  try { initialPlayerResponse = sanitize(window.ytInitialPlayerResponse); } catch (_) {}
  try {
    Object.defineProperty(window, "ytInitialPlayerResponse", {
      configurable: true,
      enumerable: true,
      get: () => initialPlayerResponse,
      set: (value) => { initialPlayerResponse = sanitize(value); }
    });
  } catch (_) {}

  const cleanPlayerResponseText = (text) => {
    try {
      const parsed = JSON.parse(text);
      return JSON.stringify(sanitize(parsed));
    } catch (_) {
      return text;
    }
  };

  const originalFetch = window.fetch && window.fetch.bind(window);
  if (originalFetch) {
    window.fetch = async (...args) => {
      const response = await originalFetch(...args);
      try {
        const rawUrl = typeof args[0] === "string" ? args[0] : args[0] && args[0].url;
        const url = new URL(rawUrl || "", location.href);
        if (url.hostname.endsWith("youtube.com") && url.pathname.includes("/youtubei/v1/player")) {
          const text = await response.clone().text();
          const cleaned = cleanPlayerResponseText(text);
          if (cleaned !== text) {
            const headers = new Headers(response.headers);
            headers.delete("content-length");
            return new Response(cleaned, {
              status: response.status,
              statusText: response.statusText,
              headers
            });
          }
        }
      } catch (_) {}
      return response;
    };
  }

  const cleanKnownGlobals = () => {
    try {
      if (window.ytInitialPlayerResponse) sanitize(window.ytInitialPlayerResponse);
    } catch (_) {}
    try {
      const args = window.ytplayer && window.ytplayer.config && window.ytplayer.config.args;
      if (args && typeof args.player_response === "string") {
        args.player_response = cleanPlayerResponseText(args.player_response);
      }
    } catch (_) {}
  };

  const skipSelectors = [
    ".ytp-ad-skip-button",
    ".ytp-ad-skip-button-modern",
    ".ytp-skip-ad-button",
    "button[class*='skip'][class*='ad']"
  ];

  const adUiSelectors = [
    "#player-ads",
    ".ytp-ad-module",
    ".ytp-ad-overlay-container",
    ".ytp-ad-player-overlay",
    "ytd-display-ad-renderer",
    "ytd-promoted-sparkles-web-renderer",
    "ytd-action-companion-ad-renderer",
    "ytd-in-feed-ad-layout-renderer",
    "ytd-ad-slot-renderer",
    "ytd-banner-promo-renderer",
    "ytd-statement-banner-renderer",
    "ytd-masthead-ad-v3-renderer"
  ];

  const killActiveAd = () => {
    cleanKnownGlobals();

    for (const selector of skipSelectors) {
      try {
        const button = document.querySelector(selector);
        if (button && typeof button.click === "function") button.click();
      } catch (_) {}
    }

    const player = document.querySelector(".html5-video-player");
    const adShowing = !!(
      player &&
      (player.classList.contains("ad-showing") || player.classList.contains("ad-interrupting"))
    );

    if (adShowing) {
      const video = document.querySelector("video.html5-main-video, video");
      if (video) {
        try {
          video.muted = true;
          const duration = Number(video.duration);
          if (Number.isFinite(duration) && duration > 0.2) {
            video.currentTime = Math.max(0, duration - 0.05);
          }
          const play = video.play && video.play();
          if (play && typeof play.catch === "function") play.catch(() => {});
        } catch (_) {}
      }
    }

    for (const selector of adUiSelectors) {
      try {
        document.querySelectorAll(selector).forEach((node) => {
          node.style.setProperty("display", "none", "important");
        });
      } catch (_) {}
    }
  };

  const installStyle = () => {
    if (!document.documentElement || document.getElementById("gekko-youtube-clean-style")) return;
    const style = document.createElement("style");
    style.id = "gekko-youtube-clean-style";
    style.textContent = `
      #player-ads,
      .ytp-ad-module,
      .ytp-ad-overlay-container,
      .ytp-ad-player-overlay,
      ytd-display-ad-renderer,
      ytd-promoted-sparkles-web-renderer,
      ytd-action-companion-ad-renderer,
      ytd-in-feed-ad-layout-renderer,
      ytd-ad-slot-renderer,
      ytd-banner-promo-renderer,
      ytd-statement-banner-renderer,
      ytd-masthead-ad-v3-renderer {
        display: none !important;
        visibility: hidden !important;
        pointer-events: none !important;
      }
    `;
    document.documentElement.appendChild(style);
  };

  installStyle();
  cleanKnownGlobals();
  killActiveAd();

  try {
    new MutationObserver(() => {
      installStyle();
      killActiveAd();
    }).observe(document.documentElement, {
      subtree: true,
      childList: true,
      attributes: true,
      attributeFilter: ["class"]
    });
  } catch (_) {}

  setInterval(() => {
    installStyle();
    killActiveAd();
  }, 350);
})();
