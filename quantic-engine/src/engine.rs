use std::{
    collections::BTreeMap,
    fmt,
    path::Path,
    sync::{Arc, Mutex, RwLock},
};

use image::RgbaImage;
use url::Url;

use crate::{
    cache::{is_cacheable_kind, ResourceCache},
    css,
    dom::{Document, NodeKind},
    font::{self, FontSystem},
    html, image_decode,
    interactive::{InteractivePage, NavigationRequest, PageError},
    js_runtime::ConsoleMessage,
    layout::{self, DisplayItem, ImageDimensions},
    network::{self, FetchResponse, NetworkClient, NetworkError},
    paint::{self, DecodedImages},
    privacy::{PrivacyDecision, PrivacyPolicy, RequestContext, ResourceKind},
    resources::{ResourceClass, ResourcePlan},
    storage::{origin_key, PartitionedCookieJar, PartitionedStorage},
    style::{self, ComputedStyle},
    url_context::effective_base_url,
};

#[derive(Debug, Clone)]
pub struct EngineOutput {
    pub document: Document,
    pub styles: Vec<ComputedStyle>,
    pub display_list: Vec<DisplayItem>,
    pub decoded_images: DecodedImages,
    pub viewport_width: u32,
    pub content_height: u32,
    pub base_url: Option<Url>,
    pub blocked_resources: Vec<String>,
    pub console_messages: Vec<ConsoleMessage>,
    pub script_errors: Vec<String>,
}

#[derive(Debug)]
pub enum EngineLoadError {
    InvalidUrl(url::ParseError),
    Network(NetworkError),
    Image(image::ImageError),
}

impl fmt::Display for EngineLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUrl(error) => write!(formatter, "invalid URL: {error}"),
            Self::Network(error) => write!(formatter, "network error: {error}"),
            Self::Image(error) => write!(formatter, "image error: {error}"),
        }
    }
}

impl std::error::Error for EngineLoadError {}

#[derive(Debug, Clone)]
pub struct Engine {
    pub(crate) privacy: PrivacyPolicy,
    pub(crate) network: NetworkClient,
    pub(crate) viewport_width: u32,
    pub(crate) storage: Arc<Mutex<PartitionedStorage>>,
    pub(crate) cookies: Arc<Mutex<PartitionedCookieJar>>,
    pub(crate) cache: Arc<Mutex<ResourceCache>>,
    pub(crate) fonts: Arc<RwLock<FontSystem>>,
}

impl Default for Engine {
    fn default() -> Self {
        Self {
            privacy: PrivacyPolicy::default(),
            network: NetworkClient::default(),
            viewport_width: 1024,
            storage: Arc::new(Mutex::new(PartitionedStorage::default())),
            cookies: Arc::new(Mutex::new(PartitionedCookieJar::default())),
            cache: Arc::new(Mutex::new(ResourceCache::default())),
            fonts: Arc::new(RwLock::new(FontSystem::system())),
        }
    }
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_viewport_width(mut self, width: u32) -> Self {
        self.viewport_width = width.max(240);
        self
    }

    pub fn resource_cache_entries(&self) -> usize {
        self.cache
            .lock()
            .map(|mut cache| cache.len())
            .unwrap_or_default()
    }

    pub fn resource_cache_bytes(&self) -> usize {
        self.cache
            .lock()
            .map(|mut cache| cache.total_bytes())
            .unwrap_or_default()
    }

    pub fn clear_resource_cache(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }

    pub fn font_face_count(&self) -> usize {
        self.fonts
            .read()
            .map(|fonts| fonts.face_count())
            .unwrap_or_default()
    }

    pub fn register_font_bytes(
        &self,
        family_alias: &str,
        bytes: Vec<u8>,
    ) -> Result<usize, &'static str> {
        self.fonts
            .write()
            .map(|mut fonts| fonts.register_font_bytes(family_alias, bytes))
            .map_err(|_| "font database lock is poisoned")
    }

    pub fn privacy(&self) -> &PrivacyPolicy {
        &self.privacy
    }

    pub fn privacy_mut(&mut self) -> &mut PrivacyPolicy {
        &mut self.privacy
    }

    pub fn evaluate_request(&self, request: &RequestContext) -> PrivacyDecision {
        self.privacy.decide(request)
    }

    pub fn load_html(&self, source: &str) -> EngineOutput {
        self.load_document(source, None, String::new(), Vec::new())
    }

    pub fn load_html_with_css(&self, source: &str, css_source: &str) -> EngineOutput {
        self.load_document(source, None, css_source.to_string(), Vec::new())
    }

    pub fn load_interactive_html(&self, source: &str) -> Result<InteractivePage, PageError> {
        InteractivePage::from_html(self.clone(), source)
    }

    pub fn load_interactive_url(&self, address: &str) -> Result<InteractivePage, PageError> {
        InteractivePage::from_url(self.clone(), address)
    }

    pub fn follow_navigation(
        &self,
        request: &NavigationRequest,
    ) -> Result<InteractivePage, PageError> {
        InteractivePage::from_navigation(self.clone(), request)
    }

    pub fn load_interactive_html_with_base(
        &self,
        source: &str,
        base_address: &str,
    ) -> Result<InteractivePage, PageError> {
        let base_url = network::parse_address(base_address).map_err(PageError::InvalidUrl)?;
        InteractivePage::from_html_with_base(self.clone(), source, base_url)
    }

    pub fn storage_snapshot(
        &self,
        url: Option<&Url>,
    ) -> std::collections::BTreeMap<String, String> {
        let origin = origin_key(url);
        self.storage
            .lock()
            .map(|storage| storage.snapshot(&origin))
            .unwrap_or_default()
    }

    pub fn load_url(&self, address: &str) -> Result<EngineOutput, EngineLoadError> {
        let url = network::parse_address(address).map_err(EngineLoadError::InvalidUrl)?;
        let response = self
            .fetch_resource(&url, &url, ResourceKind::Document, "GET", None, None)
            .map_err(EngineLoadError::Network)?;
        let document_url = response.final_url.clone();
        let source = String::from_utf8_lossy(&response.body).into_owned();
        let document = html::parse(&source);
        let base_url = effective_base_url(&document, &document_url);

        let mut blocked_resources = Vec::new();
        let plan = ResourcePlan::discover(&document, &base_url);
        self.warm_preloads(&plan, &document_url, &mut blocked_resources);
        let css_source = self.load_external_styles(&plan, &document_url, &mut blocked_resources);
        let decoded_images =
            self.fetch_document_images_from_plan(&plan, &document_url, &mut blocked_resources);

        Ok(self.render_existing_document_with_images(
            document,
            Some(base_url),
            css_source,
            blocked_resources,
            Vec::new(),
            Vec::new(),
            decoded_images,
        ))
    }

    pub fn rasterize(&self, output: &EngineOutput) -> RgbaImage {
        match self.fonts.read() {
            Ok(fonts) => paint::rasterize_with_images_and_fonts(
                &output.display_list,
                output.viewport_width,
                output.content_height.max(200),
                &output.decoded_images,
                &fonts,
            ),
            Err(_) => {
                let fallback = FontSystem::empty();
                paint::rasterize_with_images_and_fonts(
                    &output.display_list,
                    output.viewport_width,
                    output.content_height.max(200),
                    &output.decoded_images,
                    &fallback,
                )
            }
        }
    }

    pub fn save_png<P: AsRef<Path>>(
        &self,
        output: &EngineOutput,
        path: P,
    ) -> Result<(), EngineLoadError> {
        self.rasterize(output)
            .save(path)
            .map_err(EngineLoadError::Image)
    }

    fn load_document(
        &self,
        source: &str,
        base_url: Option<Url>,
        extra_css: String,
        blocked_resources: Vec<String>,
    ) -> EngineOutput {
        self.render_existing_document(
            html::parse(source),
            base_url,
            extra_css,
            blocked_resources,
            Vec::new(),
            Vec::new(),
        )
    }

    pub(crate) fn render_existing_document(
        &self,
        document: Document,
        base_url: Option<Url>,
        extra_css: String,
        blocked_resources: Vec<String>,
        console_messages: Vec<ConsoleMessage>,
        script_errors: Vec<String>,
    ) -> EngineOutput {
        self.render_existing_document_with_images(
            document,
            base_url,
            extra_css,
            blocked_resources,
            console_messages,
            script_errors,
            DecodedImages::new(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_existing_document_with_images(
        &self,
        document: Document,
        base_url: Option<Url>,
        extra_css: String,
        mut blocked_resources: Vec<String>,
        console_messages: Vec<ConsoleMessage>,
        script_errors: Vec<String>,
        decoded_images: DecodedImages,
    ) -> EngineOutput {
        let embedded = embedded_css(&document);
        if let Some(base) = base_url.as_ref() {
            self.load_web_fonts_from_css(base, base, &embedded, &mut blocked_resources);
        }

        let mut css_source = embedded;
        if !extra_css.trim().is_empty() {
            css_source.push('\n');
            css_source.push_str(&extra_css);
        }
        let stylesheet = css::parse_stylesheet_for_viewport(&css_source, self.viewport_width);
        let styles = style::compute_styles(&document, &stylesheet);
        let image_dimensions = decoded_images
            .iter()
            .map(|(id, image)| (*id, (image.width(), image.height())))
            .collect::<ImageDimensions>();
        let layout = match self.fonts.read() {
            Ok(fonts) => layout::build_display_list_with_resources(
                &document,
                &styles,
                self.viewport_width,
                &image_dimensions,
                &fonts,
            ),
            Err(_) => {
                let fallback = FontSystem::empty();
                layout::build_display_list_with_resources(
                    &document,
                    &styles,
                    self.viewport_width,
                    &image_dimensions,
                    &fallback,
                )
            }
        };

        EngineOutput {
            document,
            styles,
            display_list: layout.items,
            decoded_images,
            viewport_width: self.viewport_width,
            content_height: layout.content_height,
            base_url,
            blocked_resources,
            console_messages,
            script_errors,
        }
    }

    pub(crate) fn load_web_fonts_from_css(
        &self,
        top_level: &Url,
        css_base: &Url,
        source: &str,
        blocked_resources: &mut Vec<String>,
    ) {
        for rule in font::parse_font_face_rules(source) {
            let already_loaded = self
                .fonts
                .read()
                .map(|fonts| fonts.has_alias(&rule.family))
                .unwrap_or(false);
            if already_loaded {
                continue;
            }

            let mut loaded = false;
            for source_url in &rule.sources {
                let target = match network::resolve_url(css_base, source_url) {
                    Ok(target) => target,
                    Err(error) => {
                        blocked_resources.push(format!("{source_url} (invalid font URL: {error})"));
                        continue;
                    }
                };

                match self.fetch_resource(top_level, &target, ResourceKind::Font, "GET", None, None)
                {
                    Ok(response) => {
                        let registered = self
                            .fonts
                            .write()
                            .map(|mut fonts| fonts.register_font_bytes(&rule.family, response.body))
                            .unwrap_or(0);
                        if registered > 0 {
                            loaded = true;
                            break;
                        }
                        blocked_resources.push(format!(
                            "{} (font format not yet supported by Quantic)",
                            response.final_url
                        ));
                    }
                    Err(NetworkError::Blocked { url, reason }) => {
                        blocked_resources.push(format!("{url} (font blocked: {reason})"));
                    }
                    Err(error) => {
                        blocked_resources.push(format!("{target} (font load failed: {error})"));
                    }
                }
            }

            if !loaded {
                blocked_resources.push(format!(
                    "font-family '{}' unavailable after @font-face resolution",
                    rule.family
                ));
            }
        }

        blocked_resources.sort();
        blocked_resources.dedup();
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn fetch_resource(
        &self,
        top_level: &Url,
        target: &Url,
        kind: ResourceKind,
        method: &str,
        body: Option<&str>,
        content_type: Option<&str>,
    ) -> Result<FetchResponse, NetworkError> {
        self.fetch_resource_with_headers(
            top_level,
            target,
            kind,
            method,
            body,
            content_type,
            &BTreeMap::new(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn fetch_resource_with_headers(
        &self,
        top_level: &Url,
        target: &Url,
        kind: ResourceKind,
        method: &str,
        body: Option<&str>,
        content_type: Option<&str>,
        headers: &BTreeMap<String, String>,
    ) -> Result<FetchResponse, NetworkError> {
        self.enforce_privacy(top_level, target, kind)?;

        let cookie_header = self
            .cookies
            .lock()
            .ok()
            .and_then(|jar| jar.request_header(top_level, target));
        let cacheable = method.eq_ignore_ascii_case("GET")
            && body.is_none()
            && cookie_header.is_none()
            && headers.is_empty()
            && is_cacheable_kind(kind);

        if cacheable {
            let cached = self
                .cache
                .lock()
                .ok()
                .and_then(|mut cache| cache.get(top_level, target, kind));
            if let Some(response) = cached {
                self.enforce_privacy(top_level, &response.final_url, kind)?;
                return Ok(response);
            }
        }

        let response = self.network.fetch_with_request_headers_and_extra(
            &self.privacy,
            top_level,
            target,
            kind,
            method,
            body,
            content_type,
            cookie_header.as_deref(),
            (!headers.is_empty()).then_some(headers),
        )?;

        if !response.set_cookie_headers.is_empty() {
            let cookie_top = if kind == ResourceKind::Document {
                &response.final_url
            } else {
                top_level
            };
            if let Ok(mut jar) = self.cookies.lock() {
                for header in &response.set_cookie_headers {
                    jar.store_response_cookie(cookie_top, &response.final_url, header);
                }
            }
        } else if cacheable {
            if let Ok(mut cache) = self.cache.lock() {
                cache.put(top_level, target, kind, response.clone());
            }
        }

        Ok(response)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn fetch_resource_bytes(
        &self,
        top_level: &Url,
        target: &Url,
        kind: ResourceKind,
        method: &str,
        body: Option<&[u8]>,
        content_type: Option<&str>,
    ) -> Result<FetchResponse, NetworkError> {
        self.enforce_privacy(top_level, target, kind)?;

        let cookie_header = self
            .cookies
            .lock()
            .ok()
            .and_then(|jar| jar.request_header(top_level, target));

        let response = self.network.fetch_with_request_bytes_headers_and_extra(
            &self.privacy,
            top_level,
            target,
            kind,
            method,
            body,
            content_type,
            cookie_header.as_deref(),
            None,
        )?;

        if !response.set_cookie_headers.is_empty() {
            let cookie_top = if kind == ResourceKind::Document {
                &response.final_url
            } else {
                top_level
            };
            if let Ok(mut jar) = self.cookies.lock() {
                for header in &response.set_cookie_headers {
                    jar.store_response_cookie(cookie_top, &response.final_url, header);
                }
            }
        }

        Ok(response)
    }

    fn enforce_privacy(
        &self,
        top_level: &Url,
        target: &Url,
        kind: ResourceKind,
    ) -> Result<(), NetworkError> {
        match self.privacy.decide(&RequestContext {
            top_level_host: top_level.host_str().unwrap_or_default().to_string(),
            request_host: target.host_str().unwrap_or_default().to_string(),
            kind,
        }) {
            PrivacyDecision::Allow => Ok(()),
            PrivacyDecision::Block { reason } => Err(NetworkError::Blocked {
                url: target.to_string(),
                reason,
            }),
        }
    }

    pub(crate) fn document_cookie(&self, top_level: &Url, page_url: &Url) -> String {
        self.cookies
            .lock()
            .map(|jar| jar.document_cookie(top_level, page_url))
            .unwrap_or_default()
    }

    pub(crate) fn set_document_cookie(
        &self,
        top_level: &Url,
        page_url: &Url,
        assignment: &str,
    ) -> bool {
        self.cookies
            .lock()
            .map(|mut jar| jar.store_document_cookie(top_level, page_url, assignment))
            .unwrap_or(false)
    }

    pub(crate) fn warm_preloads(
        &self,
        plan: &ResourcePlan,
        top_level: &Url,
        blocked_resources: &mut Vec<String>,
    ) {
        for candidate in plan
            .candidates
            .iter()
            .filter(|candidate| candidate.class == ResourceClass::Preload)
        {
            let kind = candidate.fetch_kind();
            if kind == ResourceKind::Other {
                continue;
            }
            match self.fetch_resource(top_level, &candidate.url, kind, "GET", None, None) {
                Ok(_) => {}
                Err(NetworkError::Blocked { url, reason }) => {
                    blocked_resources.push(format!("{url} (preload blocked: {reason})"));
                }
                Err(error) => {
                    blocked_resources.push(format!("{} (preload failed: {error})", candidate.url))
                }
            }
        }
    }

    pub(crate) fn load_external_styles(
        &self,
        plan: &ResourcePlan,
        top_level: &Url,
        blocked_resources: &mut Vec<String>,
    ) -> String {
        let mut css_source = String::new();
        for candidate in plan
            .candidates
            .iter()
            .filter(|candidate| candidate.class == ResourceClass::Stylesheet)
        {
            match self.fetch_resource(
                top_level,
                &candidate.url,
                ResourceKind::Style,
                "GET",
                None,
                None,
            ) {
                Ok(response) => {
                    let source = String::from_utf8_lossy(&response.body).into_owned();
                    self.load_web_fonts_from_css(
                        top_level,
                        &response.final_url,
                        &source,
                        blocked_resources,
                    );
                    css_source.push('\n');
                    css_source.push_str(&source);
                }
                Err(NetworkError::Blocked { url, reason }) => {
                    blocked_resources.push(format!("{url} ({reason})"));
                }
                Err(error) => blocked_resources.push(format!("{} ({error})", candidate.url)),
            }
        }
        css_source
    }

    pub(crate) fn fetch_document_images(
        &self,
        document: &Document,
        document_url: &Url,
        base_url: &Url,
        blocked_resources: &mut Vec<String>,
    ) -> DecodedImages {
        let plan = ResourcePlan::discover(document, base_url);
        self.fetch_document_images_from_plan(&plan, document_url, blocked_resources)
    }

    pub(crate) fn fetch_document_images_from_plan(
        &self,
        plan: &ResourcePlan,
        document_url: &Url,
        blocked_resources: &mut Vec<String>,
    ) -> DecodedImages {
        let mut images = DecodedImages::new();
        for candidate in plan
            .candidates
            .iter()
            .filter(|candidate| candidate.class == ResourceClass::Image)
        {
            match self.fetch_resource(
                document_url,
                &candidate.url,
                ResourceKind::Image,
                "GET",
                None,
                None,
            ) {
                Ok(response) => match image_decode::decode(&response.body) {
                    Ok(decoded) => {
                        images.insert(candidate.node_id, decoded.pixels);
                    }
                    Err(error) => blocked_resources
                        .push(format!("{} (image decode failed: {error})", candidate.url)),
                },
                Err(NetworkError::Blocked { url, reason }) => {
                    blocked_resources.push(format!("{url} ({reason})"));
                }
                Err(error) => blocked_resources.push(format!("{} ({error})", candidate.url)),
            }
        }
        images
    }
}

pub(crate) fn embedded_css(document: &Document) -> String {
    document
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(id, node)| match &node.kind {
            NodeKind::Element { tag, .. } if tag == "style" => Some(document.text_content(id)),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::DisplayItemKind;

    #[test]
    fn creates_styled_display_output_from_html() {
        let engine = Engine::new().with_viewport_width(480);
        let output = engine.load_html(
            "<style>h1{color:#ff0000;font-size:24px}</style><article><h1>Glide</h1><p>Independent engine.</p></article>",
        );
        let glide = output
            .display_list
            .iter()
            .find(|item| item.kind == DisplayItemKind::Text && item.text == "Glide")
            .unwrap();
        assert_eq!(glide.color, [255, 0, 0, 255]);
        assert_eq!(glide.font_size, 24);
    }

    #[test]
    fn effective_base_url_drives_relative_resource_planning() {
        let document = html::parse(
            r#"<base href="https://cdn.example/assets/"><img src="hero.png">"#,
        );
        let page = Url::parse("https://app.example/page").unwrap();
        let base = effective_base_url(&document, &page);
        let plan = ResourcePlan::discover(&document, &base);

        assert_eq!(base.as_str(), "https://cdn.example/assets/");
        assert_eq!(
            plan.candidates[0].url.as_str(),
            "https://cdn.example/assets/hero.png"
        );
    }

    #[test]
    fn font_system_is_wired_into_engine_and_font_privacy_gate() {
        let engine = Engine::new();
        assert!(engine.font_face_count() > 0);

        let decision = engine.evaluate_request(&RequestContext {
            top_level_host: "app.example".into(),
            request_host: "fonts.tracker.invalid".into(),
            kind: ResourceKind::Font,
        });
        assert!(matches!(decision, PrivacyDecision::Block { .. }));
    }
}
