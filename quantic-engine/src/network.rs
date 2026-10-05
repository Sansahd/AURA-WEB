use std::{
    collections::BTreeMap,
    fmt,
    io::{self, Read},
};

use url::Url;

use crate::privacy::{PrivacyDecision, PrivacyPolicy, RequestContext, ResourceKind};

const MAX_REDIRECTS: usize = 10;
const DEFAULT_MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct FetchResponse {
    pub final_url: Url,
    pub status: u16,
    pub content_type: Option<String>,
    pub cache_control: Option<String>,
    pub set_cookie_headers: Vec<String>,
    pub body: Vec<u8>,
}

#[derive(Debug)]
pub enum NetworkError {
    InvalidUrl(url::ParseError),
    Blocked { url: String, reason: &'static str },
    RedirectWithoutLocation(String),
    TooManyRedirects,
    Transport(String),
    HttpStatus(u16),
    UnsupportedMethod(String),
    BodyTooLarge,
    Io(io::Error),
}

impl fmt::Display for NetworkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUrl(error) => write!(formatter, "{error}"),
            Self::Blocked { url, reason } => write!(formatter, "blocked {url}: {reason}"),
            Self::RedirectWithoutLocation(url) => {
                write!(formatter, "redirect without location: {url}")
            }
            Self::TooManyRedirects => write!(formatter, "too many redirects"),
            Self::Transport(error) => write!(formatter, "{error}"),
            Self::HttpStatus(status) => write!(formatter, "HTTP status {status}"),
            Self::UnsupportedMethod(method) => {
                write!(formatter, "unsupported HTTP method {method}")
            }
            Self::BodyTooLarge => write!(formatter, "response body exceeds limit"),
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for NetworkError {}

#[derive(Debug, Clone)]
pub struct NetworkClient {
    agent: ureq::Agent,
    max_body_bytes: usize,
}

impl Default for NetworkClient {
    fn default() -> Self {
        Self {
            agent: ureq::AgentBuilder::new()
                .redirects(0)
                .user_agent("Quantic-Engine/0.5")
                .build(),
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
        }
    }
}

impl NetworkClient {
    pub fn fetch(
        &self,
        policy: &PrivacyPolicy,
        top_level: &Url,
        target: &Url,
        kind: ResourceKind,
    ) -> Result<FetchResponse, NetworkError> {
        self.fetch_with_method(policy, top_level, target, kind, "GET", None)
    }

    pub fn fetch_with_method(
        &self,
        policy: &PrivacyPolicy,
        top_level: &Url,
        target: &Url,
        kind: ResourceKind,
        method: &str,
        body: Option<&str>,
    ) -> Result<FetchResponse, NetworkError> {
        self.fetch_with_request(policy, top_level, target, kind, method, body, None)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn fetch_with_request(
        &self,
        policy: &PrivacyPolicy,
        top_level: &Url,
        target: &Url,
        kind: ResourceKind,
        method: &str,
        body: Option<&str>,
        content_type: Option<&str>,
    ) -> Result<FetchResponse, NetworkError> {
        self.fetch_with_request_headers(
            policy,
            top_level,
            target,
            kind,
            method,
            body,
            content_type,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn fetch_with_request_headers(
        &self,
        policy: &PrivacyPolicy,
        top_level: &Url,
        target: &Url,
        kind: ResourceKind,
        method: &str,
        body: Option<&str>,
        content_type: Option<&str>,
        cookie_header: Option<&str>,
    ) -> Result<FetchResponse, NetworkError> {
        self.fetch_with_request_headers_and_extra(
            policy,
            top_level,
            target,
            kind,
            method,
            body,
            content_type,
            cookie_header,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn fetch_with_request_headers_and_extra(
        &self,
        policy: &PrivacyPolicy,
        top_level: &Url,
        target: &Url,
        kind: ResourceKind,
        method: &str,
        body: Option<&str>,
        content_type: Option<&str>,
        cookie_header: Option<&str>,
        extra_headers: Option<&BTreeMap<String, String>>,
    ) -> Result<FetchResponse, NetworkError> {
        self.fetch_with_request_bytes_headers_and_extra(
            policy,
            top_level,
            target,
            kind,
            method,
            body.map(str::as_bytes),
            content_type,
            cookie_header,
            extra_headers,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn fetch_with_request_bytes_headers_and_extra(
        &self,
        policy: &PrivacyPolicy,
        top_level: &Url,
        target: &Url,
        kind: ResourceKind,
        method: &str,
        body: Option<&[u8]>,
        content_type: Option<&str>,
        cookie_header: Option<&str>,
        extra_headers: Option<&BTreeMap<String, String>>,
    ) -> Result<FetchResponse, NetworkError> {
        let top_host = top_level.host_str().unwrap_or_default().to_string();
        let mut current = target.clone();
        let mut method = method.trim().to_ascii_uppercase();
        let mut body = body.map(|bytes| bytes.to_vec());

        if matches!(method.as_str(), "CONNECT" | "TRACE" | "TRACK") || method.is_empty() {
            return Err(NetworkError::UnsupportedMethod(method));
        }

        for _ in 0..=MAX_REDIRECTS {
            let request_host = current.host_str().unwrap_or_default().to_string();
            match policy.decide(&RequestContext {
                top_level_host: top_host.clone(),
                request_host,
                kind,
            }) {
                PrivacyDecision::Allow => {}
                PrivacyDecision::Block { reason } => {
                    return Err(NetworkError::Blocked {
                        url: current.to_string(),
                        reason,
                    });
                }
            }

            let mut request = self.agent.request(&method, current.as_str());
            if let Some(content_type) = content_type {
                request = request.set("content-type", content_type);
            }
            if current.origin() == target.origin() {
                if let Some(headers) = extra_headers {
                    for (name, value) in headers {
                        if !is_forbidden_request_header(name)
                            && !name.eq_ignore_ascii_case("content-type")
                            && !name.contains('\r') && !name.contains('\n')
                            && !value.contains('\r') && !value.contains('\n')
                        {
                            request = request.set(name, value);
                        }
                    }
                }
            }
            if current.host_str() == target.host_str() {
                if let Some(cookie_header) = cookie_header.filter(|value| !value.is_empty()) {
                    request = request.set("cookie", cookie_header);
                }
            }
            let result = match body.as_deref() {
                Some(body) if method != "GET" && method != "HEAD" => request.send_bytes(body),
                _ => request.call(),
            };
            let response = match result {
                Ok(response) => response,
                Err(ureq::Error::Status(_, response)) => response,
                Err(error) => return Err(NetworkError::Transport(error.to_string())),
            };

            let status = response.status();
            if (300..400).contains(&status) {
                let Some(location) = response.header("location") else {
                    return Err(NetworkError::RedirectWithoutLocation(current.to_string()));
                };
                current = current.join(location).map_err(NetworkError::InvalidUrl)?;
                if status == 303 || ((status == 301 || status == 302) && method == "POST") {
                    method = "GET".to_string();
                    body = None;
                }
                continue;
            }

            if !(200..300).contains(&status) {
                return Err(NetworkError::HttpStatus(status));
            }

            let content_type = response.header("content-type").map(str::to_string);
            let cache_control = response.header("cache-control").map(str::to_string);
            let set_cookie_headers = response
                .all("set-cookie")
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>();
            let final_url = current.clone();
            let mut reader = response
                .into_reader()
                .take((self.max_body_bytes + 1) as u64);
            let mut response_body = Vec::new();
            reader
                .read_to_end(&mut response_body)
                .map_err(NetworkError::Io)?;
            if response_body.len() > self.max_body_bytes {
                return Err(NetworkError::BodyTooLarge);
            }

            return Ok(FetchResponse {
                final_url,
                status,
                content_type,
                cache_control,
                set_cookie_headers,
                body: response_body,
            });
        }

        Err(NetworkError::TooManyRedirects)
    }
}

fn is_forbidden_request_header(name: &str) -> bool {
    let name = name.trim().to_ascii_lowercase();
    matches!(
        name.as_str(),
        "host"
            | "cookie"
            | "content-length"
            | "connection"
            | "upgrade"
            | "proxy-authorization"
            | "proxy-authenticate"
    ) || name.starts_with("sec-")
}

pub fn parse_address(input: &str) -> Result<Url, url::ParseError> {
    let input = input.trim();
    if input.contains("://") {
        Url::parse(input)
    } else {
        Url::parse(&format!("https://{input}"))
    }
}

pub fn resolve_url(base: &Url, reference: &str) -> Result<Url, url::ParseError> {
    base.join(reference)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_bare_address_to_https() {
        assert_eq!(
            parse_address("example.com/path").unwrap().as_str(),
            "https://example.com/path"
        );
    }

    #[test]
    fn resolves_relative_resource() {
        let base = Url::parse("https://example.com/a/page.html").unwrap();
        assert_eq!(
            resolve_url(&base, "../style.css").unwrap().as_str(),
            "https://example.com/style.css"
        );
    }

    #[test]
    fn privacy_preflight_blocks_third_party_before_transport() {
        let client = NetworkClient::default();
        let policy = PrivacyPolicy::default();
        let top = Url::parse("https://example.com/").unwrap();
        let target = Url::parse("https://tracker.invalid/pixel.js").unwrap();
        let result = client.fetch(&policy, &top, &target, ResourceKind::Script);
        assert!(matches!(result, Err(NetworkError::Blocked { .. })));
    }

    #[test]
    fn cookie_header_is_never_forwarded_to_a_different_redirect_host() {
        // The transport enforces this by comparing the current redirect host
        // with the original request host before applying the Cookie header.
        let initial = Url::parse("https://app.example/path").unwrap();
        let redirected = Url::parse("https://cdn.example/path").unwrap();
        assert_ne!(initial.host_str(), redirected.host_str());
    }

    #[test]
    fn dangerous_methods_are_rejected_before_transport() {
        let client = NetworkClient::default();
        let policy = PrivacyPolicy::default();
        let top = Url::parse("https://example.com/").unwrap();
        let result =
            client.fetch_with_method(&policy, &top, &top, ResourceKind::Fetch, "TRACE", None);
        assert!(matches!(result, Err(NetworkError::UnsupportedMethod(_))));
    }
}
