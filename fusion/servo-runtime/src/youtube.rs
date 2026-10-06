use url::Url;

pub const YOUTUBE_CLEAN_PLAYBACK_SCRIPT: &str = r#"
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
"#;

fn host_matches(host: &str, domain: &str) -> bool {
    host == domain || host.strip_suffix(domain).is_some_and(|prefix| prefix.ends_with('.'))
}

fn youtube_context(referrer: Option<&Url>) -> bool {
    referrer
        .and_then(Url::host_str)
        .map(|host| {
            let host = host.to_ascii_lowercase();
            host_matches(&host, "youtube.com") || host_matches(&host, "youtube-nocookie.com")
        })
        .unwrap_or(false)
}

fn has_ad_query_marker(url: &Url) -> bool {
    url.query_pairs().any(|(key, value)| {
        let key = key.to_ascii_lowercase();
        let value = value.to_ascii_lowercase();
        matches!(
            key.as_ref(),
            "adformat" | "ad_type" | "adunit" | "ad_preroll" | "adurl" | "ad_tag"
        ) || (key == "oad" && value != "0" && value != "false")
    })
}

pub fn is_youtube_ad_resource(url: &Url, referrer: Option<&Url>) -> bool {
    if !youtube_context(referrer) {
        return false;
    }

    let Some(host) = url.host_str().map(str::to_ascii_lowercase) else {
        return false;
    };
    let path = url.path().to_ascii_lowercase();

    if host_matches(&host, "doubleclick.net")
        || host_matches(&host, "googlesyndication.com")
        || host_matches(&host, "googleadservices.com")
    {
        return true;
    }

    if host_matches(&host, "youtube.com") || host_matches(&host, "youtube-nocookie.com") {
        return [
            "/pagead/",
            "/api/stats/ads",
            "/get_midroll_info",
            "/ptracking",
            "/pagead/interaction",
            "/pagead/conversion",
        ]
        .iter()
        .any(|needle| path.contains(needle));
    }

    host_matches(&host, "googlevideo.com")
        && path.contains("/videoplayback")
        && has_ad_query_marker(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_youtube_page_ad_request() {
        let page = Url::parse("https://www.youtube.com/watch?v=abc").unwrap();
        let ad = Url::parse("https://www.youtube.com/pagead/interaction/?ai=test").unwrap();
        assert!(is_youtube_ad_resource(&ad, Some(&page)));
    }

    #[test]
    fn blocks_marked_googlevideo_ad_stream() {
        let page = Url::parse("https://www.youtube.com/watch?v=abc").unwrap();
        let ad = Url::parse("https://rr1---sn.example.googlevideo.com/videoplayback?oad=1&id=abc").unwrap();
        assert!(is_youtube_ad_resource(&ad, Some(&page)));
    }

    #[test]
    fn keeps_normal_googlevideo_stream() {
        let page = Url::parse("https://www.youtube.com/watch?v=abc").unwrap();
        let media = Url::parse("https://rr1---sn.example.googlevideo.com/videoplayback?id=abc&itag=399").unwrap();
        assert!(!is_youtube_ad_resource(&media, Some(&page)));
    }

    #[test]
    fn youtube_heuristics_do_not_leak_to_other_sites() {
        let page = Url::parse("https://example.com/").unwrap();
        let media = Url::parse("https://rr1---sn.example.googlevideo.com/videoplayback?oad=1&id=abc").unwrap();
        assert!(!is_youtube_ad_resource(&media, Some(&page)));
    }

    #[test]
    fn clean_playback_script_has_three_layers() {
        assert!(YOUTUBE_CLEAN_PLAYBACK_SCRIPT.contains("/youtubei/v1/player"));
        assert!(YOUTUBE_CLEAN_PLAYBACK_SCRIPT.contains("adPlacements"));
        assert!(YOUTUBE_CLEAN_PLAYBACK_SCRIPT.contains("MutationObserver"));
        assert!(YOUTUBE_CLEAN_PLAYBACK_SCRIPT.contains("ad-showing"));
    }
}
