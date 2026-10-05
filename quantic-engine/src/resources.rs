use std::collections::BTreeSet;

use url::Url;

use crate::{dom::Document, privacy::ResourceKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResourceClass {
    Stylesheet,
    Script,
    Image,
    Preload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceCandidate {
    pub node_id: usize,
    pub class: ResourceClass,
    pub url: Url,
    pub render_blocking: bool,
    pub preload_as: Option<String>,
}

impl ResourceCandidate {
    pub fn fetch_kind(&self) -> ResourceKind {
        match self.class {
            ResourceClass::Stylesheet => ResourceKind::Style,
            ResourceClass::Script => ResourceKind::Script,
            ResourceClass::Image => ResourceKind::Image,
            ResourceClass::Preload => match self.preload_as.as_deref() {
                Some("style") => ResourceKind::Style,
                Some("script") => ResourceKind::Script,
                Some("image") => ResourceKind::Image,
                Some("font") => ResourceKind::Font,
                _ => ResourceKind::Other,
            },
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResourcePlan {
    pub candidates: Vec<ResourceCandidate>,
}

impl ResourcePlan {
    pub fn discover(document: &Document, base: &Url) -> Self {
        let mut candidates = Vec::new();
        let mut seen = BTreeSet::new();

        for node_id in 0..document.nodes.len() {
            let Some(tag) = document.element_tag(node_id) else {
                continue;
            };

            let candidate = match tag {
                "link" => discover_link(document, node_id, base),
                "script" => document
                    .attribute(node_id, "src")
                    .and_then(|src| resolve(base, src))
                    .map(|url| ResourceCandidate {
                        node_id,
                        class: ResourceClass::Script,
                        url,
                        render_blocking: !document
                            .attributes(node_id)
                            .unwrap_or_default()
                            .iter()
                            .any(|attribute| {
                                attribute.name.eq_ignore_ascii_case("async")
                                    || attribute.name.eq_ignore_ascii_case("defer")
                            }),
                        preload_as: None,
                    }),
                "img" => discover_image(document, node_id, base),
                _ => None,
            };

            let Some(candidate) = candidate else {
                continue;
            };
            let key = match candidate.class {
                ResourceClass::Image | ResourceClass::Script => {
                    format!("{:?}:{}:{}", candidate.class, candidate.node_id, candidate.url)
                }
                _ => format!("{:?}:{}", candidate.class, candidate.url),
            };
            if seen.insert(key) {
                candidates.push(candidate);
            }
        }

        candidates.sort_by_key(|candidate| {
            (
                !candidate.render_blocking,
                candidate.class,
                candidate.node_id,
            )
        });
        Self { candidates }
    }

    pub fn render_blocking(&self) -> impl Iterator<Item = &ResourceCandidate> {
        self.candidates
            .iter()
            .filter(|candidate| candidate.render_blocking)
    }

    pub fn deferred(&self) -> impl Iterator<Item = &ResourceCandidate> {
        self.candidates
            .iter()
            .filter(|candidate| !candidate.render_blocking)
    }
}

fn discover_image(document: &Document, node_id: usize, base: &Url) -> Option<ResourceCandidate> {
    let reference = picture_source(document, node_id)
        .or_else(|| document.attribute(node_id, "srcset").and_then(select_srcset_candidate))
        .or_else(|| document.attribute(node_id, "src"))?;
    let url = resolve(base, reference)?;
    Some(ResourceCandidate {
        node_id,
        class: ResourceClass::Image,
        url,
        render_blocking: false,
        preload_as: None,
    })
}

fn picture_source<'a>(document: &'a Document, image_id: usize) -> Option<&'a str> {
    let parent = document.nodes.get(image_id)?.parent?;
    if document.element_tag(parent) != Some("picture") {
        return None;
    }
    for child in &document.nodes[parent].children {
        if *child == image_id {
            break;
        }
        if document.element_tag(*child) != Some("source") {
            continue;
        }
        if let Some(candidate) = document
            .attribute(*child, "srcset")
            .and_then(select_srcset_candidate)
        {
            return Some(candidate);
        }
    }
    None
}

fn select_srcset_candidate(value: &str) -> Option<&str> {
    let candidates = value
        .split(',')
        .filter_map(|candidate| {
            let mut parts = candidate.split_ascii_whitespace();
            let url = parts.next()?.trim();
            if url.is_empty() {
                return None;
            }
            let descriptor = parts.next().unwrap_or("1x").trim();
            let density = descriptor
                .strip_suffix('x')
                .and_then(|raw| raw.parse::<f32>().ok())
                .unwrap_or(1.0);
            Some((url, density))
        })
        .collect::<Vec<_>>();

    candidates
        .iter()
        .filter(|(_, density)| *density >= 1.0)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .or_else(|| candidates.last())
        .map(|(url, _)| *url)
}

fn discover_link(document: &Document, node_id: usize, base: &Url) -> Option<ResourceCandidate> {
    let rel = document.attribute(node_id, "rel").unwrap_or_default();
    let href = document.attribute(node_id, "href")?;
    let url = resolve(base, href)?;
    let tokens = rel
        .split_ascii_whitespace()
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();

    if tokens.iter().any(|token| token == "stylesheet") {
        return Some(ResourceCandidate {
            node_id,
            class: ResourceClass::Stylesheet,
            url,
            render_blocking: true,
            preload_as: None,
        });
    }
    if tokens.iter().any(|token| token == "preload") {
        return Some(ResourceCandidate {
            node_id,
            class: ResourceClass::Preload,
            url,
            render_blocking: false,
            preload_as: document
                .attribute(node_id, "as")
                .map(|value| value.trim().to_ascii_lowercase()),
        });
    }
    None
}

fn resolve(base: &Url, reference: &str) -> Option<Url> {
    let reference = reference.trim();
    if reference.is_empty() || reference.starts_with("data:") || reference.starts_with("blob:") {
        return None;
    }
    base.join(reference).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html;

    #[test]
    fn responsive_images_remain_node_addressable() {
        let document = html::parse(
            r#"<picture>
                 <source srcset="/hero.webp 1x, /hero@2x.webp 2x">
                 <img id="one" src="/fallback.jpg">
               </picture>
               <img id="two" srcset="/same.png 1x, /same@2x.png 2x">
               <img id="three" src="/same.png">"#,
        );
        let base = Url::parse("https://quantic.test/").unwrap();
        let plan = ResourcePlan::discover(&document, &base);
        let images = plan
            .candidates
            .iter()
            .filter(|candidate| candidate.class == ResourceClass::Image)
            .collect::<Vec<_>>();

        assert_eq!(images.len(), 3);
        assert_eq!(images[0].url.as_str(), "https://quantic.test/hero.webp");
        assert_eq!(images[1].url.as_str(), "https://quantic.test/same.png");
        assert_eq!(images[2].url.as_str(), "https://quantic.test/same.png");
        assert_ne!(images[1].node_id, images[2].node_id);
    }

    #[test]
    fn preload_kind_follows_as_attribute() {
        let document = html::parse(
            r#"<link rel="preload" as="font" href="/font.woff2">
               <link rel="preload" as="image" href="/hero.webp">"#,
        );
        let base = Url::parse("https://quantic.test/").unwrap();
        let plan = ResourcePlan::discover(&document, &base);

        assert_eq!(plan.candidates[0].fetch_kind(), ResourceKind::Font);
        assert_eq!(plan.candidates[1].fetch_kind(), ResourceKind::Image);
    }

    #[test]
    fn discovers_and_prioritizes_document_resources() {
        let document = html::parse(
            r#"<link rel="stylesheet" href="/app.css">
               <script src="/app.js"></script>
               <script async src="/async.js"></script>
               <img src="/hero.webp">
               <link rel="preload" href="/font.woff2">"#,
        );
        let base = Url::parse("https://quantic.test/a/").unwrap();
        let plan = ResourcePlan::discover(&document, &base);

        assert_eq!(plan.candidates.len(), 5);
        assert_eq!(plan.render_blocking().count(), 2);
        assert_eq!(plan.deferred().count(), 3);
        assert_eq!(
            plan.candidates[0].url.as_str(),
            "https://quantic.test/app.css"
        );
    }
}
