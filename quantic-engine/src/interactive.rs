use std::collections::{BTreeMap, BTreeSet};

use url::{form_urlencoded, Url};

use crate::{
    css,
    dom::{Document, NodeKind},
    engine::{embedded_css, Engine, EngineOutput},
    html,
    js_runtime::{
        BoaRuntime, ConsoleMessage, DomMutation, FetchResolution, JsRuntime, JsRuntimeError,
        RuntimeEffects,
    },
    network::{self, NetworkError},
    paint::DecodedImages,
    privacy::ResourceKind,
    resources::ResourcePlan,
    storage::origin_key,
    style,
    url_context::effective_base_url,
    websocket::WebSocketHub,
};

const MAX_EFFECT_ROUNDS: usize = 24;
const MAX_FETCHES_PER_PUMP: usize = 128;
const MAX_MODULE_GRAPH_DEPTH: usize = 16;
const MAX_MODULE_GRAPH_MODULES: usize = 128;

#[derive(Debug)]
pub enum PageError {
    InvalidUrl(url::ParseError),
    Network(NetworkError),
    Javascript(JsRuntimeError),
    StoragePoisoned,
    Protocol(String),
    FormNotFound(String),
}

impl std::fmt::Display for PageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUrl(error) => write!(formatter, "invalid URL: {error}"),
            Self::Network(error) => write!(formatter, "network error: {error}"),
            Self::Javascript(error) => write!(formatter, "{error}"),
            Self::StoragePoisoned => write!(formatter, "partitioned storage lock is poisoned"),
            Self::Protocol(error) => write!(formatter, "interactive page protocol error: {error}"),
            Self::FormNotFound(id) => write!(formatter, "form not found: {id}"),
        }
    }
}

impl std::error::Error for PageError {}

impl From<JsRuntimeError> for PageError {
    fn from(error: JsRuntimeError) -> Self {
        Self::Javascript(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormField {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormSubmission {
    pub action: String,
    pub method: String,
    pub fields: Vec<FormField>,
    pub prevented: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationRequest {
    pub url: String,
    pub method: String,
    pub body: Option<String>,
    pub content_type: Option<String>,
    pub replace: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptSchedule {
    Blocking,
    Async,
    Defer,
}

#[derive(Debug, Clone)]
struct ScriptDescriptor {
    src: Option<String>,
    source: String,
    schedule: ScriptSchedule,
    module: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryUpdate {
    pub url: String,
    pub replace: bool,
    pub delta: Option<i32>,
}

pub struct InteractivePage {
    engine: Engine,
    document: Document,
    external_css: String,
    runtime: BoaRuntime,
    initial_document_url: Option<Url>,
    document_url: Option<Url>,
    base_url: Option<Url>,
    blocked_resources: Vec<String>,
    console_messages: Vec<ConsoleMessage>,
    script_errors: Vec<String>,
    decoded_images: DecodedImages,
    origin: String,
    pending_navigation: Vec<NavigationRequest>,
    pending_history: Vec<HistoryUpdate>,
    websockets: WebSocketHub,
}

impl InteractivePage {
    pub(crate) fn from_html(engine: Engine, source: &str) -> Result<Self, PageError> {
        Self::build(engine, source, None, String::new(), Vec::new())
    }

    pub(crate) fn from_html_with_base(
        engine: Engine,
        source: &str,
        base_url: Url,
    ) -> Result<Self, PageError> {
        Self::build(engine, source, Some(base_url), String::new(), Vec::new())
    }

    pub(crate) fn from_url(engine: Engine, address: &str) -> Result<Self, PageError> {
        let url = network::parse_address(address).map_err(PageError::InvalidUrl)?;
        let response = engine
            .fetch_resource(&url, &url, ResourceKind::Document, "GET", None, None)
            .map_err(PageError::Network)?;
        let source = String::from_utf8_lossy(&response.body).into_owned();
        Self::from_remote_source(engine, response.final_url, source)
    }

    pub(crate) fn from_navigation(
        engine: Engine,
        request: &NavigationRequest,
    ) -> Result<Self, PageError> {
        let url = network::parse_address(&request.url).map_err(PageError::InvalidUrl)?;
        let response = engine
            .fetch_resource(
                &url,
                &url,
                ResourceKind::Document,
                &request.method,
                request.body.as_deref(),
                request.content_type.as_deref(),
            )
            .map_err(PageError::Network)?;
        let source = String::from_utf8_lossy(&response.body).into_owned();
        Self::from_remote_source(engine, response.final_url, source)
    }

    pub(crate) fn from_remote_source(
        engine: Engine,
        document_url: Url,
        source: String,
    ) -> Result<Self, PageError> {
        let document = html::parse(&source);
        let base_url = effective_base_url(&document, &document_url);
        let mut blocked_resources = Vec::new();
        let plan = ResourcePlan::discover(&document, &base_url);
        engine.warm_preloads(&plan, &document_url, &mut blocked_resources);
        let external_css =
            engine.load_external_styles(&plan, &document_url, &mut blocked_resources);

        Self::build(
            engine,
            &source,
            Some(document_url),
            external_css,
            blocked_resources,
        )
    }

    fn build(
        engine: Engine,
        source: &str,
        document_url: Option<Url>,
        external_css: String,
        mut blocked_resources: Vec<String>,
    ) -> Result<Self, PageError> {
        let document = html::parse(source);
        let base_url = document_url
            .as_ref()
            .map(|url| effective_base_url(&document, url));
        let origin = origin_key(document_url.as_ref());
        let local_storage = engine
            .storage
            .lock()
            .map_err(|_| PageError::StoragePoisoned)?
            .snapshot(&origin);
        let document_cookie = document_url
            .as_ref()
            .map(|url| engine.document_cookie(url, url))
            .unwrap_or_default();
        let mut css_source = embedded_css(&document);
        if !external_css.trim().is_empty() {
            css_source.push('\n');
            css_source.push_str(&external_css);
        }
        let stylesheet = css::parse_stylesheet_for_viewport(&css_source, engine.viewport_width);
        let computed_styles = style::compute_styles(&document, &stylesheet);
        let runtime = BoaRuntime::new_for_page_with_environment(
            &document,
            &computed_styles,
            &local_storage,
            document_url.as_ref(),
            &document_cookie,
            engine.viewport_width,
        )?;
        let decoded_images = match (document_url.as_ref(), base_url.as_ref()) {
            (Some(document_url), Some(base_url)) => engine.fetch_document_images(
                &document,
                document_url,
                base_url,
                &mut blocked_resources,
            ),
            _ => DecodedImages::new(),
        };

        let mut page = Self {
            engine,
            document,
            external_css,
            runtime,
            initial_document_url: document_url.clone(),
            document_url,
            base_url,
            blocked_resources,
            console_messages: Vec::new(),
            script_errors: Vec::new(),
            decoded_images,
            origin,
            pending_navigation: Vec::new(),
            pending_history: Vec::new(),
            websockets: WebSocketHub::default(),
        };
        page.execute_document_scripts()?;
        Ok(page)
    }

    pub fn snapshot(&self) -> EngineOutput {
        self.engine.render_existing_document_with_images(
            self.document.clone(),
            self.base_url.clone(),
            self.external_css.clone(),
            self.blocked_resources.clone(),
            self.console_messages.clone(),
            self.script_errors.clone(),
            self.decoded_images.clone(),
        )
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn initial_document_url(&self) -> Option<&Url> {
        self.initial_document_url.as_ref()
    }

    pub fn document_url(&self) -> Option<&Url> {
        self.document_url.as_ref()
    }

    pub fn base_url(&self) -> Option<&Url> {
        self.base_url.as_ref()
    }

    pub fn blocked_resources(&self) -> &[String] {
        &self.blocked_resources
    }

    pub fn script_errors(&self) -> &[String] {
        &self.script_errors
    }

    pub fn console_messages(&self) -> &[ConsoleMessage] {
        &self.console_messages
    }

    pub fn pending_navigations(&self) -> &[NavigationRequest] {
        &self.pending_navigation
    }

    pub fn take_navigation(&mut self) -> Option<NavigationRequest> {
        if self.pending_navigation.is_empty() {
            None
        } else {
            Some(self.pending_navigation.remove(0))
        }
    }

    pub fn take_history_update(&mut self) -> Option<HistoryUpdate> {
        if self.pending_history.is_empty() {
            None
        } else {
            Some(self.pending_history.remove(0))
        }
    }

    pub fn document_cookie(&self) -> String {
        self.document_url
            .as_ref()
            .map(|url| self.engine.document_cookie(url, url))
            .unwrap_or_default()
    }

    pub fn poll_async_events(&mut self) -> Result<usize, PageError> {
        let delivered = self.deliver_websocket_events()?;
        if delivered > 0 {
            self.pump_runtime()?;
        }
        Ok(delivered)
    }

    pub fn local_storage(&self) -> Result<BTreeMap<String, String>, PageError> {
        Ok(self
            .engine
            .storage
            .lock()
            .map_err(|_| PageError::StoragePoisoned)?
            .snapshot(&self.origin))
    }

    pub fn execute_script(&mut self, source: &str) -> Result<(), PageError> {
        if let Err(error) = self.runtime.eval_script(source) {
            self.script_errors.push(error.to_string());
            return Err(PageError::Javascript(error));
        }
        self.pump_runtime()
    }

    pub fn dispatch_event(
        &mut self,
        target_selector: &str,
        event_type: &str,
    ) -> Result<bool, PageError> {
        let allowed = self.runtime.dispatch_event(target_selector, event_type)?;
        self.pump_runtime()?;
        Ok(allowed)
    }

    pub fn click(&mut self, element_id: &str) -> Result<bool, PageError> {
        self.dispatch_event(&format!("#{element_id}"), "click")
    }

    pub fn link_navigation(
        &mut self,
        element_id: &str,
    ) -> Result<Option<NavigationRequest>, PageError> {
        let node_id = self
            .document
            .find_by_id(element_id)
            .filter(|id| self.document.element_tag(*id) == Some("a"))
            .ok_or_else(|| PageError::Protocol(format!("link not found: {element_id}")))?;

        let allowed = self.click(element_id)?;
        if !allowed {
            return Ok(None);
        }

        let Some(href) = self.document.attribute(node_id, "href") else {
            return Ok(None);
        };
        if href.trim().is_empty() || href.trim().starts_with("javascript:") {
            return Ok(None);
        }

        let url = match self.base_url.as_ref() {
            Some(base) => base
                .join(href)
                .map(|url| url.to_string())
                .unwrap_or_else(|_| href.to_string()),
            None => href.to_string(),
        };

        Ok(Some(NavigationRequest {
            url,
            method: "GET".to_string(),
            body: None,
            content_type: None,
            replace: false,
        }))
    }

    pub fn set_value(&mut self, element_id: &str, value: &str) -> Result<(), PageError> {
        let node_id = self
            .document
            .find_by_id(element_id)
            .ok_or_else(|| PageError::Protocol(format!("element not found: {element_id}")))?;
        self.document.set_attribute(node_id, "value", value);
        self.runtime.host_set_value(element_id, value)?;
        self.dispatch_event(&format!("#{element_id}"), "input")?;
        self.dispatch_event(&format!("#{element_id}"), "change")?;
        Ok(())
    }

    pub fn submit_form(&mut self, form_id: &str) -> Result<FormSubmission, PageError> {
        let form = self
            .document
            .find_by_id(form_id)
            .filter(|id| self.document.element_tag(*id) == Some("form"))
            .ok_or_else(|| PageError::FormNotFound(form_id.to_string()))?;

        let allowed = self.dispatch_event(&format!("#{form_id}"), "submit")?;
        let method = self
            .document
            .attribute(form, "method")
            .unwrap_or("GET")
            .to_ascii_uppercase();
        let raw_action = self.document.attribute(form, "action").unwrap_or("");
        let action = match &self.base_url {
            Some(base) if raw_action.is_empty() => base.to_string(),
            Some(base) => base
                .join(raw_action)
                .map(|url| url.to_string())
                .unwrap_or_else(|_| raw_action.to_string()),
            None => raw_action.to_string(),
        };

        let fields = self
            .document
            .descendants(form)
            .into_iter()
            .filter_map(|id| form_field(&self.document, id))
            .collect();

        Ok(FormSubmission {
            action,
            method,
            fields,
            prevented: !allowed,
        })
    }

    pub fn form_navigation(
        &mut self,
        form_id: &str,
    ) -> Result<Option<NavigationRequest>, PageError> {
        let submission = self.submit_form(form_id)?;
        if submission.prevented {
            return Ok(None);
        }

        let encoded = encode_form_fields(&submission.fields);
        if submission.method == "GET" {
            let url = append_query(&submission.action, &submission.fields);
            return Ok(Some(NavigationRequest {
                url,
                method: "GET".to_string(),
                body: None,
                content_type: None,
                replace: false,
            }));
        }

        Ok(Some(NavigationRequest {
            url: submission.action,
            method: submission.method,
            body: Some(encoded),
            content_type: Some("application/x-www-form-urlencoded".to_string()),
            replace: false,
        }))
    }

    fn execute_document_scripts(&mut self) -> Result<(), PageError> {
        let scripts = script_descriptors(&self.document);
        for schedule in [
            ScriptSchedule::Blocking,
            ScriptSchedule::Async,
            ScriptSchedule::Defer,
        ] {
            for script in scripts.iter().filter(|script| script.schedule == schedule) {
                self.execute_script_descriptor(script)?;
            }
        }

        let _ = self
            .runtime
            .dispatch_event("document", "DOMContentLoaded")?;
        self.pump_runtime()?;
        let _ = self.runtime.dispatch_event("window", "load")?;
        self.pump_runtime()
    }

    fn execute_script_descriptor(&mut self, script: &ScriptDescriptor) -> Result<(), PageError> {
        let mut module_base_url = self.base_url.clone();
        let source = if let Some(src) = script.src.as_deref() {
            let (Some(base), Some(document_url)) =
                (self.base_url.clone(), self.document_url.clone())
            else {
                self.script_errors
                    .push(format!("external script skipped without document URL: {src}"));
                return Ok(());
            };
            let target = match network::resolve_url(&base, src) {
                Ok(target) => target,
                Err(error) => {
                    self.script_errors
                        .push(format!("invalid script URL {src}: {error}"));
                    return Ok(());
                }
            };
            match self.engine.fetch_resource(
                &document_url,
                &target,
                ResourceKind::Script,
                "GET",
                None,
                None,
            ) {
                Ok(response) => {
                    if script.module {
                        module_base_url = Some(response.final_url.clone());
                    }
                    String::from_utf8_lossy(&response.body).into_owned()
                }
                Err(NetworkError::Blocked {
                    url: blocked,
                    reason,
                }) => {
                    self.blocked_resources.push(format!("{blocked} ({reason})"));
                    self.script_errors
                        .push(format!("script blocked: {blocked} ({reason})"));
                    return Ok(());
                }
                Err(error) => {
                    self.script_errors
                        .push(format!("script load failed {target}: {error}"));
                    return Ok(());
                }
            }
        } else {
            script.source.clone()
        };

        if source.trim().is_empty() {
            return Ok(());
        }

        let evaluation = if script.module {
            if let Some(module_base_url) = module_base_url {
                match self.collect_relative_module_graph(&module_base_url, &source) {
                    Ok(dependencies) => self.runtime.eval_module_graph(
                        &source,
                        &module_virtual_path(&module_base_url),
                        &dependencies,
                    ),
                    Err(error) => {
                        self.script_errors.push(error);
                        return Ok(());
                    }
                }
            } else {
                self.runtime.eval_module(&source)
            }
        } else {
            self.runtime.eval_script(&source)
        };
        if let Err(error) = evaluation {
            self.script_errors.push(error.to_string());
            return Ok(());
        }
        self.pump_runtime()
    }

    fn collect_relative_module_graph(
        &mut self,
        root_url: &Url,
        root_source: &str,
    ) -> Result<BTreeMap<String, String>, String> {
        let top_level = self
            .document_url
            .clone()
            .unwrap_or_else(|| root_url.clone());
        let mut dependencies = BTreeMap::new();
        let mut visited = BTreeSet::new();
        visited.insert(module_virtual_path(root_url));

        let mut pending = vec![(
            root_url.clone(),
            root_url.clone(),
            root_source.to_string(),
            0usize,
        )];

        while let Some((actual_base, loader_base, source, depth)) = pending.pop() {
            if depth >= MAX_MODULE_GRAPH_DEPTH {
                return Err(format!(
                    "module graph exceeds maximum depth of {MAX_MODULE_GRAPH_DEPTH}"
                ));
            }

            for specifier in relative_module_specifiers(&source) {
                let actual_target = actual_base
                    .join(&specifier)
                    .map_err(|error| format!("invalid module specifier {specifier}: {error}"))?;
                let loader_target = loader_base.join(&specifier).map_err(|error| {
                    format!("invalid module loader specifier {specifier}: {error}")
                })?;
                let loader_path = module_virtual_path(&loader_target);

                if !visited.insert(loader_path.clone()) {
                    continue;
                }
                if dependencies.len() >= MAX_MODULE_GRAPH_MODULES {
                    return Err(format!(
                        "module graph exceeds maximum size of {MAX_MODULE_GRAPH_MODULES} modules"
                    ));
                }

                let response = match self.engine.fetch_resource(
                    &top_level,
                    &actual_target,
                    ResourceKind::Script,
                    "GET",
                    None,
                    None,
                ) {
                    Ok(response) => response,
                    Err(NetworkError::Blocked {
                        url: blocked,
                        reason,
                    }) => {
                        self.blocked_resources.push(format!("{blocked} ({reason})"));
                        return Err(format!("module blocked: {blocked} ({reason})"));
                    }
                    Err(error) => {
                        return Err(format!("module load failed {actual_target}: {error}"));
                    }
                };

                let dependency_source = String::from_utf8_lossy(&response.body).into_owned();
                dependencies.insert(loader_path, dependency_source.clone());
                pending.push((
                    response.final_url,
                    loader_target,
                    dependency_source,
                    depth + 1,
                ));
            }
        }

        Ok(dependencies)
    }

    fn pump_runtime(&mut self) -> Result<(), PageError> {
        let mut fetch_count = 0usize;
        for _ in 0..MAX_EFFECT_ROUNDS {
            let effects = self.runtime.drain_effects()?;
            let has_fetches = !effects.fetches.is_empty();
            let had_websocket_commands = !effects.websockets.is_empty();
            self.apply_effects(&effects)?;

            for request in effects.fetches {
                fetch_count += 1;
                if fetch_count > MAX_FETCHES_PER_PUMP {
                    return Err(PageError::Protocol(
                        "fetch limit exceeded while settling page".to_string(),
                    ));
                }
                let resolution = self.resolve_fetch_request(request);
                self.runtime.resolve_fetch(&resolution)?;
            }

            let websocket_events = self.deliver_websocket_events()?;
            if !has_fetches && !had_websocket_commands && websocket_events == 0 {
                return Ok(());
            }
        }

        Err(PageError::Protocol(
            "effect loop did not settle within the safety budget".to_string(),
        ))
    }

    fn apply_effects(&mut self, effects: &RuntimeEffects) -> Result<(), PageError> {
        let refresh_images = effects.mutations.iter().any(mutation_may_change_images);

        // JavaScript allocates dynamic node IDs against its current DOM snapshot.
        // Reserve those IDs first so host-side textContent updates cannot consume
        // an ID that a later createElement/createText mutation already owns.
        for mutation in &effects.mutations {
            if is_dom_creation(mutation) {
                apply_dom_mutation(&mut self.document, mutation)?;
            }
        }
        for mutation in &effects.mutations {
            if !is_dom_creation(mutation) {
                apply_dom_mutation(&mut self.document, mutation)?;
            }
        }

        if !effects.mutations.is_empty() {
            self.runtime.sync_next_node_id(self.document.nodes.len())?;
        }
        if refresh_images {
            self.refresh_images();
        }
        if !effects.storage.is_empty() {
            self.engine
                .storage
                .lock()
                .map_err(|_| PageError::StoragePoisoned)?
                .apply(&self.origin, &effects.storage);
        }

        if let Some(page_url) = self.document_url.as_ref() {
            for cookie in &effects.cookies {
                self.engine
                    .set_document_cookie(page_url, page_url, &cookie.value);
            }
        }

        for history in &effects.history {
            let (Some(document_url), Some(base_url)) =
                (self.document_url.as_ref(), self.base_url.as_ref())
            else {
                continue;
            };
            let Ok(target) = base_url.join(&history.url) else {
                self.script_errors
                    .push(format!("history URL rejected: {}", history.url));
                continue;
            };
            if target.origin() != document_url.origin() {
                self.script_errors.push(format!(
                    "history cross-origin URL rejected: {}",
                    history.url
                ));
                continue;
            }

            self.document_url = Some(target.clone());
            self.base_url = Some(effective_base_url(&self.document, &target));
            self.pending_history.push(HistoryUpdate {
                url: target.to_string(),
                replace: history.replace,
                delta: history.delta,
            });
        }

        for navigation in &effects.navigation {
            let url = match self.base_url.as_ref() {
                Some(base) => base
                    .join(&navigation.url)
                    .map(|url| url.to_string())
                    .unwrap_or_else(|_| navigation.url.clone()),
                None => navigation.url.clone(),
            };
            self.pending_navigation.push(NavigationRequest {
                url,
                method: "GET".to_string(),
                body: None,
                content_type: None,
                replace: navigation.replace,
            });
        }

        for request in &effects.websockets {
            let socket_id = request.socket_id;
            let Some(top_level) = self.document_url.as_ref() else {
                self.runtime
                    .resolve_websocket_event(&crate::js_runtime::WebSocketEvent::error(
                        socket_id,
                        "WebSocket is disabled for local documents without a base URL".to_string(),
                    ))?;
                self.runtime
                    .resolve_websocket_event(&crate::js_runtime::WebSocketEvent::close(
                        socket_id,
                        1006,
                        "no network origin".to_string(),
                        false,
                    ))?;
                continue;
            };

            let cookie_header = request
                .url
                .as_deref()
                .and_then(|raw| Url::parse(raw).ok())
                .and_then(|target| {
                    self.engine
                        .cookies
                        .lock()
                        .ok()
                        .and_then(|jar| jar.request_header(top_level, &target))
                });

            if let Err(error) = self.websockets.handle_request(
                &self.engine.privacy,
                top_level,
                cookie_header.as_deref(),
                request.clone(),
            ) {
                if let Some(url) = request.url.as_deref() {
                    self.blocked_resources
                        .push(format!("{url} (WebSocket: {error})"));
                }
                self.runtime
                    .resolve_websocket_event(&crate::js_runtime::WebSocketEvent::error(
                        socket_id, error,
                    ))?;
                self.runtime
                    .resolve_websocket_event(&crate::js_runtime::WebSocketEvent::close(
                        socket_id,
                        1006,
                        "WebSocket host rejected request".to_string(),
                        false,
                    ))?;
            }
        }

        self.console_messages
            .extend(effects.console.iter().cloned());
        Ok(())
    }

    fn deliver_websocket_events(&mut self) -> Result<usize, PageError> {
        let events = self.websockets.drain_events();
        let count = events.len();
        for event in events {
            self.runtime.resolve_websocket_event(&event)?;
        }
        Ok(count)
    }

    fn refresh_images(&mut self) {
        let (Some(document_url), Some(base_url)) =
            (self.document_url.as_ref(), self.base_url.as_ref())
        else {
            self.decoded_images.clear();
            return;
        };
        self.decoded_images = self.engine.fetch_document_images(
            &self.document,
            document_url,
            base_url,
            &mut self.blocked_resources,
        );
        self.blocked_resources.sort();
        self.blocked_resources.dedup();
    }

    fn resolve_fetch_request(
        &mut self,
        request: crate::js_runtime::FetchRequest,
    ) -> FetchResolution {
        let (Some(base), Some(document_url)) =
            (self.base_url.as_ref(), self.document_url.as_ref())
        else {
            return FetchResolution {
                id: request.id,
                url: request.url,
                status: 0,
                body: String::new(),
                content_type: None,
                error: Some(
                    "network fetch is disabled for local documents without a base URL".into(),
                ),
            };
        };

        let target = match network::resolve_url(base, &request.url) {
            Ok(target) => target,
            Err(error) => {
                return FetchResolution {
                    id: request.id,
                    url: request.url,
                    status: 0,
                    body: String::new(),
                    content_type: None,
                    error: Some(error.to_string()),
                }
            }
        };

        let content_type = request
            .headers
            .get("content-type")
            .map(String::as_str);
        match self.engine.fetch_resource_with_headers(
            document_url,
            &target,
            ResourceKind::Fetch,
            &request.method,
            request.body.as_deref(),
            content_type,
            &request.headers,
        ) {
            Ok(response) => FetchResolution {
                id: request.id,
                url: response.final_url.to_string(),
                status: response.status,
                body: String::from_utf8_lossy(&response.body).into_owned(),
                content_type: response.content_type,
                error: None,
            },
            Err(NetworkError::Blocked { url, reason }) => {
                self.blocked_resources.push(format!("{url} ({reason})"));
                FetchResolution {
                    id: request.id,
                    url,
                    status: 0,
                    body: String::new(),
                    content_type: None,
                    error: Some(format!("blocked by Quantic Privacy Core: {reason}")),
                }
            }
            Err(error) => FetchResolution {
                id: request.id,
                url: target.to_string(),
                status: 0,
                body: String::new(),
                content_type: None,
                error: Some(error.to_string()),
            },
        }
    }
}

fn module_virtual_path(url: &Url) -> String {
    let authority = match (url.host_str(), url.port()) {
        (Some(host), Some(port)) => format!("{host}_{port}"),
        (Some(host), None) => host.to_string(),
        (None, _) => "local".to_string(),
    };
    let authority = authority.replace(':', "_");
    let mut path = url.path().trim_start_matches('/').to_string();
    if path.is_empty() || path.ends_with('/') {
        path.push_str("__inline_module__.js");
    }
    format!(
        "quantic_modules/{}/{authority}/{path}",
        url.scheme().replace(':', "_")
    )
}

fn relative_module_specifiers(source: &str) -> Vec<String> {
    let mut specifiers = Vec::new();

    for statement in source.split(';') {
        let statement = strip_leading_js_comments(statement);
        let normalized = statement
            .replace('\n', " ")
            .replace('\r', " ")
            .replace('\t', " ");
        let statement = normalized.trim();

        let candidate = if let Some(rest) = statement.strip_prefix("import ") {
            if let Some(after_from) = after_keyword(rest, "from") {
                first_quoted_literal(after_from)
            } else {
                first_quoted_literal(rest)
            }
        } else if let Some(rest) = statement.strip_prefix("export ") {
            after_keyword(rest, "from").and_then(first_quoted_literal)
        } else {
            None
        };

        if let Some(specifier) = candidate {
            if (specifier.starts_with("./") || specifier.starts_with("../"))
                && !specifiers.contains(&specifier)
            {
                specifiers.push(specifier);
            }
        }
    }

    specifiers
}

fn strip_leading_js_comments(mut input: &str) -> &str {
    loop {
        input = input.trim_start();
        if let Some(rest) = input.strip_prefix("//") {
            let Some(newline) = rest.find('\n') else {
                return "";
            };
            input = &rest[newline + 1..];
            continue;
        }
        if let Some(rest) = input.strip_prefix("/*") {
            let Some(end) = rest.find("*/") else {
                return "";
            };
            input = &rest[end + 2..];
            continue;
        }
        return input;
    }
}

fn after_keyword<'a>(input: &'a str, keyword: &str) -> Option<&'a str> {
    for (offset, _) in input.match_indices(keyword) {
        let before = input[..offset].chars().next_back();
        let after = input[offset + keyword.len()..].chars().next();
        let before_boundary = before.is_none_or(|ch| !is_identifier_char(ch));
        let after_boundary = after.is_none_or(|ch| !is_identifier_char(ch));
        if before_boundary && after_boundary {
            return Some(&input[offset + keyword.len()..]);
        }
    }
    None
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == char::from(36)
}

fn first_quoted_literal(input: &str) -> Option<String> {
    let mut quote = None;
    let mut escaped = false;
    let mut value = String::new();

    for ch in input.chars() {
        if let Some(active_quote) = quote {
            if escaped {
                value.push(ch);
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                return Some(value);
            } else {
                value.push(ch);
            }
        } else if ch == '\'' || ch == '"' {
            quote = Some(ch);
        }
    }

    None
}

fn script_descriptors(document: &Document) -> Vec<ScriptDescriptor> {
    document
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(id, node)| match &node.kind {
            NodeKind::Element { tag, .. } if tag == "script" && supported_script(document, id) => {
                let src = document.attribute(id, "src").map(str::to_string);
                let module = document
                    .attribute(id, "type")
                    .is_some_and(|value| value.trim().eq_ignore_ascii_case("module"));
                let schedule = if document.attribute(id, "async").is_some() {
                    ScriptSchedule::Async
                } else if module || (src.is_some() && document.attribute(id, "defer").is_some()) {
                    ScriptSchedule::Defer
                } else {
                    ScriptSchedule::Blocking
                };
                Some(ScriptDescriptor {
                    src,
                    source: document.text_content(id),
                    schedule,
                    module,
                })
            }
            _ => None,
        })
        .collect()
}

fn supported_script(document: &Document, id: usize) -> bool {
    match document.attribute(id, "type").unwrap_or("").trim() {
        "" | "text/javascript" | "application/javascript" | "module" => true,
        _ => false,
    }
}

fn is_dom_creation(mutation: &DomMutation) -> bool {
    matches!(
        mutation,
        DomMutation::CreateElement { .. }
            | DomMutation::CreateText { .. }
            | DomMutation::SetText {
                text_node_id: Some(_),
                ..
            }
    )
}

fn apply_dom_mutation(document: &mut Document, mutation: &DomMutation) -> Result<(), PageError> {
    match mutation {
        DomMutation::SetText {
            node_id,
            value,
            text_node_id,
        } => {
            let old_children = document
                .nodes
                .get(*node_id)
                .map(|node| node.children.clone())
                .unwrap_or_default();
            for child in old_children {
                document.detach(child);
            }
            if let Some(text_node_id) = text_node_id {
                document
                    .create_detached_expected(*text_node_id, NodeKind::Text(value.clone()))
                    .map_err(|error| PageError::Protocol(error.to_string()))?;
                if !document.attach(*node_id, *text_node_id) {
                    return Err(PageError::Protocol(
                        "cannot attach dynamic textContent node".into(),
                    ));
                }
            }
        }
        DomMutation::SetTextNode { node_id, value } => {
            document.set_text_node(*node_id, value.clone());
        }
        DomMutation::SetAttribute {
            node_id,
            name,
            value,
        } => {
            document.set_attribute(*node_id, name, value.clone());
        }
        DomMutation::RemoveAttribute { node_id, name } => {
            document.remove_attribute(*node_id, name);
        }
        DomMutation::SetStyle {
            node_id,
            name,
            value,
        } => {
            document.set_style_property(*node_id, name, value);
        }
        DomMutation::CreateElement { node_id, tag } => {
            document
                .create_detached_expected(
                    *node_id,
                    NodeKind::Element {
                        tag: tag.to_ascii_lowercase(),
                        attributes: Vec::new(),
                    },
                )
                .map_err(|error| PageError::Protocol(error.to_string()))?;
        }
        DomMutation::CreateText { node_id, value } => {
            document
                .create_detached_expected(*node_id, NodeKind::Text(value.clone()))
                .map_err(|error| PageError::Protocol(error.to_string()))?;
        }
        DomMutation::Append { parent_id, node_id } => {
            if !document.attach(*parent_id, *node_id) {
                return Err(PageError::Protocol("cannot append dynamic DOM node".into()));
            }
        }
        DomMutation::InsertBefore {
            parent_id,
            node_id,
            before_id,
        } => {
            if !document.insert_before(*parent_id, *node_id, *before_id) {
                return Err(PageError::Protocol(
                    "cannot insert dynamic DOM node before reference".into(),
                ));
            }
        }
        DomMutation::Detach { node_id } => {
            document.detach(*node_id);
        }
    }
    Ok(())
}

fn encode_form_fields(fields: &[FormField]) -> String {
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    for field in fields {
        serializer.append_pair(&field.name, &field.value);
    }
    serializer.finish()
}

fn append_query(action: &str, fields: &[FormField]) -> String {
    if let Ok(mut url) = Url::parse(action) {
        {
            let mut query = url.query_pairs_mut();
            for field in fields {
                query.append_pair(&field.name, &field.value);
            }
        }
        return url.to_string();
    }

    let encoded = encode_form_fields(fields);
    if encoded.is_empty() {
        action.to_string()
    } else if action.contains('?') {
        format!("{action}&{encoded}")
    } else {
        format!("{action}?{encoded}")
    }
}

fn mutation_may_change_images(mutation: &DomMutation) -> bool {
    match mutation {
        DomMutation::CreateElement { tag, .. } => tag.eq_ignore_ascii_case("img"),
        DomMutation::SetAttribute { name, .. } => name.eq_ignore_ascii_case("src"),
        DomMutation::RemoveAttribute { name, .. } => name.eq_ignore_ascii_case("src"),
        _ => false,
    }
}

fn form_field(document: &Document, id: usize) -> Option<FormField> {
    let tag = document.element_tag(id)?;
    if !matches!(tag, "input" | "textarea" | "select")
        || document.attribute(id, "disabled").is_some()
    {
        return None;
    }
    let name = document.attribute(id, "name")?.to_string();
    if name.is_empty() {
        return None;
    }

    let value = match tag {
        "textarea" => document.text_content(id),
        "input" => {
            let input_type = document.attribute(id, "type").unwrap_or("text");
            if matches!(input_type, "checkbox" | "radio")
                && document.attribute(id, "checked").is_none()
            {
                return None;
            }
            if matches!(input_type, "submit" | "button" | "reset" | "file") {
                return None;
            }
            document.attribute(id, "value").unwrap_or("").to_string()
        }
        "select" => selected_option_value(document, id).unwrap_or_default(),
        _ => String::new(),
    };

    Some(FormField { name, value })
}

fn selected_option_value(document: &Document, select_id: usize) -> Option<String> {
    let options = document
        .descendants(select_id)
        .into_iter()
        .filter(|id| document.element_tag(*id) == Some("option"))
        .collect::<Vec<_>>();
    let selected = options
        .iter()
        .copied()
        .find(|id| document.attribute(*id, "selected").is_some())
        .or_else(|| options.first().copied())?;
    Some(
        document
            .attribute(selected, "value")
            .map(str::to_string)
            .unwrap_or_else(|| document.text_content(selected)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_graph_discovers_multiline_relative_imports_and_exports() {
        let source = "
            // leading comment must not hide an import
            import {
                value
            } from './dep.js';
            /* block comment */ import '../side.js';
            export { helper } from './helper.js';
            import 'bare-package';
        ";

        assert_eq!(
            relative_module_specifiers(source),
            vec![
                "./dep.js".to_string(),
                "../side.js".to_string(),
                "./helper.js".to_string(),
            ]
        );
    }

    #[test]
    fn module_virtual_paths_preserve_url_directory_structure() {
        let root = Url::parse("https://app.example/assets/main.js").unwrap();
        let dependency = root.join("../shared/dep.js").unwrap();

        assert_eq!(
            module_virtual_path(&root),
            "quantic_modules/https/app.example/assets/main.js"
        );
        assert_eq!(
            module_virtual_path(&dependency),
            "quantic_modules/https/app.example/shared/dep.js"
        );
    }

    #[test]
    fn javascript_location_replace_preserves_replace_semantics() {
        let engine = Engine::new();
        let mut page = engine
            .load_interactive_html_with_base(
                "<main>replace navigation</main>",
                "https://app.example/start",
            )
            .unwrap();

        page.execute_script("location.replace('/final');").unwrap();
        let request = page.take_navigation().expect("replace navigation request");

        assert_eq!(request.url, "https://app.example/final");
        assert_eq!(request.method, "GET");
        assert!(request.replace);
    }

    #[test]
    fn module_scripts_are_deferred_and_evaluated_as_modules() {
        let engine = Engine::new();
        let page = engine
            .load_interactive_html(
                "<p id='order'></p>
                 <script>
                   document.getElementById('order').textContent='classic';
                 </script>
                 <script type='module'>
                   const node=document.getElementById('order');
                   node.textContent += '|module';
                   export const ok = true;
                 </script>",
            )
            .unwrap();

        let order = page.document().find_by_id("order").unwrap();
        assert_eq!(page.document().text_content(order), "classic|module");
        assert!(page.script_errors().is_empty());
    }

    #[test]
    fn inline_script_event_timer_and_storage_update_page() {
        let engine = Engine::new();
        let mut page = engine
            .load_interactive_html(
                "<button id='go'>Go</button><p id='out'>Old</p>
                 <script>
                   localStorage.setItem('mode','private');
                   document.getElementById('go').addEventListener('click', () => {
                     setTimeout(() => document.getElementById('out').textContent = 'Changed', 1);
                   });
                 </script>",
            )
            .unwrap();

        page.click("go").unwrap();
        assert_eq!(
            page.document()
                .find_by_id("out")
                .map(|id| page.document().text_content(id))
                .as_deref(),
            Some("Changed")
        );
        assert_eq!(
            page.local_storage()
                .unwrap()
                .get("mode")
                .map(String::as_str),
            Some("private")
        );
    }

    #[test]
    fn dynamic_dom_creation_is_applied_to_rust_dom() {
        let engine = Engine::new();
        let page = engine
            .load_interactive_html(
                "<main id='root'></main><script>
                   const p = document.createElement('p');
                   p.id = 'dynamic';
                   p.append('Hello dynamic DOM');
                   document.getElementById('root').appendChild(p);
                 </script>",
            )
            .unwrap();

        let dynamic = page.document().find_by_id("dynamic").unwrap();
        assert_eq!(page.document().text_content(dynamic), "Hello dynamic DOM");
    }

    #[test]
    fn fetch_is_rejected_by_privacy_core_before_third_party_transport() {
        let engine = Engine::new();
        let page = engine
            .load_interactive_html_with_base(
                "<p id='out'>pending</p><script>
                   fetch('https://tracker.invalid/data')
                     .catch(() => document.getElementById('out').textContent = 'blocked');
                 </script>",
                "https://example.com/page",
            )
            .unwrap();

        let out = page.document().find_by_id("out").unwrap();
        assert_eq!(page.document().text_content(out), "blocked");
        assert!(page
            .blocked_resources()
            .iter()
            .any(|entry| entry.contains("tracker.invalid")));
    }

    #[test]
    fn local_storage_does_not_leak_between_origins() {
        let engine = Engine::new();
        engine
            .load_interactive_html_with_base(
                "<script>localStorage.setItem('secret','alpha')</script>",
                "https://a.example/",
            )
            .unwrap();

        let page = engine
            .load_interactive_html_with_base(
                "<p id='out'>x</p><script>
                   document.getElementById('out').textContent = localStorage.getItem('secret') || 'empty';
                 </script>",
                "https://b.example/",
            )
            .unwrap();
        let out = page.document().find_by_id("out").unwrap();
        assert_eq!(page.document().text_content(out), "empty");
    }

    #[test]
    fn link_navigation_respects_prevent_default() {
        let engine = Engine::new();
        let mut page = engine
            .load_interactive_html_with_base(
                "<a id='go' href='/next'>Next</a><script>
                   document.getElementById('go').addEventListener('click', event => event.preventDefault());
                 </script>",
                "https://example.com/start",
            )
            .unwrap();

        assert_eq!(page.link_navigation("go").unwrap(), None);
    }

    #[test]
    fn get_form_navigation_url_encodes_current_values() {
        let engine = Engine::new();
        let mut page = engine
            .load_interactive_html_with_base(
                "<form id='search' action='/find' method='get'>
                   <input id='q' name='q' value='quantic engine'>
                 </form>",
                "https://example.com/start",
            )
            .unwrap();
        page.set_value("q", "privacy first").unwrap();

        let navigation = page.form_navigation("search").unwrap().unwrap();
        assert_eq!(navigation.method, "GET");
        assert!(navigation.url.starts_with("https://example.com/find?"));
        assert!(navigation.url.contains("q=privacy+first"));
        assert_eq!(navigation.body, None);
    }

    #[test]
    fn dynamic_image_creation_enters_render_pipeline() {
        let engine = Engine::new();
        let page = engine
            .load_interactive_html(
                "<main id='root'></main><script>
                   const img = document.createElement('img');
                   img.id = 'dynamic-image';
                   img.setAttribute('alt', 'Generated');
                   img.setAttribute('width', '48');
                   img.setAttribute('height', '24');
                   document.getElementById('root').appendChild(img);
                 </script>",
            )
            .unwrap();
        let output = page.snapshot();
        assert!(output.display_list.iter().any(|item| {
            item.kind == crate::layout::DisplayItemKind::Image
                && item.text == "Generated"
                && item.width == 48
                && item.height == 24
        }));
    }

    #[test]
    fn document_cookie_history_and_location_are_bridged_to_the_host() {
        let engine = Engine::new();
        let mut page = engine
            .load_interactive_html_with_base(
                "<p id='out'>x</p>
                 <script>
                   document.cookie = 'theme=dark; Path=/';
                   history.pushState({}, '', '/inside?x=1');
                   location.assign('/next');
                   document.getElementById('out').textContent = document.cookie;
                 </script>",
                "https://app.example/start",
            )
            .unwrap();

        assert_eq!(page.document_cookie(), "theme=dark");
        assert_eq!(
            page.base_url().map(Url::as_str),
            Some("https://app.example/inside?x=1")
        );
        let navigation = page.take_navigation().unwrap();
        assert_eq!(navigation.url, "https://app.example/next");
        let out = page.document().find_by_id("out").unwrap();
        assert_eq!(page.document().text_content(out), "theme=dark");
    }

    #[test]
    fn third_party_websocket_is_rejected_without_network_access() {
        let engine = Engine::new();
        let page = engine
            .load_interactive_html_with_base(
                "<p id='out'>pending</p>
                 <script>
                   const socket = new WebSocket('wss://tracker.invalid/events');
                   socket.onerror = () => {
                     document.getElementById('out').textContent = 'blocked';
                   };
                 </script>",
                "https://app.example/start",
            )
            .unwrap();

        let out = page.document().find_by_id("out").unwrap();
        assert_eq!(page.document().text_content(out), "blocked");
        assert!(page
            .blocked_resources()
            .iter()
            .any(|resource| resource.contains("third-party-denied-by-default")));
    }

    #[test]
    fn text_update_and_dynamic_creation_share_ids_without_collision() {
        let engine = Engine::new();
        let page = engine
            .load_interactive_html(
                "<p id='status'>Old</p><main id='root'></main>
                 <script>
                   document.getElementById('status').textContent = 'Updated';
                   const card = document.createElement('section');
                   card.id = 'dynamic-after-text';
                   card.append('Created safely');
                   document.getElementById('root').appendChild(card);
                 </script>",
            )
            .unwrap();

        let status = page.document().find_by_id("status").unwrap();
        let dynamic = page.document().find_by_id("dynamic-after-text").unwrap();
        assert_eq!(page.document().text_content(status), "Updated");
        assert_eq!(page.document().text_content(dynamic), "Created safely");
    }

    #[test]
    fn form_values_can_be_changed_and_collected() {
        let engine = Engine::new();
        let mut page = engine
            .load_interactive_html(
                "<form id='profile' method='post'>
                   <input id='name' name='name' value='Ada'>
                   <textarea name='bio'>Hello</textarea>
                 </form>",
            )
            .unwrap();

        page.set_value("name", "Grace").unwrap();
        let submission = page.submit_form("profile").unwrap();
        assert_eq!(submission.method, "POST");
        assert!(submission.fields.contains(&FormField {
            name: "name".into(),
            value: "Grace".into()
        }));
    }
}
