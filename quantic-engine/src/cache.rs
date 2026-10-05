use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

use url::Url;

use crate::{network::FetchResponse, privacy::ResourceKind};

const DEFAULT_MAX_ENTRIES: usize = 128;
const DEFAULT_MAX_BYTES: usize = 32 * 1024 * 1024;
const DEFAULT_TTL_SECONDS: u64 = 60;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    partition: String,
    target: String,
    kind: ResourceKind,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    response: FetchResponse,
    expires_at: Instant,
    bytes: usize,
}

#[derive(Debug)]
pub struct ResourceCache {
    entries: HashMap<CacheKey, CacheEntry>,
    order: VecDeque<CacheKey>,
    total_bytes: usize,
    max_entries: usize,
    max_bytes: usize,
    ttl: Duration,
}

impl Default for ResourceCache {
    fn default() -> Self {
        Self::new(
            DEFAULT_MAX_ENTRIES,
            DEFAULT_MAX_BYTES,
            Duration::from_secs(DEFAULT_TTL_SECONDS),
        )
    }
}

impl ResourceCache {
    pub fn new(max_entries: usize, max_bytes: usize, ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            total_bytes: 0,
            max_entries: max_entries.max(1),
            max_bytes: max_bytes.max(1),
            ttl,
        }
    }

    pub fn get(
        &mut self,
        top_level: &Url,
        target: &Url,
        kind: ResourceKind,
    ) -> Option<FetchResponse> {
        self.purge_expired();
        let key = cache_key(top_level, target, kind);
        let response = self.entries.get(&key)?.response.clone();
        self.touch(&key);
        Some(response)
    }

    pub fn put(
        &mut self,
        top_level: &Url,
        target: &Url,
        kind: ResourceKind,
        response: FetchResponse,
    ) -> bool {
        if !is_cacheable_kind(kind)
            || !response.set_cookie_headers.is_empty()
            || response.body.len() > self.max_bytes
        {
            return false;
        }

        let Some(lifetime) = cache_lifetime(&response, self.ttl) else {
            return false;
        };

        self.purge_expired();
        let key = cache_key(top_level, target, kind);
        self.remove(&key);

        let bytes = response.body.len();
        while (!self.entries.is_empty())
            && (self.entries.len() >= self.max_entries
                || self.total_bytes.saturating_add(bytes) > self.max_bytes)
        {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            self.remove_entry_only(&oldest);
        }

        if self.total_bytes.saturating_add(bytes) > self.max_bytes {
            return false;
        }

        self.total_bytes = self.total_bytes.saturating_add(bytes);
        self.order.push_back(key.clone());
        self.entries.insert(
            key,
            CacheEntry {
                response,
                expires_at: Instant::now() + lifetime,
                bytes,
            },
        );
        true
    }

    pub fn len(&mut self) -> usize {
        self.purge_expired();
        self.entries.len()
    }

    pub fn is_empty(&mut self) -> bool {
        self.len() == 0
    }

    pub fn total_bytes(&mut self) -> usize {
        self.purge_expired();
        self.total_bytes
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.total_bytes = 0;
    }

    fn purge_expired(&mut self) {
        let now = Instant::now();
        let expired = self
            .entries
            .iter()
            .filter_map(|(key, entry)| {
                (now >= entry.expires_at).then_some(key.clone())
            })
            .collect::<Vec<_>>();
        for key in expired {
            self.remove(&key);
        }
    }

    fn touch(&mut self, key: &CacheKey) {
        self.order.retain(|candidate| candidate != key);
        self.order.push_back(key.clone());
    }

    fn remove(&mut self, key: &CacheKey) {
        self.order.retain(|candidate| candidate != key);
        self.remove_entry_only(key);
    }

    fn remove_entry_only(&mut self, key: &CacheKey) {
        if let Some(entry) = self.entries.remove(key) {
            self.total_bytes = self.total_bytes.saturating_sub(entry.bytes);
        }
    }
}

fn cache_lifetime(response: &FetchResponse, default_ttl: Duration) -> Option<Duration> {
    let Some(value) = response.cache_control.as_deref() else {
        return (!default_ttl.is_zero()).then_some(default_ttl);
    };

    let mut max_age = None;
    for directive in value.split(',').map(str::trim) {
        let lower = directive.to_ascii_lowercase();
        if matches!(lower.as_str(), "no-store" | "no-cache") {
            return None;
        }
        if let Some(raw) = lower.strip_prefix("max-age=") {
            let raw = raw.trim().trim_matches('"');
            if let Ok(seconds) = raw.parse::<u64>() {
                max_age = Some(Duration::from_secs(seconds));
            }
        }
    }

    match max_age {
        Some(duration) if duration.is_zero() => None,
        Some(duration) => Some(duration),
        None if default_ttl.is_zero() => None,
        None => Some(default_ttl),
    }
}

pub fn is_cacheable_kind(kind: ResourceKind) -> bool {
    matches!(
        kind,
        ResourceKind::Style | ResourceKind::Script | ResourceKind::Image | ResourceKind::Font
    )
}

fn cache_key(top_level: &Url, target: &Url, kind: ResourceKind) -> CacheKey {
    let mut normalized = target.clone();
    normalized.set_fragment(None);
    CacheKey {
        partition: top_level.origin().ascii_serialization(),
        target: normalized.to_string(),
        kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(url: &str, body: &[u8]) -> FetchResponse {
        FetchResponse {
            final_url: Url::parse(url).unwrap(),
            status: 200,
            content_type: Some("text/css".to_string()),
            cache_control: None,
            set_cookie_headers: Vec::new(),
            body: body.to_vec(),
        }
    }

    #[test]
    fn cache_is_partitioned_by_top_level_origin() {
        let mut cache = ResourceCache::default();
        let a = Url::parse("https://a.example/page").unwrap();
        let b = Url::parse("https://b.example/page").unwrap();
        let target = Url::parse("https://cdn.example/app.css").unwrap();

        assert!(cache.put(
            &a,
            &target,
            ResourceKind::Style,
            response(target.as_str(), b"a{}"),
        ));
        assert!(cache.get(&a, &target, ResourceKind::Style).is_some());
        assert!(cache.get(&b, &target, ResourceKind::Style).is_none());
    }

    #[test]
    fn responses_with_set_cookie_are_not_cached() {
        let mut cache = ResourceCache::default();
        let top = Url::parse("https://app.example/").unwrap();
        let target = Url::parse("https://app.example/app.js").unwrap();
        let mut value = response(target.as_str(), b"console.log(1)");
        value.set_cookie_headers.push("session=secret".to_string());

        assert!(!cache.put(&top, &target, ResourceKind::Script, value));
        assert!(cache.is_empty());
    }

    #[test]
    fn cache_control_no_store_and_no_cache_are_respected() {
        let mut cache = ResourceCache::default();
        let top = Url::parse("https://app.example/").unwrap();
        let target = Url::parse("https://app.example/app.css").unwrap();

        for directive in ["no-store", "no-cache", "max-age=0"] {
            let mut value = response(target.as_str(), b"a{}");
            value.cache_control = Some(directive.to_string());
            assert!(!cache.put(&top, &target, ResourceKind::Style, value));
        }
        assert!(cache.is_empty());
    }

    #[test]
    fn cache_control_max_age_overrides_default_ttl() {
        let mut cache = ResourceCache::new(8, 1024, Duration::ZERO);
        let top = Url::parse("https://app.example/").unwrap();
        let target = Url::parse("https://app.example/app.css").unwrap();
        let mut value = response(target.as_str(), b"a{}");
        value.cache_control = Some("public, max-age=60".to_string());

        assert!(cache.put(&top, &target, ResourceKind::Style, value));
        assert!(cache.get(&top, &target, ResourceKind::Style).is_some());
    }

    #[test]
    fn cache_evicts_oldest_entries_to_respect_bounds() {
        let mut cache = ResourceCache::new(1, 32, Duration::from_secs(60));
        let top = Url::parse("https://app.example/").unwrap();
        let one = Url::parse("https://app.example/one.css").unwrap();
        let two = Url::parse("https://app.example/two.css").unwrap();

        assert!(cache.put(
            &top,
            &one,
            ResourceKind::Style,
            response(one.as_str(), b"one"),
        ));
        assert!(cache.put(
            &top,
            &two,
            ResourceKind::Style,
            response(two.as_str(), b"two"),
        ));
        assert!(cache.get(&top, &one, ResourceKind::Style).is_none());
        assert!(cache.get(&top, &two, ResourceKind::Style).is_some());
    }
}
