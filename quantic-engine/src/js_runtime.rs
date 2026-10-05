use std::{collections::BTreeMap, path::Path, rc::Rc};

use boa_engine::{
    builtins::promise::PromiseState,
    module::{MapModuleLoader, Module},
    Context, Source,
};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    dom::{Document, NodeKind},
    storage::{CookieMutation, StorageMutation},
    style::{ComputedStyle, Display},
};

#[derive(Debug, Clone, Serialize)]
struct ComputedStyleSnapshot {
    node_id: usize,
    properties: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
struct DomNodeSnapshot {
    node_id: usize,
    kind: &'static str,
    tag: Option<String>,
    text: String,
    parent_id: Option<usize>,
    attributes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DomMutation {
    SetText {
        node_id: usize,
        value: String,
        #[serde(default)]
        text_node_id: Option<usize>,
    },
    SetTextNode {
        node_id: usize,
        value: String,
    },
    SetAttribute {
        node_id: usize,
        name: String,
        value: String,
    },
    RemoveAttribute {
        node_id: usize,
        name: String,
    },
    SetStyle {
        node_id: usize,
        name: String,
        value: String,
    },
    CreateElement {
        node_id: usize,
        tag: String,
    },
    CreateText {
        node_id: usize,
        value: String,
    },
    Append {
        parent_id: usize,
        node_id: usize,
    },
    InsertBefore {
        parent_id: usize,
        node_id: usize,
        before_id: Option<usize>,
    },
    Detach {
        node_id: usize,
    },
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct FetchRequest {
    pub id: u64,
    pub url: String,
    pub method: String,
    pub body: Option<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ConsoleMessage {
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct NavigationEffect {
    pub url: String,
    pub replace: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct HistoryEffect {
    pub url: String,
    pub replace: bool,
    #[serde(default)]
    pub delta: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct WebSocketRequest {
    pub socket_id: u64,
    pub action: String,
    pub url: Option<String>,
    #[serde(default)]
    pub protocols: Vec<String>,
    pub text: Option<String>,
    pub binary: Option<Vec<u8>>,
    pub code: Option<u16>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WebSocketEvent {
    pub socket_id: u64,
    pub event_type: String,
    pub protocol: Option<String>,
    pub text: Option<String>,
    pub binary: Option<Vec<u8>>,
    pub code: Option<u16>,
    pub reason: Option<String>,
    pub clean: Option<bool>,
    pub error: Option<String>,
}

impl WebSocketEvent {
    pub fn open(socket_id: u64, protocol: String) -> Self {
        Self {
            socket_id,
            event_type: "open".to_string(),
            protocol: Some(protocol),
            text: None,
            binary: None,
            code: None,
            reason: None,
            clean: None,
            error: None,
        }
    }

    pub fn text(socket_id: u64, text: String) -> Self {
        Self {
            socket_id,
            event_type: "message".to_string(),
            protocol: None,
            text: Some(text),
            binary: None,
            code: None,
            reason: None,
            clean: None,
            error: None,
        }
    }

    pub fn binary(socket_id: u64, binary: Vec<u8>) -> Self {
        Self {
            socket_id,
            event_type: "message".to_string(),
            protocol: None,
            text: None,
            binary: Some(binary),
            code: None,
            reason: None,
            clean: None,
            error: None,
        }
    }

    pub fn close(socket_id: u64, code: u16, reason: String, clean: bool) -> Self {
        Self {
            socket_id,
            event_type: "close".to_string(),
            protocol: None,
            text: None,
            binary: None,
            code: Some(code),
            reason: Some(reason),
            clean: Some(clean),
            error: None,
        }
    }

    pub fn error(socket_id: u64, error: String) -> Self {
        Self {
            socket_id,
            event_type: "error".to_string(),
            protocol: None,
            text: None,
            binary: None,
            code: None,
            reason: None,
            clean: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RuntimeEffects {
    #[serde(default)]
    pub mutations: Vec<DomMutation>,
    #[serde(default)]
    pub fetches: Vec<FetchRequest>,
    #[serde(default)]
    pub storage: Vec<StorageMutation>,
    #[serde(default)]
    pub cookies: Vec<CookieMutation>,
    #[serde(default)]
    pub navigation: Vec<NavigationEffect>,
    #[serde(default)]
    pub history: Vec<HistoryEffect>,
    #[serde(default)]
    pub websockets: Vec<WebSocketRequest>,
    #[serde(default)]
    pub console: Vec<ConsoleMessage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FetchResolution {
    pub id: u64,
    pub url: String,
    pub status: u16,
    pub body: String,
    pub content_type: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug)]
pub enum JsRuntimeError {
    Engine(String),
    Protocol(String),
    Serialization(serde_json::Error),
}

impl std::fmt::Display for JsRuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Engine(error) => write!(formatter, "JavaScript engine error: {error}"),
            Self::Protocol(error) => write!(formatter, "JavaScript host protocol error: {error}"),
            Self::Serialization(error) => {
                write!(formatter, "JavaScript serialization error: {error}")
            }
        }
    }
}

impl std::error::Error for JsRuntimeError {}

impl From<serde_json::Error> for JsRuntimeError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}

pub trait JsRuntime {
    fn eval_script(&mut self, source: &str) -> Result<(), JsRuntimeError>;
    fn dispatch_event(&mut self, target: &str, event_type: &str) -> Result<bool, JsRuntimeError>;
    fn host_set_value(&mut self, element_id: &str, value: &str) -> Result<(), JsRuntimeError>;
    fn drain_effects(&mut self) -> Result<RuntimeEffects, JsRuntimeError>;
    fn resolve_fetch(&mut self, resolution: &FetchResolution) -> Result<(), JsRuntimeError>;
    fn resolve_websocket_event(&mut self, event: &WebSocketEvent) -> Result<(), JsRuntimeError>;
    fn sync_next_node_id(&mut self, next_id: usize) -> Result<(), JsRuntimeError>;
}

pub struct BoaRuntime {
    context: Context,
    module_loader: Rc<MapModuleLoader>,
}

impl BoaRuntime {
    pub fn new(
        document: &Document,
        local_storage: &BTreeMap<String, String>,
    ) -> Result<Self, JsRuntimeError> {
        Self::new_for_page(document, local_storage, None, "")
    }

    pub fn new_for_page(
        document: &Document,
        local_storage: &BTreeMap<String, String>,
        page_url: Option<&Url>,
        document_cookie: &str,
    ) -> Result<Self, JsRuntimeError> {
        Self::new_for_page_with_viewport(
            document,
            local_storage,
            page_url,
            document_cookie,
            1024,
        )
    }

    pub fn new_for_page_with_viewport(
        document: &Document,
        local_storage: &BTreeMap<String, String>,
        page_url: Option<&Url>,
        document_cookie: &str,
        viewport_width: u32,
    ) -> Result<Self, JsRuntimeError> {
        let fallback_styles = crate::style::compute_styles(
            document,
            &crate::css::Stylesheet::default(),
        );
        Self::new_for_page_with_environment(
            document,
            &fallback_styles,
            local_storage,
            page_url,
            document_cookie,
            viewport_width,
        )
    }

    pub fn new_for_page_with_environment(
        document: &Document,
        computed_styles: &[ComputedStyle],
        local_storage: &BTreeMap<String, String>,
        page_url: Option<&Url>,
        document_cookie: &str,
        viewport_width: u32,
    ) -> Result<Self, JsRuntimeError> {
        let nodes = snapshot_document(document);
        let nodes_json = serde_json::to_string(&nodes)?;
        let style_json = serde_json::to_string(&snapshot_computed_styles(computed_styles))?;
        let storage_json = serde_json::to_string(local_storage)?;
        let url_json = serde_json::to_string(&page_url.map(Url::as_str).unwrap_or(""))?;
        let cookie_json = serde_json::to_string(document_cookie)?;
        let bootstrap = BOOTSTRAP
            .replace("__QUANTIC_NODES__", &nodes_json)
            .replace("__QUANTIC_COMPUTED_STYLES__", &style_json)
            .replace("__QUANTIC_STORAGE__", &storage_json)
            .replace("__QUANTIC_URL__", &url_json)
            .replace("__QUANTIC_COOKIE__", &cookie_json)
            .replace("__QUANTIC_VIEWPORT_WIDTH__", &viewport_width.max(1).to_string());

        let module_loader = Rc::new(MapModuleLoader::new());
        let mut context = Context::builder()
            .module_loader(module_loader.clone())
            .build()
            .map_err(|error| JsRuntimeError::Engine(error.to_string()))?;
        context
            .runtime_limits_mut()
            .set_loop_iteration_limit(5_000_000);
        context.runtime_limits_mut().set_recursion_limit(256);
        context.runtime_limits_mut().set_backtrace_limit(32);
        context
            .eval(Source::from_bytes(bootstrap.as_bytes()))
            .map_err(|error| JsRuntimeError::Engine(error.to_string()))?;
        context
            .run_jobs()
            .map_err(|error| JsRuntimeError::Engine(error.to_string()))?;

        Ok(Self {
            context,
            module_loader,
        })
    }

    pub fn eval_module(&mut self, source: &str) -> Result<(), JsRuntimeError> {
        let module = Module::parse(
            Source::from_bytes(source.as_bytes()),
            None,
            &mut self.context,
        )
        .map_err(|error| JsRuntimeError::Engine(error.to_string()))?;
        self.evaluate_module(module)
    }

    pub fn eval_module_graph(
        &mut self,
        root_source: &str,
        root_path: &str,
        dependencies: &BTreeMap<String, String>,
    ) -> Result<(), JsRuntimeError> {
        self.module_loader.clear();

        for (path, source) in dependencies {
            let source_path = Path::new(path);
            let module = Module::parse(
                Source::from_bytes(source.as_bytes()).with_path(source_path),
                None,
                &mut self.context,
            )
            .map_err(|error| JsRuntimeError::Engine(error.to_string()))?;
            self.module_loader.insert(path, module);
        }

        let root_path = Path::new(root_path);
        let module = Module::parse(
            Source::from_bytes(root_source.as_bytes()).with_path(root_path),
            None,
            &mut self.context,
        )
        .map_err(|error| JsRuntimeError::Engine(error.to_string()))?;
        self.module_loader.insert(root_path.to_string_lossy().as_ref(), module.clone());
        self.evaluate_module(module)
    }

    fn evaluate_module(&mut self, module: Module) -> Result<(), JsRuntimeError> {
        let promise = module.load_link_evaluate(&mut self.context);
        self.context
            .run_jobs()
            .map_err(|error| JsRuntimeError::Engine(error.to_string()))?;

        if let PromiseState::Rejected(reason) = promise.state() {
            let message = reason
                .to_string(&mut self.context)
                .map(|value| value.to_std_string_escaped())
                .unwrap_or_else(|_| "module evaluation rejected".to_string());
            return Err(JsRuntimeError::Engine(message));
        }
        self.settle()
    }

    fn eval_host(&mut self, source: &str) -> Result<boa_engine::JsValue, JsRuntimeError> {
        self.context
            .eval(Source::from_bytes(source.as_bytes()))
            .map_err(|error| JsRuntimeError::Engine(error.to_string()))
    }

    fn settle(&mut self) -> Result<(), JsRuntimeError> {
        self.context
            .run_jobs()
            .map_err(|error| JsRuntimeError::Engine(error.to_string()))?;
        self.eval_host("__q_flush_timers(64);")?;
        self.context
            .run_jobs()
            .map_err(|error| JsRuntimeError::Engine(error.to_string()))?;
        Ok(())
    }

    fn eval_json<T: for<'de> Deserialize<'de>>(
        &mut self,
        expression: &str,
    ) -> Result<T, JsRuntimeError> {
        let source = format!("JSON.stringify({expression})");
        let value = self.eval_host(&source)?;
        let string = value
            .to_string(&mut self.context)
            .map_err(|error| JsRuntimeError::Engine(error.to_string()))?
            .to_std_string_escaped();
        serde_json::from_str(&string).map_err(JsRuntimeError::Serialization)
    }
}

impl JsRuntime for BoaRuntime {
    fn eval_script(&mut self, source: &str) -> Result<(), JsRuntimeError> {
        self.context
            .eval(Source::from_bytes(source.as_bytes()))
            .map_err(|error| JsRuntimeError::Engine(error.to_string()))?;
        self.settle()
    }

    fn dispatch_event(&mut self, target: &str, event_type: &str) -> Result<bool, JsRuntimeError> {
        let target = serde_json::to_string(target)?;
        let event_type = serde_json::to_string(event_type)?;
        let allowed = self.eval_json(&format!("__q_dispatch_selector({target}, {event_type})"))?;
        self.settle()?;
        Ok(allowed)
    }

    fn host_set_value(&mut self, element_id: &str, value: &str) -> Result<(), JsRuntimeError> {
        let element_id = serde_json::to_string(element_id)?;
        let value = serde_json::to_string(value)?;
        self.eval_host(&format!("__q_host_set_value({element_id}, {value});"))?;
        self.settle()
    }

    fn drain_effects(&mut self) -> Result<RuntimeEffects, JsRuntimeError> {
        self.eval_json("__q_drain_effects()")
    }

    fn resolve_fetch(&mut self, resolution: &FetchResolution) -> Result<(), JsRuntimeError> {
        let payload = serde_json::to_string(resolution)?;
        self.eval_host(&format!("__q_resolve_fetch({payload});"))?;
        self.settle()
    }

    fn resolve_websocket_event(&mut self, event: &WebSocketEvent) -> Result<(), JsRuntimeError> {
        let payload = serde_json::to_string(event)?;
        self.eval_host(&format!("__q_resolve_websocket_event({payload});"))?;
        self.settle()
    }

    fn sync_next_node_id(&mut self, next_id: usize) -> Result<(), JsRuntimeError> {
        self.eval_host(&format!("__q_sync_next_node_id({next_id});"))?;
        Ok(())
    }
}

fn snapshot_computed_styles(styles: &[ComputedStyle]) -> Vec<ComputedStyleSnapshot> {
    styles
        .iter()
        .enumerate()
        .map(|(node_id, style)| {
            let mut properties = BTreeMap::new();
            properties.insert(
                "display".to_string(),
                match style.display {
                    Display::None => "none",
                    Display::Block => "block",
                    Display::Inline => "inline",
                    Display::Flex => "flex",
                    Display::Grid => "grid",
                }
                .to_string(),
            );
            properties.insert("color".to_string(), css_color(style.color));
            properties.insert(
                "background-color".to_string(),
                style
                    .background
                    .map(css_color)
                    .unwrap_or_else(|| "rgba(0, 0, 0, 0)".to_string()),
            );
            properties.insert("font-size".to_string(), format!("{}px", style.font_size));
            properties.insert(
                "line-height".to_string(),
                format!("{}px", style.line_height()),
            );
            properties.insert("font-family".to_string(), style.font_families.join(", "));
            properties.insert("font-weight".to_string(), style.font_weight.to_string());
            properties.insert(
                "font-style".to_string(),
                if style.font_italic { "italic" } else { "normal" }.to_string(),
            );
            for (name, value) in [
                ("margin-top", style.margin_top),
                ("margin-right", style.margin_right),
                ("margin-bottom", style.margin_bottom),
                ("margin-left", style.margin_left),
                ("padding-top", style.padding_top),
                ("padding-right", style.padding_right),
                ("padding-bottom", style.padding_bottom),
                ("padding-left", style.padding_left),
                ("border-width", style.border_width),
                ("gap", style.gap),
            ] {
                properties.insert(name.to_string(), format!("{value}px"));
            }
            if let Some(color) = style.border_color {
                properties.insert("border-color".to_string(), css_color(color));
            }
            properties.insert(
                "width".to_string(),
                style
                    .width
                    .map(|value| format!("{value}px"))
                    .unwrap_or_else(|| "auto".to_string()),
            );
            properties.insert(
                "height".to_string(),
                style
                    .height
                    .map(|value| format!("{value}px"))
                    .unwrap_or_else(|| "auto".to_string()),
            );
            for (name, value) in &style.custom_properties {
                properties.insert(name.clone(), value.clone());
            }
            ComputedStyleSnapshot {
                node_id,
                properties,
            }
        })
        .collect()
}

fn css_color(color: [u8; 4]) -> String {
    if color[3] == 255 {
        format!("rgb({}, {}, {})", color[0], color[1], color[2])
    } else {
        let alpha = f32::from(color[3]) / 255.0;
        format!("rgba({}, {}, {}, {:.3})", color[0], color[1], color[2], alpha)
    }
}

fn snapshot_document(document: &Document) -> Vec<DomNodeSnapshot> {
    document
        .nodes
        .iter()
        .enumerate()
        .map(|(node_id, node)| {
            let attributes = document
                .attributes(node_id)
                .unwrap_or_default()
                .iter()
                .map(|attribute| (attribute.name.clone(), attribute.value.clone()))
                .collect::<BTreeMap<_, _>>();
            match &node.kind {
                NodeKind::Document => DomNodeSnapshot {
                    node_id,
                    kind: "document",
                    tag: None,
                    text: String::new(),
                    parent_id: None,
                    attributes,
                },
                NodeKind::Element { tag, .. } => DomNodeSnapshot {
                    node_id,
                    kind: "element",
                    tag: Some(tag.clone()),
                    text: document.text_content(node_id),
                    parent_id: node.parent,
                    attributes,
                },
                NodeKind::Text(text) => DomNodeSnapshot {
                    node_id,
                    kind: "text",
                    tag: None,
                    text: text.clone(),
                    parent_id: node.parent,
                    attributes,
                },
            }
        })
        .collect()
}

const BOOTSTRAP: &str = r##"
(() => {
  const initialNodes = __QUANTIC_NODES__;
  const initialComputedStyles = __QUANTIC_COMPUTED_STYLES__;
  const initialStorage = __QUANTIC_STORAGE__;
  const initialURL = __QUANTIC_URL__;
  const initialCookie = __QUANTIC_COOKIE__;
  const initialViewportWidth = Number(__QUANTIC_VIEWPORT_WIDTH__) || 1024;
  const computedStyles = new Map(
    initialComputedStyles.map(entry => [entry.node_id, { ...(entry.properties || {}) }])
  );
  const nodes = new Map(initialNodes.map(node => [node.node_id, {
    ...node,
    attributes: { ...(node.attributes || {}) },
    children: []
  }]));
  for (const node of nodes.values()) {
    if (node.parent_id !== null && nodes.has(node.parent_id)) {
      nodes.get(node.parent_id).children.push(node.node_id);
    }
  }

  let nextNodeId = Math.max(0, ...Array.from(nodes.keys())) + 1;
  let nextFetchId = 1;
  let nextWebSocketId = 1;
  let nextTimerId = 1;
  const mutations = [];
  const fetches = [];
  const storageMutations = [];
  const cookieMutations = [];
  const navigationEffects = [];
  const historyEffects = [];
  const websocketCommands = [];
  const consoleMessages = [];
  const handlers = new Map();
  const timers = [];
  const fetchResolvers = new Map();

  function record(mutation) { mutations.push(mutation); }
  function handlerKey(target, type) { return String(target) + "::" + String(type); }
  function addHandler(target, type, fn, options = undefined) {
    if (typeof fn !== "function") return;
    const key = handlerKey(target, type);
    if (!handlers.has(key)) handlers.set(key, []);
    const once = Boolean(options && typeof options === "object" && options.once);
    handlers.get(key).push({ fn, once });
  }

  function removeHandler(target, type, fn) {
    const key = handlerKey(target, type);
    const list = handlers.get(key);
    if (!list) return;
    handlers.set(key, list.filter(entry => entry.fn !== fn));
  }

  class Event {
    constructor(type, init = {}) {
      this.type = String(type);
      this.bubbles = Boolean(init.bubbles);
      this.cancelable = Boolean(init.cancelable);
      this.defaultPrevented = false;
      this.target = null;
      this.currentTarget = null;
      this.detail = init.detail;
      this._stopped = false;
      this._immediateStopped = false;
    }
    preventDefault() { if (this.cancelable !== false) this.defaultPrevented = true; }
    stopPropagation() { this._stopped = true; }
    stopImmediatePropagation() {
      this._stopped = true;
      this._immediateStopped = true;
    }
  }
  class CustomEvent extends Event {
    constructor(type, init = {}) {
      super(type, init);
      this.detail = init.detail;
    }
  }

  function dispatchListeners(targetKey, event, currentTarget) {
    event.currentTarget = currentTarget || null;
    const key = handlerKey(targetKey, event.type);
    const list = handlers.get(key) || [];
    for (const entry of [...list]) {
      entry.fn.call(currentTarget || globalThis, event);
      if (entry.once) removeHandler(targetKey, event.type, entry.fn);
      if (event._immediateStopped) break;
    }
  }

  function dispatchKey(targetKey, type, targetObject) {
    const event = new Event(type, { cancelable: true, bubbles: true });
    event.target = targetObject || null;

    if (typeof targetKey === "number") {
      let current = nodes.get(targetKey);
      while (current) {
        dispatchListeners(current.node_id, event, wrap(current));
        if (event._stopped || !event.bubbles) break;
        current = current.parent_id === null ? null : nodes.get(current.parent_id);
      }
      if (!event._stopped && event.bubbles) {
        dispatchListeners("document", event, documentObject);
      }
      if (!event._stopped && event.bubbles) {
        dispatchListeners("window", event, globalThis);
      }
    } else {
      dispatchListeners(targetKey, event, targetObject || globalThis);
    }

    return !event.defaultPrevented;
  }

  function findNodeById(id) {
    for (const node of nodes.values()) {
      if (node.kind === "element" && node.attributes.id === id) return node;
    }
    return null;
  }

  function classTokens(node) {
    return String(node.attributes.class || "")
      .split(/\s+/)
      .map(token => token.trim())
      .filter(Boolean);
  }

  function writeClassTokens(node, tokens) {
    const value = Array.from(new Set(tokens)).join(" ");
    if (value) node.attributes.class = value;
    else delete node.attributes.class;
    record({ kind: value ? "setAttribute" : "removeAttribute", node_id: node.node_id, name: "class", ...(value ? { value } : {}) });
  }

  function classListFor(node) {
    return {
      add(...tokens) {
        const current = classTokens(node);
        writeClassTokens(node, current.concat(tokens.map(String)));
      },
      remove(...tokens) {
        const removed = new Set(tokens.map(String));
        writeClassTokens(node, classTokens(node).filter(token => !removed.has(token)));
      },
      contains(token) { return classTokens(node).includes(String(token)); },
      toggle(token, force) {
        token = String(token);
        const present = this.contains(token);
        const shouldHave = force === undefined ? !present : Boolean(force);
        if (shouldHave && !present) this.add(token);
        if (!shouldHave && present) this.remove(token);
        return shouldHave;
      },
      replace(oldToken, newToken) {
        oldToken = String(oldToken);
        newToken = String(newToken);
        const current = classTokens(node);
        const index = current.indexOf(oldToken);
        if (index < 0) return false;
        current[index] = newToken;
        writeClassTokens(node, current);
        return true;
      },
      item(index) { return classTokens(node)[Number(index)] || null; },
      get length() { return classTokens(node).length; },
      get value() { return classTokens(node).join(" "); },
      toString() { return classTokens(node).join(" "); }
    };
  }

  function dataAttributeName(property) {
    return "data-" + String(property).replace(/[A-Z]/g, letter => "-" + letter.toLowerCase());
  }

  function datasetFor(node) {
    return new Proxy({}, {
      get(_target, property) {
        if (typeof property !== "string") return undefined;
        return node.attributes[dataAttributeName(property)];
      },
      set(_target, property, value) {
        if (typeof property !== "string") return false;
        const name = dataAttributeName(property);
        node.attributes[name] = String(value);
        record({ kind: "setAttribute", node_id: node.node_id, name, value: String(value) });
        return true;
      },
      deleteProperty(_target, property) {
        if (typeof property !== "string") return false;
        const name = dataAttributeName(property);
        delete node.attributes[name];
        record({ kind: "removeAttribute", node_id: node.node_id, name });
        return true;
      }
    });
  }

  function elementChildrenOf(parent) {
    if (!parent) return [];
    return parent.children
      .map(id => nodes.get(id))
      .filter(child => child && child.kind === "element");
  }

  function elementPosition(node) {
    const parent = node.parent_id === null ? null : nodes.get(node.parent_id);
    const children = elementChildrenOf(parent);
    const index = children.findIndex(child => child.node_id === node.node_id);
    return { parent, children, index };
  }

  function previousElementSiblings(node) {
    const { children, index } = elementPosition(node);
    return index < 0 ? [] : children.slice(0, index);
  }

  function pseudoMatches(node, pseudo, argument) {
    switch (pseudo) {
      case "checked": return Object.prototype.hasOwnProperty.call(node.attributes, "checked");
      case "disabled": return Object.prototype.hasOwnProperty.call(node.attributes, "disabled");
      case "root": return String(node.tag || "").toLowerCase() === "html";
      case "empty":
        return node.children.every(id => {
          const child = nodes.get(id);
          return !child || child.kind === "text" ? !String(child?.text || "").trim() : false;
        });
      case "first-child": {
        const { children } = elementPosition(node);
        return children[0]?.node_id === node.node_id;
      }
      case "last-child": {
        const { children } = elementPosition(node);
        return children[children.length - 1]?.node_id === node.node_id;
      }
      case "first-of-type":
      case "last-of-type": {
        const { children } = elementPosition(node);
        const tag = String(node.tag || "").toLowerCase();
        const sameType = children.filter(child => String(child.tag || "").toLowerCase() === tag);
        return pseudo === "first-of-type"
          ? sameType[0]?.node_id === node.node_id
          : sameType[sameType.length - 1]?.node_id === node.node_id;
      }
      case "nth-child": {
        const { index } = elementPosition(node);
        if (index < 0) return false;
        const position = index + 1;
        const value = String(argument || "").trim().toLowerCase();
        if (value === "odd") return position % 2 === 1;
        if (value === "even") return position % 2 === 0;
        const expected = Number(value);
        return Number.isInteger(expected) && expected > 0 && position === expected;
      }
      default: return false;
    }
  }

  function attributeMatches(actual, operator, expected) {
    if (operator === null) return actual !== undefined;
    if (actual === undefined) return false;
    actual = String(actual);
    expected = String(expected ?? "");
    switch (operator) {
      case "=": return actual === expected;
      case "~=": return actual.split(/\s+/).includes(expected);
      case "|=": return actual === expected || actual.startsWith(expected + "-");
      case "^=": return actual.startsWith(expected);
      case "$=": return actual.endsWith(expected);
      case "*=": return actual.includes(expected);
      default: return false;
    }
  }

  function matchesSimple(node, selector) {
    if (!node || node.kind !== "element") return false;
    selector = String(selector || "").trim();
    if (!selector) return false;
    if (selector === "*") return true;

    const attributes = [];
    selector = selector.replace(/\[([\w-]+)(?:\s*(\^=|\$=|\*=|~=|\|=|=)\s*(["']?)(.*?)\3)?\]/g, (_all, name, operator, _quote, value) => {
      attributes.push([
        String(name).toLowerCase(),
        operator === undefined ? null : String(operator),
        value === undefined ? null : String(value)
      ]);
      return "";
    });

    const pseudos = [];
    selector = selector.replace(/:([\w-]+)(?:\(([^)]*)\))?/g, (_all, pseudo, argument) => {
      pseudos.push([String(pseudo).toLowerCase(), argument === undefined ? null : String(argument)]);
      return "";
    });

    const simple = selector.match(/^([a-zA-Z][\w-]*|\*)?(#[\w-]+)?((?:\.[\w-]+)*)$/);
    if (!simple) return false;
    const tag = simple[1];
    const id = simple[2] ? simple[2].slice(1) : null;
    const classes = simple[3] ? simple[3].split(".").filter(Boolean) : [];

    if (tag && tag !== "*" && String(node.tag || "").toLowerCase() !== tag.toLowerCase()) return false;
    if (id && node.attributes.id !== id) return false;
    const existing = classTokens(node);
    if (!classes.every(token => existing.includes(token))) return false;
    if (!attributes.every(([name, operator, expected]) =>
      attributeMatches(node.attributes[name], operator, expected)
    )) return false;
    return pseudos.every(([pseudo, argument]) => pseudoMatches(node, pseudo, argument));
  }

  function tokenizeSelector(selector) {
    const parts = [];
    const combinators = [];
    let current = "";
    let bracketDepth = 0;
    let parenDepth = 0;
    let pendingSpace = false;

    for (const character of String(selector).trim()) {
      if (character === "[") {
        bracketDepth++;
        current += character;
      } else if (character === "]") {
        bracketDepth = Math.max(0, bracketDepth - 1);
        current += character;
      } else if (character === "(") {
        parenDepth++;
        current += character;
      } else if (character === ")") {
        parenDepth = Math.max(0, parenDepth - 1);
        current += character;
      } else if ((character === ">" || character === "+" || character === "~") && bracketDepth === 0 && parenDepth === 0) {
        if (current.trim()) {
          parts.push(current.trim());
          current = "";
        }
        if (!parts.length) return null;
        const combinator = character === ">"
          ? "child"
          : (character === "+" ? "adjacent" : "generalSibling");
        if (combinators.length < parts.length) combinators.push(combinator);
        else combinators[combinators.length - 1] = combinator;
        pendingSpace = false;
      } else if (/\s/.test(character) && bracketDepth === 0 && parenDepth === 0) {
        if (current.trim()) {
          parts.push(current.trim());
          current = "";
          pendingSpace = true;
        }
      } else {
        if (pendingSpace && parts.length && combinators.length < parts.length) {
          combinators.push("descendant");
        }
        pendingSpace = false;
        current += character;
      }
    }
    if (current.trim()) parts.push(current.trim());
    while (combinators.length >= parts.length && combinators.length) combinators.pop();
    return parts.length ? { parts, combinators } : null;
  }

  function matchesComplex(node, selector) {
    const parsed = tokenizeSelector(selector);
    if (!parsed || !matchesSimple(node, parsed.parts[parsed.parts.length - 1])) return false;
    let current = node;

    for (let index = parsed.parts.length - 1; index > 0; index--) {
      const left = parsed.parts[index - 1];
      const combinator = parsed.combinators[index - 1] || "descendant";
      if (combinator === "child") {
        const parent = current.parent_id === null ? null : nodes.get(current.parent_id);
        if (!parent || !matchesSimple(parent, left)) return false;
        current = parent;
      } else if (combinator === "adjacent") {
        const siblings = previousElementSiblings(current);
        const sibling = siblings[siblings.length - 1] || null;
        if (!sibling || !matchesSimple(sibling, left)) return false;
        current = sibling;
      } else if (combinator === "generalSibling") {
        const siblings = previousElementSiblings(current);
        let found = null;
        for (let siblingIndex = siblings.length - 1; siblingIndex >= 0; siblingIndex--) {
          if (matchesSimple(siblings[siblingIndex], left)) {
            found = siblings[siblingIndex];
            break;
          }
        }
        if (!found) return false;
        current = found;
      } else {
        let parent = current.parent_id === null ? null : nodes.get(current.parent_id);
        let found = null;
        while (parent) {
          if (matchesSimple(parent, left)) {
            found = parent;
            break;
          }
          parent = parent.parent_id === null ? null : nodes.get(parent.parent_id);
        }
        if (!found) return false;
        current = found;
      }
    }
    return true;
  }

  function matches(node, selector) {
    return String(selector)
      .split(",")
      .map(part => part.trim())
      .filter(Boolean)
      .some(part => matchesComplex(node, part));
  }

  function descendants(nodeId, out = []) {
    const node = nodes.get(nodeId);
    if (!node) return out;
    for (const childId of node.children) {
      const child = nodes.get(childId);
      if (child) {
        out.push(child);
        descendants(childId, out);
      }
    }
    return out;
  }

  function query(selector, all = false) {
    const found = [];
    for (const node of nodes.values()) {
      if (matches(node, selector)) {
        found.push(wrap(node));
        if (!all) break;
      }
    }
    return all ? found : (found[0] || null);
  }

  function cssPropertyName(property) {
    property = String(property);
    if (property.startsWith("--")) return property;
    return property.replace(/[A-Z]/g, letter => "-" + letter.toLowerCase());
  }

  function inlineStyleEntries(node) {
    const entries = new Map();
    for (const declaration of String(node.attributes.style || "").split(";")) {
      const colon = declaration.indexOf(":");
      if (colon < 0) continue;
      const name = declaration.slice(0, colon).trim();
      const value = declaration.slice(colon + 1).trim();
      if (name) entries.set(name, value);
    }
    return entries;
  }

  function writeInlineStyle(node, entries) {
    const value = Array.from(entries.entries())
      .map(([name, entry]) => name + ":" + entry)
      .join(";");
    if (value) node.attributes.style = value;
    else delete node.attributes.style;
    record({
      kind: value ? "setAttribute" : "removeAttribute",
      node_id: node.node_id,
      name: "style",
      ...(value ? { value } : {})
    });
  }

  function styleProxy(node) {
    const api = {
      getPropertyValue(property) {
        return inlineStyleEntries(node).get(cssPropertyName(property)) || "";
      },
      setProperty(property, value, priority = "") {
        const name = cssPropertyName(property);
        const entries = inlineStyleEntries(node);
        let serialized = String(value);
        if (String(priority).toLowerCase() === "important") serialized += " !important";
        entries.set(name, serialized);
        writeInlineStyle(node, entries);
      },
      removeProperty(property) {
        const name = cssPropertyName(property);
        const entries = inlineStyleEntries(node);
        const previous = entries.get(name) || "";
        entries.delete(name);
        writeInlineStyle(node, entries);
        return previous;
      }
    };
    return new Proxy(api, {
      get(target, property) {
        if (property in target) return target[property];
        return target.getPropertyValue(property);
      },
      set(target, property, value) {
        target.setProperty(property, value);
        return true;
      }
    });
  }

  function computedStyleFor(node) {
    const base = { ...(computedStyles.get(node.node_id) || {}) };
    for (const [name, value] of inlineStyleEntries(node)) {
      base[name] = String(value).replace(/\s*!important\s*$/i, "").trim();
    }
    const api = {
      getPropertyValue(property) {
        return base[cssPropertyName(property)] ?? "";
      }
    };
    return new Proxy(api, {
      get(target, property) {
        if (property in target) return target[property];
        return target.getPropertyValue(property);
      },
      set() { return false; }
    });
  }

  globalThis.getComputedStyle = element => {
    if (!element || typeof element.__nodeId !== "number") {
      throw new TypeError("getComputedStyle expects an element");
    }
    const node = nodes.get(element.__nodeId);
    if (!node || node.kind !== "element") {
      throw new TypeError("getComputedStyle expects an element");
    }
    return computedStyleFor(node);
  };

  const fragmentVoidTags = new Set([
    "area","base","br","col","embed","hr","img","input","link",
    "meta","param","source","track","wbr"
  ]);

  function decodeHtmlFragmentText(value) {
    return String(value)
      .replace(/&#x([0-9a-f]+);/gi, (_all, hex) =>
        String.fromCodePoint(parseInt(hex, 16))
      )
      .replace(/&#([0-9]+);/g, (_all, decimal) =>
        String.fromCodePoint(parseInt(decimal, 10))
      )
      .replace(/&lt;/gi, "<")
      .replace(/&gt;/gi, ">")
      .replace(/&quot;/gi, '"')
      .replace(/&#39;|&apos;/gi, "'")
      .replace(/&amp;/gi, "&");
  }

  function escapeHtmlText(value) {
    return String(value)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
  }

  function escapeHtmlAttribute(value) {
    return escapeHtmlText(value).replace(/"/g, "&quot;");
  }

  function serializeNode(node) {
    if (!node) return "";
    if (node.kind === "text") return escapeHtmlText(node.text || "");
    if (node.kind !== "element") {
      return node.children.map(id => serializeNode(nodes.get(id))).join("");
    }
    const attributes = Object.entries(node.attributes)
      .map(([name, value]) => " " + name + '="' + escapeHtmlAttribute(value) + '"')
      .join("");
    const tag = String(node.tag || "").toLowerCase();
    if (fragmentVoidTags.has(tag)) return "<" + tag + attributes + ">";
    return "<" + tag + attributes + ">"
      + node.children.map(id => serializeNode(nodes.get(id))).join("")
      + "</" + tag + ">";
  }

  function parseFragmentAttributes(element, source) {
    const pattern = /([^\s=/>]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'=<>]+)))?/g;
    let match;
    while ((match = pattern.exec(source))) {
      const name = String(match[1] || "").toLowerCase();
      if (!name) continue;
      const value = decodeHtmlFragmentText(
        match[2] ?? match[3] ?? match[4] ?? ""
      );
      element.setAttribute(name, value);
    }
  }

  function parseHtmlFragment(source) {
    source = String(source);
    const roots = [];
    const stack = [];
    const tokenPattern = /<!--[\s\S]*?-->|<\/?[a-zA-Z][^>]*>|[^<]+|</g;
    let match;

    const appendParsed = child => {
      if (stack.length) stack[stack.length - 1].appendChild(child);
      else roots.push(child);
    };

    while ((match = tokenPattern.exec(source))) {
      const token = match[0];
      if (token.startsWith("<!--")) continue;
      if (token.startsWith("</")) {
        const closing = token
          .slice(2, -1)
          .trim()
          .split(/\s+/)[0]
          .toLowerCase();
        for (let index = stack.length - 1; index >= 0; index--) {
          if (String(stack[index].tagName || "").toLowerCase() === closing) {
            stack.length = index;
            break;
          }
        }
        continue;
      }
      if (token.startsWith("<") && /^<[a-zA-Z]/.test(token)) {
        const opening = token.match(/^<\s*([^\s/>]+)([\s\S]*?)(\/?)>$/);
        if (!opening) {
          appendParsed(documentObject.createTextNode(token));
          continue;
        }
        const tag = opening[1].toLowerCase();
        const element = documentObject.createElement(tag);
        parseFragmentAttributes(element, opening[2] || "");
        appendParsed(element);
        if (!opening[3] && !fragmentVoidTags.has(tag)) stack.push(element);
        continue;
      }
      appendParsed(documentObject.createTextNode(decodeHtmlFragmentText(token)));
    }
    return roots;
  }

  function wrap(node) {
    if (!node) return null;
    if (node.kind === "text") {
      return {
        __nodeId: node.node_id,
        get nodeType() { return 3; },
        get textContent() { return node.text; },
        set textContent(value) {
          node.text = String(value);
          record({ kind: "setTextNode", node_id: node.node_id, value: node.text });
        },
        remove() {
          record({ kind: "detach", node_id: node.node_id });
        }
      };
    }

    return {
      __nodeId: node.node_id,
      get nodeType() { return node.kind === "document" ? 9 : 1; },
      get tagName() { return node.tag ? String(node.tag).toUpperCase() : undefined; },
      get id() { return node.attributes.id || ""; },
      set id(value) { this.setAttribute("id", value); },
      get className() { return node.attributes.class || ""; },
      set className(value) { this.setAttribute("class", value); },
      get classList() { return classListFor(node); },
      get dataset() { return datasetFor(node); },
      get value() { return node.attributes.value || ""; },
      set value(value) {
        node.attributes.value = String(value);
        record({ kind: "setAttribute", node_id: node.node_id, name: "value", value: String(value) });
      },
      get textContent() {
        const collect = current => {
          if (!current) return "";
          if (current.kind === "text") return String(current.text || "");
          return current.children.map(id => collect(nodes.get(id))).join("");
        };
        return collect(node);
      },
      set textContent(value) {
        const text = String(value);
        for (const childId of [...node.children]) {
          const child = nodes.get(childId);
          if (child) child.parent_id = null;
        }
        node.children = [];
        node.text = "";
        let textNodeId = null;
        if (text) {
          textNodeId = nextNodeId++;
          nodes.set(textNodeId, {
            node_id: textNodeId,
            kind: "text",
            tag: null,
            text,
            parent_id: node.node_id,
            attributes: {},
            children: []
          });
          node.children.push(textNodeId);
        }
        record({
          kind: "setText",
          node_id: node.node_id,
          value: text,
          text_node_id: textNodeId
        });
      },
      get innerText() { return this.textContent; },
      set innerText(value) { this.textContent = value; },
      get innerHTML() {
        return node.children.map(id => serializeNode(nodes.get(id))).join("");
      },
      set innerHTML(value) {
        for (const childId of [...node.children]) {
          this.removeChild(wrap(nodes.get(childId)));
        }
        for (const child of parseHtmlFragment(String(value))) {
          this.appendChild(child);
        }
      },
      get style() { return styleProxy(node); },
      get parentNode() { return node.parent_id === null ? null : wrap(nodes.get(node.parent_id)); },
      get parentElement() {
        if (node.parent_id === null) return null;
        const parent = nodes.get(node.parent_id);
        return parent?.kind === "element" ? wrap(parent) : null;
      },
      get childNodes() { return node.children.map(id => wrap(nodes.get(id))).filter(Boolean); },
      get children() { return node.children.map(id => nodes.get(id)).filter(n => n?.kind === "element").map(wrap); },
      get childElementCount() { return this.children.length; },
      get firstChild() { return node.children.length ? wrap(nodes.get(node.children[0])) : null; },
      get lastChild() { return node.children.length ? wrap(nodes.get(node.children[node.children.length - 1])) : null; },
      get nextSibling() {
        if (node.parent_id === null) return null;
        const parent = nodes.get(node.parent_id);
        const index = parent?.children.indexOf(node.node_id) ?? -1;
        return index >= 0 && index + 1 < parent.children.length
          ? wrap(nodes.get(parent.children[index + 1]))
          : null;
      },
      get previousSibling() {
        if (node.parent_id === null) return null;
        const parent = nodes.get(node.parent_id);
        const index = parent?.children.indexOf(node.node_id) ?? -1;
        return index > 0 ? wrap(nodes.get(parent.children[index - 1])) : null;
      },
      get firstElementChild() { return this.children[0] || null; },
      get lastElementChild() {
        const children = this.children;
        return children.length ? children[children.length - 1] : null;
      },
      get nextElementSibling() {
        if (node.parent_id === null) return null;
        const siblings = elementChildrenOf(nodes.get(node.parent_id));
        const index = siblings.findIndex(sibling => sibling.node_id === node.node_id);
        return index >= 0 && index + 1 < siblings.length ? wrap(siblings[index + 1]) : null;
      },
      get previousElementSibling() {
        if (node.parent_id === null) return null;
        const siblings = elementChildrenOf(nodes.get(node.parent_id));
        const index = siblings.findIndex(sibling => sibling.node_id === node.node_id);
        return index > 0 ? wrap(siblings[index - 1]) : null;
      },
      getAttribute(name) {
        const key = String(name).toLowerCase();
        return Object.prototype.hasOwnProperty.call(node.attributes, key) ? node.attributes[key] : null;
      },
      setAttribute(name, value) {
        const key = String(name).toLowerCase();
        node.attributes[key] = String(value);
        record({ kind: "setAttribute", node_id: node.node_id, name: key, value: String(value) });
      },
      removeAttribute(name) {
        const key = String(name).toLowerCase();
        delete node.attributes[key];
        record({ kind: "removeAttribute", node_id: node.node_id, name: key });
      },
      hasAttribute(name) {
        return Object.prototype.hasOwnProperty.call(node.attributes, String(name).toLowerCase());
      },
      matches(selector) { return matches(node, String(selector)); },
      closest(selector) {
        let current = node;
        while (current) {
          if (matches(current, String(selector))) return wrap(current);
          current = current.parent_id === null ? null : nodes.get(current.parent_id);
        }
        return null;
      },
      contains(other) {
        if (!other || typeof other.__nodeId !== "number") return false;
        if (other.__nodeId === node.node_id) return true;
        return descendants(node.node_id).some(candidate => candidate.node_id === other.__nodeId);
      },
      addEventListener(type, fn, options) {
        addHandler(node.node_id, String(type), fn, options);
      },
      removeEventListener(type, fn) {
        removeHandler(node.node_id, String(type), fn);
      },
      dispatchEvent(event) { return dispatchKey(node.node_id, event.type, this); },
      click() { return dispatchKey(node.node_id, "click", this); },
      appendChild(child) {
        if (!child || typeof child.__nodeId !== "number") {
          throw new TypeError("appendChild expects a Quantic node");
        }
        const childNode = nodes.get(child.__nodeId);
        if (!childNode) throw new TypeError("unknown child node");
        if (childNode.parent_id !== null && nodes.has(childNode.parent_id)) {
          const oldParent = nodes.get(childNode.parent_id);
          oldParent.children = oldParent.children.filter(id => id !== childNode.node_id);
        }
        childNode.parent_id = node.node_id;
        node.children = node.children.filter(id => id !== childNode.node_id);
        node.children.push(childNode.node_id);
        record({ kind: "append", parent_id: node.node_id, node_id: childNode.node_id });
        return child;
      },
      insertBefore(child, reference) {
        if (!child || typeof child.__nodeId !== "number") {
          throw new TypeError("insertBefore expects a Quantic node");
        }
        const childNode = nodes.get(child.__nodeId);
        if (!childNode) throw new TypeError("unknown child node");
        const beforeId = reference == null ? null : reference.__nodeId;
        if (beforeId !== null && !node.children.includes(beforeId)) {
          throw new TypeError("reference node is not a child");
        }
        if (childNode.parent_id !== null && nodes.has(childNode.parent_id)) {
          const oldParent = nodes.get(childNode.parent_id);
          oldParent.children = oldParent.children.filter(id => id !== childNode.node_id);
        }
        childNode.parent_id = node.node_id;
        node.children = node.children.filter(id => id !== childNode.node_id);
        const index = beforeId === null ? -1 : node.children.indexOf(beforeId);
        if (index < 0) node.children.push(childNode.node_id);
        else node.children.splice(index, 0, childNode.node_id);
        record({
          kind: "insertBefore",
          parent_id: node.node_id,
          node_id: childNode.node_id,
          before_id: beforeId
        });
        return child;
      },
      append(...items) {
        for (const item of items) {
          const child = typeof item === "string" ? document.createTextNode(item) : item;
          this.appendChild(child);
        }
      },
      prepend(...items) {
        const reference = this.firstChild;
        for (const item of items) {
          const child = typeof item === "string" ? document.createTextNode(item) : item;
          this.insertBefore(child, reference);
        }
      },
      replaceChildren(...items) {
        for (const child of [...this.childNodes]) this.removeChild(child);
        this.append(...items);
      },
      before(...items) {
        const parent = this.parentNode;
        if (!parent) return;
        for (const item of items) {
          const child = typeof item === "string" ? document.createTextNode(item) : item;
          parent.insertBefore(child, this);
        }
      },
      after(...items) {
        const parent = this.parentNode;
        if (!parent) return;
        const siblings = parent.childNodes;
        const index = siblings.findIndex(item => item?.__nodeId === this.__nodeId);
        const reference =
          index >= 0 && index + 1 < siblings.length ? siblings[index + 1] : null;
        for (const item of items) {
          const child = typeof item === "string" ? document.createTextNode(item) : item;
          parent.insertBefore(child, reference);
        }
      },
      insertAdjacentHTML(position, source) {
        const fragment = parseHtmlFragment(String(source));
        switch (String(position).toLowerCase()) {
          case "beforebegin":
            if (!this.parentNode) return;
            for (const child of fragment) this.parentNode.insertBefore(child, this);
            break;
          case "afterbegin": {
            const reference = this.firstChild;
            for (const child of fragment) this.insertBefore(child, reference);
            break;
          }
          case "beforeend":
            for (const child of fragment) this.appendChild(child);
            break;
          case "afterend": {
            if (!this.parentNode) return;
            const parent = this.parentNode;
            const siblings = parent.childNodes;
            const index = siblings.findIndex(item => item?.__nodeId === this.__nodeId);
            const reference =
              index >= 0 && index + 1 < siblings.length ? siblings[index + 1] : null;
            for (const child of fragment) parent.insertBefore(child, reference);
            break;
          }
          default:
            throw new SyntaxError("Invalid insertAdjacentHTML position");
        }
      },
      removeChild(child) {
        if (!child || typeof child.__nodeId !== "number") return child;
        node.children = node.children.filter(id => id !== child.__nodeId);
        const childNode = nodes.get(child.__nodeId);
        if (childNode) childNode.parent_id = null;
        record({ kind: "detach", node_id: child.__nodeId });
        return child;
      },
      remove() {
        if (node.parent_id !== null && nodes.has(node.parent_id)) {
          const parent = nodes.get(node.parent_id);
          parent.children = parent.children.filter(id => id !== node.node_id);
        }
        node.parent_id = null;
        record({ kind: "detach", node_id: node.node_id });
      },
      querySelector(selector) {
        const found = descendants(node.node_id).find(candidate => matches(candidate, selector));
        return found ? wrap(found) : null;
      },
      querySelectorAll(selector) {
        return descendants(node.node_id).filter(candidate => matches(candidate, selector)).map(wrap);
      }
    };
  }

  const documentNode = nodes.get(0);
  const documentObject = wrap(documentNode);
  documentObject.getElementById = id => {
    const node = findNodeById(String(id));
    return node ? wrap(node) : null;
  };
  documentObject.querySelector = selector => query(String(selector), false);
  documentObject.querySelectorAll = selector => query(String(selector), true);
  documentObject.getElementsByTagName = tag => query(String(tag), true);
  documentObject.getElementsByClassName = className => query("." + String(className), true);
  Object.defineProperty(documentObject, "readyState", { get() { return "complete"; } });
  Object.defineProperty(documentObject, "title", {
    get() {
      const title = query("title", false);
      return title ? title.textContent : "";
    },
    set(value) {
      let title = query("title", false);
      if (!title) {
        const head = query("head", false) || query("html", false) || documentObject;
        title = documentObject.createElement("title");
        head.appendChild(title);
      }
      title.textContent = String(value);
    }
  });
  documentObject.createElement = tag => {
    const node = {
      node_id: nextNodeId++,
      kind: "element",
      tag: String(tag).toLowerCase(),
      text: "",
      parent_id: null,
      attributes: {},
      children: []
    };
    nodes.set(node.node_id, node);
    record({ kind: "createElement", node_id: node.node_id, tag: node.tag });
    return wrap(node);
  };
  documentObject.createTextNode = value => {
    const node = {
      node_id: nextNodeId++,
      kind: "text",
      tag: null,
      text: String(value),
      parent_id: null,
      attributes: {},
      children: []
    };
    nodes.set(node.node_id, node);
    record({ kind: "createText", node_id: node.node_id, value: node.text });
    return wrap(node);
  };
  documentObject.addEventListener = (type, fn, options) =>
    addHandler("document", String(type), fn, options);
  documentObject.removeEventListener = (type, fn) =>
    removeHandler("document", String(type), fn);
  documentObject.dispatchEvent = event => dispatchKey("document", event.type, documentObject);
  Object.defineProperty(documentObject, "body", {
    get() { return query("body", false); }
  });
  Object.defineProperty(documentObject, "documentElement", {
    get() { return query("html", false); }
  });

  function decodePart(value) {
    const source = String(value).replace(/\+/g, " ");
    let output = "";
    let cursor = 0;

    while (cursor < source.length) {
      if (
        source[cursor] === "%" &&
        cursor + 2 < source.length &&
        /^[0-9a-fA-F]{2}$/.test(source.slice(cursor + 1, cursor + 3))
      ) {
        let encoded = "";
        while (
          cursor < source.length &&
          source[cursor] === "%" &&
          cursor + 2 < source.length &&
          /^[0-9a-fA-F]{2}$/.test(source.slice(cursor + 1, cursor + 3))
        ) {
          encoded += source.slice(cursor, cursor + 3);
          cursor += 3;
        }
        try {
          output += decodeURIComponent(encoded);
        } catch (_error) {
          output += encoded;
        }
        continue;
      }
      output += source[cursor++];
    }

    return output;
  }

  function encodeFormPart(value) {
    return encodeURIComponent(String(value))
      .replace(/%20/g, "+")
      .replace(/[!'()~]/g, character =>
        "%" + character.charCodeAt(0).toString(16).toUpperCase().padStart(2, "0")
      );
  }

  class URLSearchParams {
    constructor(input = "", update = null) {
      this._pairs = [];
      this._update = typeof update === "function" ? update : null;
      if (input instanceof URLSearchParams) {
        this._pairs = input._pairs.map(pair => [...pair]);
      } else if (Array.isArray(input)) {
        this._pairs = input.map(pair => [String(pair[0]), String(pair[1])]);
      } else if (input && typeof input === "object" && !String(input).startsWith("?")) {
        this._pairs = Object.entries(input).map(([key, value]) => [key, String(value)]);
      } else {
        const source = String(input).replace(/^\?/, "");
        if (source) {
          for (const item of source.split("&")) {
            if (!item) continue;
            const [key, ...rest] = item.split("=");
            this._pairs.push([decodePart(key), decodePart(rest.join("="))]);
          }
        }
      }
    }
    _commit() {
      if (this._update) this._update(this.toString());
    }
    get size() { return this._pairs.length; }
    append(name, value) {
      this._pairs.push([String(name), String(value)]);
      this._commit();
    }
    delete(name, value) {
      name = String(name);
      const filterByValue = arguments.length > 1 && value !== undefined;
      this._pairs = this._pairs.filter(pair =>
        pair[0] !== name || (filterByValue && pair[1] !== String(value))
      );
      this._commit();
    }
    get(name) {
      name = String(name);
      const pair = this._pairs.find(pair => pair[0] === name);
      return pair ? pair[1] : null;
    }
    getAll(name) {
      name = String(name);
      return this._pairs.filter(pair => pair[0] === name).map(pair => pair[1]);
    }
    has(name, value) {
      name = String(name);
      const filterByValue = arguments.length > 1 && value !== undefined;
      return this._pairs.some(pair =>
        pair[0] === name && (!filterByValue || pair[1] === String(value))
      );
    }
    set(name, value) {
      name = String(name); value = String(value);
      let written = false;
      this._pairs = this._pairs.filter(pair => {
        if (pair[0] !== name) return true;
        if (!written) {
          pair[1] = value;
          written = true;
          return true;
        }
        return false;
      });
      if (!written) this._pairs.push([name, value]);
      this._commit();
    }
    sort() {
      this._pairs.sort((a, b) => a[0] < b[0] ? -1 : (a[0] > b[0] ? 1 : 0));
      this._commit();
    }
    toString() {
      return this._pairs
        .map(([key, value]) => encodeFormPart(key) + "=" + encodeFormPart(value))
        .join("&");
    }
    _iterate(kind) {
      const owner = this;
      let index = 0;
      return {
        next() {
          if (index >= owner._pairs.length) return { value: undefined, done: true };
          const pair = owner._pairs[index++];
          const value = kind === "keys" ? pair[0] : kind === "values" ? pair[1] : [...pair];
          return { value, done: false };
        },
        [Symbol.iterator]() { return this; }
      };
    }
    entries() { return this._iterate("entries"); }
    keys() { return this._iterate("keys"); }
    values() { return this._iterate("values"); }
    forEach(callback, thisArg) {
      if (typeof callback !== "function") throw new TypeError("URLSearchParams.forEach requires a callback");
      for (let index = 0; index < this._pairs.length; index++) {
        const [key, value] = this._pairs[index];
        callback.call(thisArg, value, key, this);
      }
    }
    [Symbol.iterator]() { return this.entries(); }
  }

  function normalizePath(path) {
    const parts = [];
    for (const part of String(path || "/").split("/")) {
      if (!part || part === ".") continue;
      if (part === "..") parts.pop();
      else parts.push(part);
    }
    return "/" + parts.join("/");
  }

  function parseAbsoluteUrl(value) {
    const match = String(value).match(/^([a-zA-Z][\w+.-]*:)(?:\/\/([^\/?#]*))?([^?#]*)(\?[^#]*)?(#.*)?$/);
    if (!match) return null;
    const protocol = match[1];
    const host = match[2] || "";
    const search = match[4] || "";
    const hash = match[5] || "";
    const opaque = match[2] === undefined && !(match[3] || "").startsWith("/");
    const pathname = opaque
      ? (search || hash ? match[3].replace(/ $/, "%20") : match[3].replace(/ +$/, ""))
      : normalizePath(match[3] || "/");
    const hostParts = host.split(":");
    const hostname = hostParts[0] || "";
    const port = hostParts.length > 1 ? hostParts.slice(1).join(":") : "";
    const origin = host ? protocol + "//" + host : "null";
    return { protocol, host, hostname, port, pathname, search, hash, origin, opaque };
  }

  function serializeURL(parsed, search = parsed.search, hash = parsed.hash) {
    return (parsed.opaque ? parsed.protocol : parsed.origin) + parsed.pathname + search + hash;
  }

  function resolveURL(input, base = initialURL) {
    input = String(input);
    if (/^[a-zA-Z][\w+.-]*:/.test(input)) return input;
    const parsedBase = parseAbsoluteUrl(base);
    if (!parsedBase) return input;

    if (input.startsWith("//")) return parsedBase.protocol + input;
    if (input.startsWith("#")) {
      return serializeURL(parsedBase, parsedBase.search, input);
    }
    if (input.startsWith("?")) {
      return serializeURL(parsedBase, input, "");
    }
    if (input.startsWith("/")) {
      const split = input.match(/^([^?#]*)(\?[^#]*)?(#.*)?$/);
      return parsedBase.origin + normalizePath(split[1]) + (split[2] || "") + (split[3] || "");
    }

    const split = input.match(/^([^?#]*)(\?[^#]*)?(#.*)?$/);
    const baseDir = parsedBase.pathname.endsWith("/")
      ? parsedBase.pathname
      : parsedBase.pathname.slice(0, parsedBase.pathname.lastIndexOf("/") + 1);
    return parsedBase.origin + normalizePath(baseDir + (split[1] || "")) + (split[2] || "") + (split[3] || "");
  }

  class URL {
    constructor(input, base = undefined) {
      this._searchParams = null;
      this.href = resolveURL(String(input), base === undefined ? initialURL : String(base));
    }
    get href() { return this._href; }
    set href(value) {
      const resolved = resolveURL(String(value), this._href || initialURL);
      const parsed = parseAbsoluteUrl(resolved);
      if (!parsed) throw new TypeError("Invalid URL");
      this._href = serializeURL(parsed);
      this._syncSearchParams();
    }
    _syncSearchParams() {
      if (this._searchParams) {
        this._searchParams._pairs = new URLSearchParams(this.search)._pairs;
      }
    }
    get protocol() { return parseAbsoluteUrl(this._href).protocol; }
    get origin() { return parseAbsoluteUrl(this._href).origin; }
    get host() { return parseAbsoluteUrl(this._href).host; }
    get hostname() { return parseAbsoluteUrl(this._href).hostname; }
    get port() { return parseAbsoluteUrl(this._href).port; }
    get pathname() { return parseAbsoluteUrl(this._href).pathname; }
    get search() { return parseAbsoluteUrl(this._href).search; }
    set search(value) {
      const parsed = parseAbsoluteUrl(this._href);
      const search = String(value) ? (String(value).startsWith("?") ? String(value) : "?" + String(value)) : "";
      this._href = serializeURL(parsed, search);
      this._syncSearchParams();
    }
    get hash() { return parseAbsoluteUrl(this._href).hash; }
    set hash(value) {
      const parsed = parseAbsoluteUrl(this._href);
      const hash = String(value) ? (String(value).startsWith("#") ? String(value) : "#" + String(value)) : "";
      this._href = serializeURL(parsed, parsed.search, hash);
    }
    get searchParams() {
      if (!this._searchParams) {
        this._searchParams = new URLSearchParams(this.search, serialized => {
          const parsed = parseAbsoluteUrl(this._href);
          this._href = serializeURL(parsed, serialized ? "?" + serialized : "");
        });
      }
      return this._searchParams;
    }
    toString() { return this.href; }
    toJSON() { return this.href; }
  }

  let currentURL = initialURL || "quantic://local/";
  const sameDocumentHistory = [currentURL];
  let historyIndex = 0;
  function setCurrentURL(value) {
    currentURL = resolveURL(value, currentURL);
    return currentURL;
  }

  const locationObject = {
    get href() { return currentURL; },
    set href(value) {
      const resolved = resolveURL(value, currentURL);
      navigationEffects.push({ url: resolved, replace: false });
      currentURL = resolved;
    },
    get origin() { return parseAbsoluteUrl(currentURL)?.origin || "null"; },
    get protocol() { return parseAbsoluteUrl(currentURL)?.protocol || ""; },
    get host() { return parseAbsoluteUrl(currentURL)?.host || ""; },
    get hostname() { return parseAbsoluteUrl(currentURL)?.hostname || ""; },
    get port() { return parseAbsoluteUrl(currentURL)?.port || ""; },
    get pathname() { return parseAbsoluteUrl(currentURL)?.pathname || ""; },
    get search() { return parseAbsoluteUrl(currentURL)?.search || ""; },
    get hash() { return parseAbsoluteUrl(currentURL)?.hash || ""; },
    assign(value) {
      const resolved = resolveURL(value, currentURL);
      navigationEffects.push({ url: resolved, replace: false });
      currentURL = resolved;
    },
    replace(value) {
      const resolved = resolveURL(value, currentURL);
      navigationEffects.push({ url: resolved, replace: true });
      currentURL = resolved;
    },
    reload() { navigationEffects.push({ url: currentURL, replace: true }); },
    toString() { return currentURL; }
  };

  const historyObject = {
    get length() { return sameDocumentHistory.length; },
    get state() { return null; },
    pushState(_state, _unused, url = null) {
      if (url === null || url === undefined) return;
      const resolved = resolveURL(url, currentURL);
      sameDocumentHistory.splice(historyIndex + 1);
      sameDocumentHistory.push(resolved);
      historyIndex = sameDocumentHistory.length - 1;
      currentURL = resolved;
      historyEffects.push({ url: resolved, replace: false, delta: null });
    },
    replaceState(_state, _unused, url = null) {
      if (url === null || url === undefined) return;
      const resolved = resolveURL(url, currentURL);
      sameDocumentHistory[historyIndex] = resolved;
      currentURL = resolved;
      historyEffects.push({ url: resolved, replace: true, delta: null });
    },
    go(delta = 0) {
      const requestedDelta = Math.trunc(Number(delta || 0));
      const next = Math.max(
        0,
        Math.min(sameDocumentHistory.length - 1, historyIndex + requestedDelta)
      );
      if (next === historyIndex) return;
      const appliedDelta = next - historyIndex;
      historyIndex = next;
      currentURL = sameDocumentHistory[historyIndex];
      historyEffects.push({
        url: currentURL,
        replace: true,
        delta: appliedDelta
      });
      dispatchKey("window", "popstate", globalThis);
    },
    back() { this.go(-1); },
    forward() { this.go(1); }
  };

  const cookieState = new Map();
  for (const entry of String(initialCookie || "").split(";")) {
    const [name, ...rest] = entry.trim().split("=");
    if (name) cookieState.set(name, rest.join("="));
  }
  function cookieString() {
    return Array.from(cookieState.entries()).map(([name, value]) => name + "=" + value).join("; ");
  }
  Object.defineProperty(documentObject, "cookie", {
    get() { return cookieString(); },
    set(value) {
      value = String(value);
      const pair = value.split(";")[0];
      const [name, ...rest] = pair.split("=");
      if (!name || !name.trim()) return;
      const cookieName = name.trim();
      const cookieValue = rest.join("=").trim();
      if (/max-age\s*=\s*0/i.test(value)) cookieState.delete(cookieName);
      else cookieState.set(cookieName, cookieValue);
      cookieMutations.push({ value });
    }
  });

  globalThis.URL = URL;
  globalThis.URLSearchParams = URLSearchParams;
  globalThis.location = locationObject;
  globalThis.history = historyObject;

  globalThis.window = globalThis;
  globalThis.document = documentObject;
  globalThis.Event = Event;
  globalThis.CustomEvent = CustomEvent;
  globalThis.addEventListener = (type, fn) => addHandler("window", String(type), fn);
  globalThis.removeEventListener = () => {};
  globalThis.dispatchEvent = event => dispatchKey("window", event.type, globalThis);

  class TextEncoder {
    get encoding() { return "utf-8"; }
    encode(input = "") {
      const bytes = [];
      for (const character of String(input)) {
        let code = character.codePointAt(0);
        if (code >= 0xd800 && code <= 0xdfff) code = 0xfffd;
        if (code <= 0x7f) {
          bytes.push(code);
        } else if (code <= 0x7ff) {
          bytes.push(0xc0 | (code >> 6), 0x80 | (code & 0x3f));
        } else if (code <= 0xffff) {
          bytes.push(
            0xe0 | (code >> 12),
            0x80 | ((code >> 6) & 0x3f),
            0x80 | (code & 0x3f)
          );
        } else {
          bytes.push(
            0xf0 | (code >> 18),
            0x80 | ((code >> 12) & 0x3f),
            0x80 | ((code >> 6) & 0x3f),
            0x80 | (code & 0x3f)
          );
        }
      }
      return new Uint8Array(bytes);
    }
    encodeInto(input, destination) {
      const encoded = this.encode(input);
      const written = Math.min(encoded.length, destination.length);
      for (let index = 0; index < written; index++) destination[index] = encoded[index];
      return { read: String(input).length, written };
    }
  }

  class TextDecoder {
    constructor(label = "utf-8", options = {}) {
      const normalized = String(label).toLowerCase().replace(/[_\s]/g, "-");
      if (!["utf-8", "utf8", "unicode-1-1-utf-8"].includes(normalized)) {
        throw new RangeError("Quantic TextDecoder currently supports UTF-8 only");
      }
      this.encoding = "utf-8";
      this.fatal = Boolean(options.fatal);
      this.ignoreBOM = Boolean(options.ignoreBOM);
    }
    decode(input = new Uint8Array()) {
      const bytes = input instanceof Uint8Array
        ? input
        : new Uint8Array(input.buffer || input);
      let output = "";
      let index = 0;
      while (index < bytes.length) {
        const first = bytes[index++];
        let code = first;
        let needed = 0;
        let minimum = 0;

        if (first <= 0x7f) {
          code = first;
        } else if ((first & 0xe0) === 0xc0) {
          code = first & 0x1f; needed = 1; minimum = 0x80;
        } else if ((first & 0xf0) === 0xe0) {
          code = first & 0x0f; needed = 2; minimum = 0x800;
        } else if ((first & 0xf8) === 0xf0) {
          code = first & 0x07; needed = 3; minimum = 0x10000;
        } else {
          if (this.fatal) throw new TypeError("Invalid UTF-8 sequence");
          output += "\ufffd";
          continue;
        }

        let valid = true;
        for (let offset = 0; offset < needed; offset++) {
          if (index >= bytes.length || (bytes[index] & 0xc0) !== 0x80) {
            valid = false;
            break;
          }
          code = (code << 6) | (bytes[index++] & 0x3f);
        }

        if (!valid || (needed > 0 && (code < minimum || code > 0x10ffff || (code >= 0xd800 && code <= 0xdfff)))) {
          if (this.fatal) throw new TypeError("Invalid UTF-8 sequence");
          output += "\ufffd";
          continue;
        }
        output += String.fromCodePoint(code);
      }
      if (!this.ignoreBOM && output.charCodeAt(0) === 0xfeff) output = output.slice(1);
      return output;
    }
  }

  globalThis.TextEncoder = TextEncoder;
  globalThis.TextDecoder = TextDecoder;

  function evaluateMediaQuery(query) {
    return String(query)
      .split(",")
      .map(branch => branch.trim().toLowerCase())
      .filter(Boolean)
      .some(branch => {
        const negated = branch.startsWith("not ");
        let result = true;
        const normalized = negated ? branch.slice(4).trim() : branch;
        for (const condition of normalized.split(/\band\b/).map(part => part.trim())) {
          if (!condition || condition === "all" || condition === "screen" || condition === "only screen") continue;
          const cleaned = condition
            .replace(/^only\s+/, "")
            .replace(/^\(/, "")
            .replace(/\)$/, "")
            .trim();
          let match = cleaned.match(/^min-width\s*:\s*(\d+(?:\.\d+)?)px$/);
          if (match) {
            result = result && initialViewportWidth >= Number(match[1]);
            continue;
          }
          match = cleaned.match(/^max-width\s*:\s*(\d+(?:\.\d+)?)px$/);
          if (match) {
            result = result && initialViewportWidth <= Number(match[1]);
            continue;
          }
          result = false;
        }
        return negated ? !result : result;
      });
  }

  globalThis.innerWidth = initialViewportWidth;
  globalThis.outerWidth = initialViewportWidth;
  globalThis.screen = Object.freeze({
    width: initialViewportWidth,
    availWidth: initialViewportWidth,
    colorDepth: 24,
    pixelDepth: 24
  });
  globalThis.matchMedia = query => {
    const media = String(query);
    const listeners = new Set();
    const object = {
      media,
      get matches() { return evaluateMediaQuery(media); },
      onchange: null,
      addEventListener(type, callback) {
        if (String(type) === "change" && typeof callback === "function") listeners.add(callback);
      },
      removeEventListener(type, callback) {
        if (String(type) === "change") listeners.delete(callback);
      },
      addListener(callback) {
        if (typeof callback === "function") listeners.add(callback);
      },
      removeListener(callback) { listeners.delete(callback); },
      dispatchEvent(event) {
        if (!event || event.type !== "change") return true;
        for (const callback of [...listeners]) callback.call(object, event);
        if (typeof object.onchange === "function") object.onchange.call(object, event);
        return true;
      }
    };
    return object;
  };

  Object.defineProperty(globalThis, "navigator", {
    configurable: false,
    enumerable: true,
    writable: false,
    value: Object.freeze({
      userAgent: "Mozilla/5.0 QuanticGlide/0.5",
      appName: "Netscape",
      appVersion: "5.0",
      platform: "Quantic",
      language: "en-US",
      languages: Object.freeze(["en-US", "en"]),
      hardwareConcurrency: 4,
      deviceMemory: 4,
      maxTouchPoints: 0,
      cookieEnabled: true,
      doNotTrack: "1",
      webdriver: false
    })
  });

  const performanceOrigin = Date.now();
  globalThis.performance = Object.freeze({
    timeOrigin: performanceOrigin,
    now() {
      const elapsed = Math.max(0, Date.now() - performanceOrigin);
      return Math.floor(elapsed / 8) * 8;
    }
  });

  const localState = { ...initialStorage };
  const sessionState = {};
  function makeStorage(state, persistent) {
    return {
      get length() { return Object.keys(state).length; },
      key(index) { return Object.keys(state)[Number(index)] ?? null; },
      getItem(key) {
        key = String(key);
        return Object.prototype.hasOwnProperty.call(state, key) ? state[key] : null;
      },
      setItem(key, value) {
        key = String(key); value = String(value); state[key] = value;
        if (persistent) storageMutations.push({ op: "set", key, value });
      },
      removeItem(key) {
        key = String(key); delete state[key];
        if (persistent) storageMutations.push({ op: "remove", key });
      },
      clear() {
        for (const key of Object.keys(state)) delete state[key];
        if (persistent) storageMutations.push({ op: "clear" });
      }
    };
  }
  globalThis.localStorage = makeStorage(localState, true);
  globalThis.sessionStorage = makeStorage(sessionState, false);

  globalThis.console = {
    log: (...args) => consoleMessages.push({ level: "log", message: args.map(String).join(" ") }),
    info: (...args) => consoleMessages.push({ level: "info", message: args.map(String).join(" ") }),
    warn: (...args) => consoleMessages.push({ level: "warn", message: args.map(String).join(" ") }),
    error: (...args) => consoleMessages.push({ level: "error", message: args.map(String).join(" ") })
  };

  globalThis.setTimeout = (fn, _delay = 0, ...args) => {
    const id = nextTimerId++;
    timers.push({ id, fn, args, repeat: false });
    return id;
  };
  globalThis.clearTimeout = id => {
    const index = timers.findIndex(timer => timer.id === Number(id));
    if (index >= 0) timers.splice(index, 1);
  };
  globalThis.setInterval = (fn, _delay = 0, ...args) => {
    const id = nextTimerId++;
    timers.push({ id, fn, args, repeat: true });
    return id;
  };
  globalThis.clearInterval = globalThis.clearTimeout;
  globalThis.requestAnimationFrame = fn => setTimeout(() => fn(performance.now()), 0);
  globalThis.cancelAnimationFrame = globalThis.clearTimeout;
  globalThis.queueMicrotask = fn => Promise.resolve().then(fn);
  globalThis.requestIdleCallback = fn => setTimeout(
    () => fn({ didTimeout: false, timeRemaining: () => 16 }),
    0
  );
  globalThis.cancelIdleCallback = globalThis.clearTimeout;

  const websocketInstances = new Map();

  function normalizeWebSocketURL(input) {
    let resolved = resolveURL(String(input), currentURL);
    if (resolved.startsWith("https://")) resolved = "wss://" + resolved.slice(8);
    else if (resolved.startsWith("http://")) resolved = "ws://" + resolved.slice(7);
    if (!/^wss?:\/\//i.test(resolved)) throw new TypeError("Invalid WebSocket URL");
    return resolved;
  }

  class WebSocket {
    constructor(url, protocols = []) {
      this._id = nextWebSocketId++;
      this.url = normalizeWebSocketURL(url);
      this.readyState = WebSocket.CONNECTING;
      this.bufferedAmount = 0;
      this.extensions = "";
      this.protocol = "";
      this.binaryType = "arraybuffer";
      this.onopen = null;
      this.onmessage = null;
      this.onerror = null;
      this.onclose = null;
      this._listeners = new Map();

      const protocolList = Array.isArray(protocols)
        ? protocols.map(String)
        : (protocols ? [String(protocols)] : []);
      if (new Set(protocolList).size !== protocolList.length) {
        throw new TypeError("WebSocket protocols must be unique");
      }

      websocketInstances.set(this._id, this);
      websocketCommands.push({
        socket_id: this._id,
        action: "open",
        url: this.url,
        protocols: protocolList,
        text: null,
        binary: null,
        code: null,
        reason: null
      });
    }

    addEventListener(type, callback) {
      if (typeof callback !== "function") return;
      type = String(type);
      if (!this._listeners.has(type)) this._listeners.set(type, []);
      this._listeners.get(type).push(callback);
    }

    removeEventListener(type, callback) {
      const listeners = this._listeners.get(String(type));
      if (!listeners) return;
      const index = listeners.indexOf(callback);
      if (index >= 0) listeners.splice(index, 1);
    }

    _emit(type, properties = {}) {
      const event = new Event(type, { cancelable: false, bubbles: false });
      event.target = this;
      Object.assign(event, properties);
      const handler = this["on" + type];
      if (typeof handler === "function") handler.call(this, event);
      const listeners = this._listeners.get(type) || [];
      for (const listener of [...listeners]) listener.call(this, event);
    }

    send(data) {
      if (this.readyState !== WebSocket.OPEN) {
        throw new Error("WebSocket is not open");
      }

      let text = null;
      let binary = null;
      if (data instanceof Uint8Array) {
        binary = Array.from(data);
      } else if (typeof ArrayBuffer !== "undefined" && data instanceof ArrayBuffer) {
        binary = Array.from(new Uint8Array(data));
      } else {
        text = String(data);
      }

      websocketCommands.push({
        socket_id: this._id,
        action: "send",
        url: null,
        protocols: [],
        text,
        binary,
        code: null,
        reason: null
      });
    }

    close(code = 1000, reason = "") {
      if (this.readyState === WebSocket.CLOSED || this.readyState === WebSocket.CLOSING) return;
      code = Number(code);
      if (!Number.isInteger(code) || (code !== 1000 && (code < 3000 || code > 4999))) {
        throw new RangeError("Invalid WebSocket close code");
      }
      this.readyState = WebSocket.CLOSING;
      websocketCommands.push({
        socket_id: this._id,
        action: "close",
        url: null,
        protocols: [],
        text: null,
        binary: null,
        code,
        reason: String(reason)
      });
    }
  }

  WebSocket.CONNECTING = 0;
  WebSocket.OPEN = 1;
  WebSocket.CLOSING = 2;
  WebSocket.CLOSED = 3;
  Object.defineProperties(WebSocket.prototype, {
    CONNECTING: { value: 0 },
    OPEN: { value: 1 },
    CLOSING: { value: 2 },
    CLOSED: { value: 3 }
  });

  globalThis.WebSocket = WebSocket;

  globalThis.__q_resolve_websocket_event = event => {
    const socket = websocketInstances.get(Number(event.socket_id));
    if (!socket) return false;

    switch (String(event.event_type)) {
      case "open":
        socket.readyState = WebSocket.OPEN;
        socket.protocol = String(event.protocol || "");
        socket._emit("open");
        return true;
      case "message": {
        const data = Array.isArray(event.binary)
          ? new Uint8Array(event.binary).buffer
          : String(event.text || "");
        socket._emit("message", { data });
        return true;
      }
      case "error":
        socket._emit("error", { message: String(event.error || "WebSocket error") });
        return true;
      case "close":
        socket.readyState = WebSocket.CLOSED;
        socket._emit("close", {
          code: Number(event.code || 1006),
          reason: String(event.reason || ""),
          wasClean: Boolean(event.clean)
        });
        websocketInstances.delete(socket._id);
        return true;
      default:
        return false;
    }
  };

  class Headers {
    constructor(init = undefined) {
      this._map = new Map();
      if (init instanceof Headers) {
        for (const [name, value] of init) this.set(name, value);
      } else if (Array.isArray(init)) {
        for (const pair of init) {
          if (Array.isArray(pair) && pair.length >= 2) this.append(pair[0], pair[1]);
        }
      } else if (init && typeof init === "object") {
        for (const [name, value] of Object.entries(init)) this.set(name, value);
      }
    }
    _name(name) { return String(name).trim().toLowerCase(); }
    append(name, value) {
      name = this._name(name); value = String(value).trim();
      const current = this._map.get(name);
      this._map.set(name, current ? current + ", " + value : value);
    }
    set(name, value) { this._map.set(this._name(name), String(value).trim()); }
    get(name) { return this._map.get(this._name(name)) ?? null; }
    has(name) { return this._map.has(this._name(name)); }
    delete(name) { this._map.delete(this._name(name)); }
    entries() { return this._map.entries(); }
    keys() { return this._map.keys(); }
    values() { return this._map.values(); }
    forEach(callback, thisArg = undefined) {
      for (const [name, value] of this._map) callback.call(thisArg, value, name, this);
    }
    [Symbol.iterator]() { return this.entries(); }
    toObject() { return Object.fromEntries(this._map); }
  }

  class Request {
    constructor(input, init = {}) {
      const source = input instanceof Request ? input : null;
      this.url = source ? source.url : String(input);
      this.method = String(init.method || source?.method || "GET").toUpperCase();
      this.headers = new Headers(init.headers || source?.headers);
      this.body = init.body !== undefined ? init.body : (source ? source.body : null);
      this.credentials = String(init.credentials || source?.credentials || "same-origin");
      this.mode = String(init.mode || source?.mode || "cors");
    }
    clone() {
      return new Request(this, {
        method: this.method,
        headers: this.headers,
        body: this.body,
        credentials: this.credentials,
        mode: this.mode
      });
    }
  }

  class Response {
    constructor(body = "", init = {}) {
      this._body = body == null ? "" : String(body);
      this.status = Number(init.status ?? 200);
      this.statusText = String(init.statusText || "");
      this.headers = new Headers(init.headers);
      this.url = String(init.url || "");
      this.type = String(init.type || "basic");
      this.redirected = Boolean(init.redirected);
      this.bodyUsed = false;
    }
    get ok() { return this.status >= 200 && this.status < 300; }
    text() {
      this.bodyUsed = true;
      return Promise.resolve(this._body);
    }
    json() {
      this.bodyUsed = true;
      return Promise.resolve(JSON.parse(this._body));
    }
    arrayBuffer() {
      this.bodyUsed = true;
      return Promise.resolve(new TextEncoder().encode(this._body).buffer);
    }
    clone() {
      if (this.bodyUsed) throw new TypeError("Body has already been consumed");
      return new Response(this._body, {
        status: this.status,
        statusText: this.statusText,
        headers: this.headers,
        url: this.url,
        type: this.type,
        redirected: this.redirected
      });
    }
    static json(value, init = {}) {
      const headers = new Headers(init.headers);
      if (!headers.has("content-type")) headers.set("content-type", "application/json");
      return new Response(JSON.stringify(value), { ...init, headers });
    }
  }

  globalThis.Headers = Headers;
  globalThis.Request = Request;
  globalThis.Response = Response;

  globalThis.fetch = (input, options = {}) => {
    const request = input instanceof Request
      ? new Request(input, options)
      : new Request(input, options);
    const id = nextFetchId++;
    fetches.push({
      id,
      url: request.url,
      method: request.method,
      body: request.body == null ? null : String(request.body),
      headers: request.headers.toObject()
    });
    return new Promise((resolve, reject) => fetchResolvers.set(id, { resolve, reject }));
  };

  globalThis.__q_resolve_fetch = resolution => {
    const pending = fetchResolvers.get(Number(resolution.id));
    if (!pending) return false;
    fetchResolvers.delete(Number(resolution.id));
    if (resolution.error) {
      pending.reject(new TypeError(String(resolution.error)));
      return true;
    }
    const headers = new Headers();
    if (resolution.content_type) headers.set("content-type", resolution.content_type);
    pending.resolve(new Response(String(resolution.body || ""), {
      status: Number(resolution.status),
      url: String(resolution.url),
      headers
    }));
    return true;
  };

  globalThis.__q_flush_timers = limit => {
    let count = 0;
    while (timers.length && count < Number(limit || 64)) {
      const timer = timers.shift();
      if (typeof timer.fn === "function") timer.fn(...timer.args);
      if (timer.repeat && count < Number(limit || 64) - 1) timers.push(timer);
      count++;
    }
    return count;
  };

  globalThis.__q_dispatch_selector = (selector, type) => {
    if (selector === "document") return dispatchKey("document", String(type), documentObject);
    if (selector === "window") return dispatchKey("window", String(type), globalThis);
    const target = query(String(selector), false);
    return target ? dispatchKey(target.__nodeId, String(type), target) : false;
  };

  globalThis.__q_host_set_value = (id, value) => {
    const node = findNodeById(String(id));
    if (!node) return false;
    node.attributes.value = String(value);
    return true;
  };

  globalThis.__q_sync_next_node_id = next => {
    nextNodeId = Math.max(nextNodeId, Number(next) || 0);
    return nextNodeId;
  };

  globalThis.__q_drain_effects = () => {
    const result = {
      mutations: mutations.splice(0),
      fetches: fetches.splice(0),
      storage: storageMutations.splice(0),
      cookies: cookieMutations.splice(0),
      navigation: navigationEffects.splice(0),
      history: historyEffects.splice(0),
      websockets: websocketCommands.splice(0),
      console: consoleMessages.splice(0)
    };
    return result;
  };

  for (const node of nodes.values()) {
    if (node.kind !== "element") continue;
    for (const [name, source] of Object.entries(node.attributes)) {
      if (!name.startsWith("on") || !source) continue;
      try {
        addHandler(node.node_id, name.slice(2), Function("event", String(source)));
      } catch (error) {
        consoleMessages.push({ level: "error", message: "inline handler: " + String(error) });
      }
    }
  }
})();
"##;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html;

    #[test]
    fn executes_real_javascript_and_records_dom_mutation() {
        let document = html::parse("<p id='target'>Old</p>");
        let mut runtime = BoaRuntime::new(&document, &BTreeMap::new()).unwrap();
        runtime
            .eval_script("document.getElementById('target').textContent = 'New';")
            .unwrap();
        let effects = runtime.drain_effects().unwrap();
        assert!(effects.mutations.iter().any(|mutation| matches!(
            mutation,
            DomMutation::SetText { node_id: _, value } if value == "New"
        )));
    }

    #[test]
    fn style_proxy_uses_css_names_and_dynamic_text_content() {
        let document = html::parse("<div id='root'><span>A</span></div>");
        let mut runtime = BoaRuntime::new(&document, &BTreeMap::new()).unwrap();
        runtime
            .eval_script(
                "const root=document.getElementById('root');
                 root.style.backgroundColor='red';
                 root.style.setProperty('--gap','12px');
                 const child=document.createElement('b');
                 child.append('B');
                 root.appendChild(child);
                 root.dataset.result=[root.style.backgroundColor,
                   root.style.getPropertyValue('--gap'),root.textContent].join('|');",
            )
            .unwrap();
        let effects = runtime.drain_effects().unwrap();
        assert!(effects.mutations.iter().any(|mutation| matches!(
            mutation,
            DomMutation::SetAttribute { name, value, .. }
                if name == "data-result" && value == "red|12px|AB"
        )));
        assert!(effects.mutations.iter().any(|mutation| matches!(
            mutation,
            DomMutation::SetAttribute { name, value, .. }
                if name == "style" && value.contains("background-color:red")
        )));
    }

    #[test]
    fn event_listener_removal_and_once_are_enforced() {
        let document = html::parse("<button id='go'>Go</button><p id='out'>0</p>");
        let mut runtime = BoaRuntime::new(&document, &BTreeMap::new()).unwrap();
        runtime
            .eval_script(
                "const go=document.getElementById('go');
                 const out=document.getElementById('out');
                 let count=0;
                 const removed=()=>count+=100;
                 go.addEventListener('click',removed);
                 go.removeEventListener('click',removed);
                 go.addEventListener('click',()=>count++,{once:true});
                 go.click();go.click();
                 out.textContent=String(count);",
            )
            .unwrap();
        let effects = runtime.drain_effects().unwrap();
        assert!(effects.mutations.iter().any(|mutation| matches!(
            mutation,
            DomMutation::SetText { value, .. } if value == "1"
        )));
    }

    #[test]
    fn dom_compat_classlist_dataset_and_event_bubbling_work() {
        let document =
            html::parse("<div id='parent'><button id='child' class='base'>Click</button></div>");
        let mut runtime = BoaRuntime::new(&document, &BTreeMap::new()).unwrap();
        runtime
            .eval_script(
                "const child = document.getElementById('child');
                 const parent = document.getElementById('parent');
                 parent.addEventListener('click', () => {
                   child.classList.add('active');
                   child.dataset.state = 'bubbled';
                 });",
            )
            .unwrap();
        assert!(runtime.dispatch_event("#child", "click").unwrap());
        let effects = runtime.drain_effects().unwrap();

        assert!(effects.mutations.iter().any(|mutation| matches!(
            mutation,
            DomMutation::SetAttribute { name, value, .. }
                if name == "class" && value.contains("active")
        )));
        assert!(effects.mutations.iter().any(|mutation| matches!(
            mutation,
            DomMutation::SetAttribute { name, value, .. }
                if name == "data-state" && value == "bubbled"
        )));
    }

    #[test]
    fn computed_style_bridge_exposes_rust_cascade() {
        let document = html::parse("<div id='box' style='padding-left:7px'>x</div>");
        let sheet = crate::css::parse_stylesheet("#box{color:#123456;font-size:20px}");
        let styles = crate::style::compute_styles(&document, &sheet);
        let mut runtime = BoaRuntime::new_for_page_with_environment(
            &document,
            &styles,
            &BTreeMap::new(),
            None,
            "",
            800,
        )
        .unwrap();
        runtime
            .eval_script(
                "const style=getComputedStyle(document.getElementById('box'));
                 document.getElementById('box').dataset.computed =
                   [style.color,style.fontSize,style.paddingLeft].join('|');",
            )
            .unwrap();
        let effects = runtime.drain_effects().unwrap();
        assert!(effects.mutations.iter().any(|mutation| matches!(
            mutation,
            DomMutation::SetAttribute { name, value, .. }
                if name == "data-computed" && value == "rgb(18, 52, 86)|20px|7px"
        )));
    }

    #[test]
    fn match_media_tracks_runtime_viewport_width() {
        let document = html::parse("<p id='out'>x</p>");
        let mut runtime = BoaRuntime::new_for_page_with_viewport(
            &document,
            &BTreeMap::new(),
            None,
            "",
            480,
        )
        .unwrap();
        runtime
            .eval_script(
                "document.getElementById('out').textContent =
                  [innerWidth, matchMedia('(max-width: 500px)').matches,
                   matchMedia('(min-width: 700px)').matches].join('|');",
            )
            .unwrap();
        let effects = runtime.drain_effects().unwrap();
        assert!(effects.mutations.iter().any(|mutation| matches!(
            mutation,
            DomMutation::SetText { value, .. } if value == "480|true|false"
        )));
    }

    #[test]
    fn privacy_oriented_navigator_and_performance_are_exposed() {
        let document = html::parse("<p id='out'>x</p>");
        let mut runtime = BoaRuntime::new(&document, &BTreeMap::new()).unwrap();
        runtime
            .eval_script(
                "document.getElementById('out').textContent =
                  [navigator.webdriver, navigator.doNotTrack, navigator.hardwareConcurrency,
                   typeof performance.now, document.readyState].join('|');",
            )
            .unwrap();
        let effects = runtime.drain_effects().unwrap();
        assert!(effects.mutations.iter().any(|mutation| matches!(
            mutation,
            DomMutation::SetText { value, .. }
                if value == "false|1|4|function|complete"
        )));
    }

    #[test]
    fn history_go_exposes_traversal_delta() {
        let document = html::parse("<p id='out'>x</p>");
        let base = Url::parse("https://app.example/start").unwrap();
        let mut runtime =
            BoaRuntime::new_for_page(&document, &BTreeMap::new(), Some(&base), "").unwrap();

        runtime
            .eval_script(
                "history.pushState({}, '', '/one');
                 history.pushState({}, '', '/two');
                 history.back();",
            )
            .unwrap();
        let effects = runtime.drain_effects().unwrap();
        assert_eq!(effects.history.len(), 3);
        assert_eq!(effects.history[2].url, "https://app.example/one");
        assert_eq!(effects.history[2].delta, Some(-1));
    }

    #[test]
    fn url_history_location_and_cookie_effects_are_host_visible() {
        let document = html::parse("<p id='out'>x</p>");
        let base = Url::parse("https://app.example/a/page?old=1").unwrap();
        let mut runtime =
            BoaRuntime::new_for_page(&document, &BTreeMap::new(), Some(&base), "theme=light")
                .unwrap();

        runtime
            .eval_script(
                "const params = new URLSearchParams('a=1&b=2');
                 params.set('a', '9');
                 document.cookie = 'theme=dark; Path=/';
                 history.pushState({}, '', '../next?' + params.toString());
                 location.assign('/final#ok');
                 document.getElementById('out').textContent =
                   [location.origin, document.cookie, params.get('a')].join('|');",
            )
            .unwrap();

        let effects = runtime.drain_effects().unwrap();
        assert_eq!(effects.cookies.len(), 1);
        assert_eq!(effects.history.len(), 1);
        assert_eq!(effects.navigation.len(), 1);
        assert_eq!(effects.navigation[0].url, "https://app.example/final#ok");
        assert!(effects.mutations.iter().any(|mutation| matches!(
            mutation,
            DomMutation::SetText { value, .. }
                if value == "https://app.example|theme=dark|9"
        )));
    }

    #[test]
    fn query_selector_supports_child_attribute_and_pseudo_selectors() {
        let document = html::parse(
            "<form><input id='one' type='checkbox' checked><input id='two' type='checkbox'></form>",
        );
        let mut runtime = BoaRuntime::new(&document, &BTreeMap::new()).unwrap();
        runtime
            .eval_script(
                "const hit = document.querySelector('form > input[type=checkbox]:checked:first-child');
                 hit.dataset.matched = 'yes';",
            )
            .unwrap();
        let effects = runtime.drain_effects().unwrap();
        assert!(effects.mutations.iter().any(|mutation| matches!(
            mutation,
            DomMutation::SetAttribute { name, value, .. }
                if name == "data-matched" && value == "yes"
        )));
    }

    #[test]
    fn query_selector_supports_siblings_attributes_and_nth_child() {
        let document = html::parse(
            "<section><h2>T</h2><p id='lead' data-role='leader'>A</p><span>x</span><p id='target' class='note hot' lang='fr-CA'>B</p><p id='last'>C</p></section>",
        );
        let mut runtime = BoaRuntime::new(&document, &BTreeMap::new()).unwrap();
        runtime
            .eval_script(
                "const adjacent = document.querySelector('h2 + p[data-role^=lead]');
                 const target = document.querySelector('h2 + p ~ p[class~=note][lang|=fr]:nth-child(4)');
                 const last = document.querySelector('section > p:last-of-type');
                 target.dataset.result = [adjacent.id, target.id, last.id].join('|');",
            )
            .unwrap();
        let effects = runtime.drain_effects().unwrap();
        assert!(effects.mutations.iter().any(|mutation| matches!(
            mutation,
            DomMutation::SetAttribute { name, value, .. }
                if name == "data-result" && value == "lead|target|last"
        )));
    }

    #[test]
    fn text_encoding_and_dom_traversal_shims_work() {
        let document =
            html::parse("<div><span id='a'>A</span><span id='b'>B</span></div><p id='out'>x</p>");
        let mut runtime = BoaRuntime::new(&document, &BTreeMap::new()).unwrap();
        runtime
            .eval_script(
                "const encoded = new TextEncoder().encode('hé');
                 const decoded = new TextDecoder().decode(encoded);
                 const sibling = document.getElementById('a').nextElementSibling.id;
                 document.getElementById('out').textContent =
                   decoded + '|' + encoded.length + '|' + sibling;",
            )
            .unwrap();
        let effects = runtime.drain_effects().unwrap();
        assert!(effects.mutations.iter().any(|mutation| matches!(
            mutation,
            DomMutation::SetText { value, .. } if value == "hé|3|b"
        )));
    }

    #[test]
    fn urlsearchparams_size_optional_undefined_and_url_binding_work() {
        let document = html::parse("<p id='out'>x</p>");
        let mut runtime = BoaRuntime::new(&document, &BTreeMap::new()).unwrap();
        runtime
            .eval_script(
                "const params = new URLSearchParams('a=1&b=2&a=3');
                 const before = params.size;
                 const undefinedHas = params.has('a', undefined);
                 params.delete('a');
                 const after = params.size;
                 const url = new URL('https://example.test/?a=1&b=2&a=3');
                 const live = url.searchParams;
                 live.delete('a');
                 live.append('b', '4');
                 document.getElementById('out').textContent =
                   [before, undefinedHas, after, url.search, url.searchParams.size].join('|');",
            )
            .unwrap();
        let effects = runtime.drain_effects().unwrap();
        assert!(effects.mutations.iter().any(|mutation| matches!(
            mutation,
            DomMutation::SetText { value, .. }
                if value == "3|true|1|?b=2&b=4|2"
        )));
    }

    #[test]
    fn ecmascript_module_graph_resolves_relative_imports() {
        let document = html::parse("<p id='out'>x</p>");
        let mut runtime = BoaRuntime::new(&document, &BTreeMap::new()).unwrap();
        let mut dependencies = BTreeMap::new();
        dependencies.insert(
            "quantic/dep.js".to_string(),
            "export const value = 42;".to_string(),
        );

        runtime
            .eval_module_graph(
                "import { value } from './dep.js';
                 document.getElementById('out').textContent = String(value);",
                "quantic/main.js",
                &dependencies,
            )
            .unwrap();

        let effects = runtime.drain_effects().unwrap();
        assert!(effects.mutations.iter().any(|mutation| matches!(
            mutation,
            DomMutation::SetText { value, .. } if value == "42"
        )));
    }

    #[test]
    fn ecmascript_module_evaluation_uses_boa_module_pipeline() {
        let document = html::parse("<p id='out'>x</p>");
        let mut runtime = BoaRuntime::new(&document, &BTreeMap::new()).unwrap();
        runtime
            .eval_module(
                "const value = 21 * 2;
                 document.getElementById('out').textContent = String(value);
                 export { value };",
            )
            .unwrap();
        let effects = runtime.drain_effects().unwrap();
        assert!(effects.mutations.iter().any(|mutation| matches!(
            mutation,
            DomMutation::SetText { value, .. } if value == "42"
        )));
    }

    #[test]
    fn fetch_request_response_and_headers_are_host_visible() {
        let document = html::parse("<p id='out'>x</p>");
        let mut runtime = BoaRuntime::new(&document, &BTreeMap::new()).unwrap();
        runtime
            .eval_script(
                "const headers = new Headers({'X-Test':'yes'});
                 const request = new Request('/api', {method:'POST', headers, body:'hello'});
                 fetch(request).then(response => response.text());",
            )
            .unwrap();
        let effects = runtime.drain_effects().unwrap();
        assert_eq!(effects.fetches.len(), 1);
        assert_eq!(effects.fetches[0].method, "POST");
        assert_eq!(effects.fetches[0].body.as_deref(), Some("hello"));
        assert_eq!(
            effects.fetches[0].headers.get("x-test").map(String::as_str),
            Some("yes")
        );
    }

    #[test]
    fn websocket_api_emits_host_commands_and_accepts_host_events() {
        let document = html::parse("<p id='out'>x</p>");
        let base = Url::parse("https://app.example/start").unwrap();
        let mut runtime =
            BoaRuntime::new_for_page(&document, &BTreeMap::new(), Some(&base), "").unwrap();

        runtime
            .eval_script(
                "const socket = new WebSocket('/events', ['quantic']);
                 socket.onopen = () => {
                   document.getElementById('out').textContent = socket.protocol + '|open';
                 };
                 socket.onmessage = event => {
                   document.getElementById('out').textContent += '|' + event.data;
                 };",
            )
            .unwrap();

        let effects = runtime.drain_effects().unwrap();
        assert_eq!(effects.websockets.len(), 1);
        let open = &effects.websockets[0];
        assert_eq!(open.action, "open");
        assert_eq!(open.url.as_deref(), Some("wss://app.example/events"));
        assert_eq!(open.protocols, vec!["quantic"]);

        runtime
            .resolve_websocket_event(&WebSocketEvent::open(1, "quantic".to_string()))
            .unwrap();
        runtime
            .resolve_websocket_event(&WebSocketEvent::text(1, "hello".to_string()))
            .unwrap();
        let effects = runtime.drain_effects().unwrap();
        assert!(effects.mutations.iter().any(|mutation| matches!(
            mutation,
            DomMutation::SetText { value, .. } if value == "quantic|open|hello"
        )));
    }

    #[test]
    fn urlsearchparams_uses_form_encoding_and_code_unit_sorting() {
        let document = html::parse("<p id='out'>x</p>");
        let mut runtime = BoaRuntime::new(&document, &BTreeMap::new()).unwrap();
        runtime
            .eval_script(
                "const params = new URLSearchParams();
                 params.append('a b', 'c+d');
                 params.append('!', '~');
                 const malformed = new URLSearchParams('value=%2%2af%2a');
                 const sorted = new URLSearchParams('ﬃ&🌈');
                 sorted.sort();
                 document.getElementById('out').textContent =
                   [params.toString(), malformed.get('value'), [...sorted.keys()].join(',')].join('|');",
            )
            .unwrap();
        let effects = runtime.drain_effects().unwrap();
        assert!(effects.mutations.iter().any(|mutation| matches!(
            mutation,
            DomMutation::SetText { value, .. }
                if value == "a+b=c%2Bd&%21=%7E|%2*f*|🌈,ﬃ"
        )));
    }

    #[test]
    fn text_encoder_replaces_lone_utf16_surrogates_with_replacement_character() {
        let document = html::parse("<p id='out'>x</p>");
        let mut runtime = BoaRuntime::new(&document, &BTreeMap::new()).unwrap();
        runtime
            .eval_script(
                "const bytes = Array.from(new TextEncoder().encode('\\uD800'));
                 document.getElementById('out').textContent = bytes.join(',');",
            )
            .unwrap();
        let effects = runtime.drain_effects().unwrap();
        assert!(effects.mutations.iter().any(|mutation| matches!(
            mutation,
            DomMutation::SetText { value, .. } if value == "239,191,189"
        )));
    }

    #[test]
    fn infinite_loop_hits_runtime_budget() {
        let document = html::parse("<div></div>");
        let mut runtime = BoaRuntime::new(&document, &BTreeMap::new()).unwrap();
        let result = runtime.eval_script("while (true) {}");
        assert!(result.is_err());
    }

    #[test]
    fn promises_timers_and_storage_are_host_visible() {
        let document = html::parse("<div id='out'></div>");
        let mut runtime = BoaRuntime::new(&document, &BTreeMap::new()).unwrap();
        runtime
            .eval_script(
                "localStorage.setItem('theme','dark'); setTimeout(() => document.getElementById('out').textContent='done', 1);",
            )
            .unwrap();
        let effects = runtime.drain_effects().unwrap();
        assert!(effects.storage.iter().any(|mutation| matches!(
            mutation,
            StorageMutation::Set { key, value } if key == "theme" && value == "dark"
        )));
        assert!(effects.mutations.iter().any(|mutation| matches!(
            mutation,
            DomMutation::SetText { value, .. } if value == "done"
        )));
    }
}
