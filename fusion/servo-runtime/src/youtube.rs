use url::Url;

pub const YOUTUBE_CLEAN_PLAYBACK_SCRIPT: &str = include_str!("youtube-clean.js");

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
