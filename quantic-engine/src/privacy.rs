#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    Document,
    Script,
    Style,
    Image,
    Font,
    Media,
    Fetch,
    WebSocket,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestContext {
    pub top_level_host: String,
    pub request_host: String,
    pub kind: ResourceKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivacyDecision {
    Allow,
    Block { reason: &'static str },
}

#[derive(Debug, Clone)]
pub struct PrivacyPolicy {
    /// Strict means third-party subresources are denied unless explicitly allowed.
    pub strict_third_party: bool,
    allowed_third_party_hosts: Vec<String>,
}

impl Default for PrivacyPolicy {
    fn default() -> Self {
        Self {
            strict_third_party: true,
            allowed_third_party_hosts: Vec::new(),
        }
    }
}

impl PrivacyPolicy {
    pub fn allow_third_party_host(&mut self, host: impl Into<String>) {
        let host = normalize_host(&host.into());
        if !host.is_empty() && !self.allowed_third_party_hosts.contains(&host) {
            self.allowed_third_party_hosts.push(host);
        }
    }

    pub fn decide(&self, request: &RequestContext) -> PrivacyDecision {
        let top = normalize_host(&request.top_level_host);
        let target = normalize_host(&request.request_host);

        if target.is_empty() {
            return PrivacyDecision::Block {
                reason: "invalid-host",
            };
        }

        if same_site(&top, &target) {
            return PrivacyDecision::Allow;
        }

        if self
            .allowed_third_party_hosts
            .iter()
            .any(|allowed| same_site(allowed, &target))
        {
            return PrivacyDecision::Allow;
        }

        if self.strict_third_party && request.kind != ResourceKind::Document {
            return PrivacyDecision::Block {
                reason: "third-party-denied-by-default",
            };
        }

        PrivacyDecision::Allow
    }
}

fn normalize_host(host: &str) -> String {
    host.trim()
        .trim_end_matches('.')
        .to_ascii_lowercase()
        .strip_prefix("www.")
        .unwrap_or(host.trim().trim_end_matches('.'))
        .to_ascii_lowercase()
}

fn same_site(a: &str, b: &str) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }
    a == b || a.ends_with(&format!(".{b}")) || b.ends_with(&format!(".{a}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_third_party_script_by_default() {
        let policy = PrivacyPolicy::default();
        let decision = policy.decide(&RequestContext {
            top_level_host: "example.com".into(),
            request_host: "tracker.invalid".into(),
            kind: ResourceKind::Script,
        });
        assert!(matches!(decision, PrivacyDecision::Block { .. }));
    }

    #[test]
    fn allows_same_site_resource() {
        let policy = PrivacyPolicy::default();
        assert_eq!(
            policy.decide(&RequestContext {
                top_level_host: "example.com".into(),
                request_host: "static.example.com".into(),
                kind: ResourceKind::Style,
            }),
            PrivacyDecision::Allow
        );
    }

    #[test]
    fn explicit_exception_is_possible() {
        let mut policy = PrivacyPolicy::default();
        policy.allow_third_party_host("cdn.example.net");
        assert_eq!(
            policy.decide(&RequestContext {
                top_level_host: "example.com".into(),
                request_host: "cdn.example.net".into(),
                kind: ResourceKind::Font,
            }),
            PrivacyDecision::Allow
        );
    }
}
