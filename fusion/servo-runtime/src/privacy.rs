use url::Url;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockReason {
    Advertising,
    Analytics,
    SocialTracker,
    Telemetry,
    Fingerprinting,
    InsecureSubresource,
}

impl BlockReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::Advertising => "publicité",
            Self::Analytics => "analytics",
            Self::SocialTracker => "traqueur social",
            Self::Telemetry => "télémétrie",
            Self::Fingerprinting => "fingerprinting",
            Self::InsecureSubresource => "contenu HTTP",
        }
    }
}

#[derive(Default, Clone, Copy, Debug)]
pub struct PrivacyStats {
    pub advertising: u64,
    pub analytics: u64,
    pub social: u64,
    pub telemetry: u64,
    pub fingerprinting: u64,
    pub insecure: u64,
}

impl PrivacyStats {
    pub fn record(&mut self, reason: BlockReason) {
        match reason {
            BlockReason::Advertising => self.advertising += 1,
            BlockReason::Analytics => self.analytics += 1,
            BlockReason::SocialTracker => self.social += 1,
            BlockReason::Telemetry => self.telemetry += 1,
            BlockReason::Fingerprinting => self.fingerprinting += 1,
            BlockReason::InsecureSubresource => self.insecure += 1,
        }
    }
}

const AD_HOSTS: &[&str] = &[
    "doubleclick.net",
    "googlesyndication.com",
    "googleadservices.com",
    "adservice.google.com",
    "adnxs.com",
    "adsrvr.org",
    "criteo.com",
    "criteo.net",
    "taboola.com",
    "outbrain.com",
    "amazon-adsystem.com",
    "rubiconproject.com",
    "openx.net",
    "pubmatic.com",
];

const ANALYTICS_HOSTS: &[&str] = &[
    "google-analytics.com",
    "googletagmanager.com",
    "hotjar.com",
    "clarity.ms",
    "segment.com",
    "segment.io",
    "scorecardresearch.com",
    "quantserve.com",
    "mixpanel.com",
    "amplitude.com",
    "plausible.io",
    "heap.io",
];

const SOCIAL_TRACKER_HOSTS: &[&str] = &[
    "connect.facebook.net",
    "facebook.net",
    "analytics.twitter.com",
    "ads-twitter.com",
    "snap.licdn.com",
    "px.ads.linkedin.com",
    "analytics.tiktok.com",
    "business-api.tiktok.com",
    "tr.snapchat.com",
    "ct.pinterest.com",
];

const TELEMETRY_HOSTS: &[&str] = &[
    "sentry.io",
    "browser-intake-datadoghq.com",
    "datadoghq.com",
    "newrelic.com",
    "nr-data.net",
    "bugsnag.com",
    "logrocket.io",
    "fullstory.com",
];

const FINGERPRINT_HOSTS: &[&str] = &[
    "fingerprint.com",
    "fingerprintjs.com",
    "fpjs.io",
    "openfpcdn.io",
];

const MULTI_LABEL_SUFFIXES: &[&str] = &[
    "co.uk", "com.au", "co.jp", "com.br", "co.in", "com.cn", "com.sg", "co.nz",
    "com.mx", "com.tr", "com.ar", "com.tw", "co.kr", "co.za",
];

fn host_matches(host: &str, domain: &str) -> bool {
    host == domain
        || host
            .strip_suffix(domain)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

fn host_in(host: &str, list: &[&str]) -> bool {
    list.iter().any(|domain| host_matches(host, domain))
}

fn site_key(host: &str) -> String {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host.parse::<std::net::IpAddr>().is_ok() || !host.contains('.') {
        return host;
    }

    for suffix in MULTI_LABEL_SUFFIXES {
        if host == *suffix {
            return host;
        }
        if let Some(prefix) = host.strip_suffix(suffix)
            && let Some(prefix) = prefix.strip_suffix('.')
            && let Some(label) = prefix.rsplit('.').next()
        {
            return format!("{label}.{suffix}");
        }
    }

    let mut labels = host.rsplit('.');
    let tld = labels.next().unwrap_or_default();
    let second = labels.next().unwrap_or_default();
    if second.is_empty() {
        host
    } else {
        format!("{second}.{tld}")
    }
}

pub fn is_third_party(url: &Url, referrer: Option<&Url>) -> bool {
    let Some(target_host) = url.host_str() else {
        return false;
    };
    let Some(referrer_host) = referrer.and_then(Url::host_str) else {
        return false;
    };
    site_key(target_host) != site_key(referrer_host)
}

fn generic_tracking_host(host: &str) -> Option<BlockReason> {
    let labels = host.split('.');
    for label in labels {
        if label.contains("fingerprint") || label == "fpjs" {
            return Some(BlockReason::Fingerprinting);
        }
        if label.contains("analytics") || label.contains("tracking") || label.contains("tracker") {
            return Some(BlockReason::Analytics);
        }
        if label.contains("telemetry") || label.contains("beacon") || label.contains("metrics") {
            return Some(BlockReason::Telemetry);
        }
        if label == "ads" || label.starts_with("adserver") || label.starts_with("adservice") {
            return Some(BlockReason::Advertising);
        }
    }
    None
}

pub fn classify_resource(
    url: &Url,
    referrer: Option<&Url>,
    is_main_frame: bool,
) -> Option<BlockReason> {
    if is_main_frame || !matches!(url.scheme(), "http" | "https") {
        return None;
    }

    let host = url.host_str()?.to_ascii_lowercase();

    if host_in(&host, FINGERPRINT_HOSTS) {
        return Some(BlockReason::Fingerprinting);
    }
    if host_in(&host, AD_HOSTS) {
        return Some(BlockReason::Advertising);
    }
    if host_in(&host, SOCIAL_TRACKER_HOSTS) {
        return Some(BlockReason::SocialTracker);
    }
    if host_in(&host, ANALYTICS_HOSTS) {
        return Some(BlockReason::Analytics);
    }
    if host_in(&host, TELEMETRY_HOSTS) {
        return Some(BlockReason::Telemetry);
    }

    let third_party = is_third_party(url, referrer);

    if third_party
        && referrer.is_some_and(|source| source.scheme() == "https")
        && url.scheme() == "http"
    {
        return Some(BlockReason::InsecureSubresource);
    }

    if third_party {
        return generic_tracking_host(&host);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_same_site_across_subdomains() {
        let a = Url::parse("https://www.example.com/page").unwrap();
        let b = Url::parse("https://cdn.example.com/app.js").unwrap();
        assert!(!is_third_party(&b, Some(&a)));
    }

    #[test]
    fn blocks_known_tracker_resource() {
        let page = Url::parse("https://example.com").unwrap();
        let tracker = Url::parse("https://www.google-analytics.com/collect").unwrap();
        assert_eq!(
            classify_resource(&tracker, Some(&page), false),
            Some(BlockReason::Analytics)
        );
    }

    #[test]
    fn never_blocks_main_frame_by_classifier() {
        let page = Url::parse("https://doubleclick.net").unwrap();
        assert_eq!(classify_resource(&page, None, true), None);
    }

    #[test]
    fn blocks_third_party_plain_http_under_https() {
        let page = Url::parse("https://example.com").unwrap();
        let resource = Url::parse("http://cdn.other.net/file.js").unwrap();
        assert_eq!(
            classify_resource(&resource, Some(&page), false),
            Some(BlockReason::InsecureSubresource)
        );
    }
}
