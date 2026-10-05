//! Quantic Fusion engine composition.
//!
//! Quantic is the public engine boundary. Upstream engines/components are internal
//! implementation details selected for standards coverage, performance and privacy.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpstreamFamily {
    Quantic,
    Servo,
    LadybirdLibWeb,
    MozillaGecko,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FusionComponent {
    pub name: &'static str,
    pub family: UpstreamFamily,
    pub role: &'static str,
    pub runtime: bool,
}

pub const FUSION_COMPONENTS: &[FusionComponent] = &[
    FusionComponent {
        name: "Quantic Engine",
        family: UpstreamFamily::Quantic,
        role: "browser policy, privacy, automation, orchestration and product API",
        runtime: true,
    },
    FusionComponent {
        name: "Servo 0.7",
        family: UpstreamFamily::Servo,
        role: "primary standards runtime and rendering backend",
        runtime: cfg!(feature = "fusion-servo"),
    },
    FusionComponent {
        name: "Stylo",
        family: UpstreamFamily::MozillaGecko,
        role: "CSS style system shared by Servo and Firefox/Gecko lineage",
        runtime: cfg!(feature = "fusion-servo"),
    },
    FusionComponent {
        name: "SpiderMonkey",
        family: UpstreamFamily::MozillaGecko,
        role: "JavaScript runtime used by Servo; Mozilla engine lineage",
        runtime: cfg!(feature = "fusion-servo"),
    },
    FusionComponent {
        name: "WebRender",
        family: UpstreamFamily::MozillaGecko,
        role: "GPU rendering/compositing lineage shared across Servo/Mozilla",
        runtime: cfg!(feature = "fusion-servo"),
    },
    FusionComponent {
        name: "Neqo",
        family: UpstreamFamily::MozillaGecko,
        role: "Mozilla QUIC/HTTP3 transport",
        runtime: cfg!(feature = "fusion-gecko-network"),
    },
    FusionComponent {
        name: "Ladybird LibWeb",
        family: UpstreamFamily::LadybirdLibWeb,
        role: "HTML/CSS behavior source and differential-conformance oracle during ports",
        runtime: false,
    },
];

pub fn active_runtime_components() -> impl Iterator<Item = &'static FusionComponent> {
    FUSION_COMPONENTS.iter().filter(|component| component.runtime)
}

pub fn forbidden_runtime_families() -> &'static [&'static str] {
    &["Chromium", "Electron", "CEF", "WebView2", "WebKit"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantic_is_always_the_public_engine_boundary() {
        assert!(FUSION_COMPONENTS.iter().any(|component| {
            component.family == UpstreamFamily::Quantic && component.runtime
        }));
    }

    #[test]
    fn chromium_family_is_explicitly_forbidden() {
        assert!(forbidden_runtime_families().contains(&"Chromium"));
        assert!(forbidden_runtime_families().contains(&"Electron"));
    }
}
