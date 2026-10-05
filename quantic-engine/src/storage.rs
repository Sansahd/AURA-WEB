use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum StorageMutation {
    Set { key: String, value: String },
    Remove { key: String },
    Clear,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CookieMutation {
    pub value: String,
}

#[derive(Debug, Clone, Default)]
pub struct PartitionedStorage {
    partitions: HashMap<String, BTreeMap<String, String>>,
}

impl PartitionedStorage {
    pub fn snapshot(&self, origin: &str) -> BTreeMap<String, String> {
        self.partitions.get(origin).cloned().unwrap_or_default()
    }

    pub fn apply(&mut self, origin: &str, mutations: &[StorageMutation]) {
        let partition = self.partitions.entry(origin.to_string()).or_default();
        for mutation in mutations {
            match mutation {
                StorageMutation::Set { key, value } => {
                    partition.insert(key.clone(), value.clone());
                }
                StorageMutation::Remove { key } => {
                    partition.remove(key);
                }
                StorageMutation::Clear => partition.clear(),
            }
        }
    }

    pub fn clear_origin(&mut self, origin: &str) {
        self.partitions.remove(origin);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Cookie {
    name: String,
    value: String,
    domain: String,
    path: String,
    secure: bool,
    http_only: bool,
    host_only: bool,
}

#[derive(Debug, Clone, Default)]
pub struct PartitionedCookieJar {
    partitions: HashMap<String, Vec<Cookie>>,
}

impl PartitionedCookieJar {
    pub fn store_response_cookie(
        &mut self,
        top_level: &Url,
        response_url: &Url,
        header: &str,
    ) -> bool {
        self.store_cookie(top_level, response_url, header, false)
    }

    pub fn store_document_cookie(
        &mut self,
        top_level: &Url,
        page_url: &Url,
        assignment: &str,
    ) -> bool {
        self.store_cookie(top_level, page_url, assignment, true)
    }

    pub fn request_header(&self, top_level: &Url, target: &Url) -> Option<String> {
        if !is_first_party(top_level, target) {
            return None;
        }

        let partition = self.partitions.get(&origin_key(Some(top_level)))?;
        let host = target.host_str()?.to_ascii_lowercase();
        let path = normalized_request_path(target);
        let secure_request = matches!(target.scheme(), "https" | "wss");

        let pairs = partition
            .iter()
            .filter(|cookie| {
                domain_matches(&host, cookie)
                    && path_matches(&path, &cookie.path)
                    && (!cookie.secure || secure_request)
            })
            .map(|cookie| format!("{}={}", cookie.name, cookie.value))
            .collect::<Vec<_>>();

        (!pairs.is_empty()).then(|| pairs.join("; "))
    }

    pub fn document_cookie(&self, top_level: &Url, page_url: &Url) -> String {
        if !is_first_party(top_level, page_url) {
            return String::new();
        }

        let Some(partition) = self.partitions.get(&origin_key(Some(top_level))) else {
            return String::new();
        };
        let Some(host) = page_url.host_str().map(str::to_ascii_lowercase) else {
            return String::new();
        };
        let path = normalized_request_path(page_url);
        let secure_request = matches!(page_url.scheme(), "https" | "wss");

        partition
            .iter()
            .filter(|cookie| {
                !cookie.http_only
                    && domain_matches(&host, cookie)
                    && path_matches(&path, &cookie.path)
                    && (!cookie.secure || secure_request)
            })
            .map(|cookie| format!("{}={}", cookie.name, cookie.value))
            .collect::<Vec<_>>()
            .join("; ")
    }

    pub fn clear_partition(&mut self, top_level: &Url) {
        self.partitions.remove(&origin_key(Some(top_level)));
    }

    fn store_cookie(
        &mut self,
        top_level: &Url,
        source_url: &Url,
        header: &str,
        from_document: bool,
    ) -> bool {
        if !is_first_party(top_level, source_url) {
            return false;
        }

        let Some(source_host) = source_url.host_str().map(str::to_ascii_lowercase) else {
            return false;
        };
        let mut parts = header.split(';');
        let Some(pair) = parts.next().map(str::trim) else {
            return false;
        };
        let Some((name, value)) = pair.split_once('=') else {
            return false;
        };
        let name = name.trim();
        if name.is_empty()
            || name.bytes().any(|byte| {
                byte <= 0x20
                    || matches!(
                        byte,
                        b'(' | b')'
                            | b'<'
                            | b'>'
                            | b'@'
                            | b','
                            | b';'
                            | b':'
                            | b'\\'
                            | b'"'
                            | b'/'
                            | b'['
                            | b']'
                            | b'?'
                            | b'='
                            | b'{'
                            | b'}'
                    )
            })
        {
            return false;
        }

        let mut cookie = Cookie {
            name: name.to_string(),
            value: value.trim().to_string(),
            domain: source_host.clone(),
            path: default_cookie_path(source_url),
            secure: false,
            http_only: false,
            host_only: true,
        };
        let mut delete = false;

        for attribute in parts {
            let attribute = attribute.trim();
            if attribute.is_empty() {
                continue;
            }
            let (raw_name, raw_value) = attribute
                .split_once('=')
                .map(|(name, value)| (name.trim(), Some(value.trim())))
                .unwrap_or((attribute, None));
            match raw_name.to_ascii_lowercase().as_str() {
                "domain" => {
                    let Some(domain) = raw_value else {
                        continue;
                    };
                    let domain = domain.trim_start_matches('.').to_ascii_lowercase();
                    if domain.is_empty()
                        || !(source_host == domain || source_host.ends_with(&format!(".{domain}")))
                    {
                        return false;
                    }
                    cookie.domain = domain;
                    cookie.host_only = false;
                }
                "path" => {
                    if let Some(path) = raw_value.filter(|path| path.starts_with('/')) {
                        cookie.path = path.to_string();
                    }
                }
                "secure" => cookie.secure = true,
                "httponly" if !from_document => cookie.http_only = true,
                "max-age" => {
                    if raw_value
                        .and_then(|value| value.parse::<i64>().ok())
                        .is_some_and(|seconds| seconds <= 0)
                    {
                        delete = true;
                    }
                }
                "samesite"
                    if raw_value.is_some_and(|value| {
                        value.eq_ignore_ascii_case("none") && !cookie.secure
                    }) =>
                {
                    return false;
                }
                _ => {}
            }
        }

        if cookie.secure && !source_url.scheme().eq_ignore_ascii_case("https") {
            return false;
        }

        let partition_key = origin_key(Some(top_level));
        let partition = self.partitions.entry(partition_key).or_default();
        partition.retain(|existing| {
            !(existing.name == cookie.name
                && existing.domain == cookie.domain
                && existing.path == cookie.path)
        });

        if !delete {
            partition.push(cookie);
        }
        true
    }
}

pub fn origin_key(url: Option<&Url>) -> String {
    match url {
        Some(url) => url.origin().ascii_serialization(),
        None => "quantic://local".to_string(),
    }
}

fn is_first_party(top_level: &Url, target: &Url) -> bool {
    let Some(top_host) = top_level.host_str().map(str::to_ascii_lowercase) else {
        return false;
    };
    let Some(target_host) = target.host_str().map(str::to_ascii_lowercase) else {
        return false;
    };
    top_host == target_host
        || top_host.ends_with(&format!(".{target_host}"))
        || target_host.ends_with(&format!(".{top_host}"))
}

fn normalized_request_path(url: &Url) -> String {
    let path = url.path();
    if path.is_empty() {
        "/".to_string()
    } else {
        path.to_string()
    }
}

fn default_cookie_path(url: &Url) -> String {
    let path = normalized_request_path(url);
    if path == "/" || !path.starts_with('/') {
        return "/".to_string();
    }
    let Some(index) = path.rfind('/') else {
        return "/".to_string();
    };
    if index == 0 {
        "/".to_string()
    } else {
        path[..index].to_string()
    }
}

fn domain_matches(host: &str, cookie: &Cookie) -> bool {
    if cookie.host_only {
        host == cookie.domain
    } else {
        host == cookie.domain || host.ends_with(&format!(".{}", cookie.domain))
    }
}

fn path_matches(request_path: &str, cookie_path: &str) -> bool {
    if request_path == cookie_path {
        return true;
    }
    if !request_path.starts_with(cookie_path) {
        return false;
    }
    cookie_path.ends_with('/')
        || request_path
            .as_bytes()
            .get(cookie_path.len())
            .is_some_and(|byte| *byte == b'/')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_is_partitioned_by_origin() {
        let mut storage = PartitionedStorage::default();
        storage.apply(
            "https://a.example",
            &[StorageMutation::Set {
                key: "token".into(),
                value: "A".into(),
            }],
        );
        storage.apply(
            "https://b.example",
            &[StorageMutation::Set {
                key: "token".into(),
                value: "B".into(),
            }],
        );

        assert_eq!(
            storage
                .snapshot("https://a.example")
                .get("token")
                .map(String::as_str),
            Some("A")
        );
        assert_eq!(
            storage
                .snapshot("https://b.example")
                .get("token")
                .map(String::as_str),
            Some("B")
        );
    }

    #[test]
    fn cookies_are_first_party_partitioned_and_hide_httponly_from_document() {
        let top = Url::parse("https://app.example/account").unwrap();
        let third = Url::parse("https://tracker.invalid/pixel").unwrap();
        let mut jar = PartitionedCookieJar::default();

        assert!(jar.store_response_cookie(
            &top,
            &top,
            "session=abc; Path=/; Secure; HttpOnly; SameSite=Lax"
        ));
        assert!(!jar.store_response_cookie(&top, &third, "track=1; Path=/"));
        assert_eq!(jar.document_cookie(&top, &top), "");
        assert_eq!(
            jar.request_header(&top, &top).as_deref(),
            Some("session=abc")
        );
        assert_eq!(jar.request_header(&top, &third), None);
    }

    #[test]
    fn document_cookie_can_set_and_delete_its_first_party_partition() {
        let top = Url::parse("https://app.example/a/page").unwrap();
        let mut jar = PartitionedCookieJar::default();

        assert!(jar.store_document_cookie(&top, &top, "theme=dark; Path=/"));
        assert_eq!(jar.document_cookie(&top, &top), "theme=dark");
        assert!(jar.store_document_cookie(&top, &top, "theme=gone; Path=/; Max-Age=0"));
        assert_eq!(jar.document_cookie(&top, &top), "");
    }
}
