#![forbid(unsafe_code)]

pub mod automation;
pub mod browser;
pub mod cache;
pub mod compositor;
pub mod css;
pub mod dom;
pub mod engine;
pub mod font;
pub mod html;
pub mod image_decode;
pub mod interactive;
pub mod js_runtime;
pub mod layout;
pub mod network;
pub mod paint;
pub mod privacy;
pub mod resources;
pub mod storage;
pub mod style;
pub mod url_context;
pub mod websocket;

pub use automation::{AutomationBridge, AutomationElement, AutomationField, AutomationForm};
pub use browser::{BrowserSession, FileUpload, HistoryEntry};
pub use cache::{is_cacheable_kind, ResourceCache};
pub use compositor::ViewportCompositor;
pub use engine::{Engine, EngineLoadError, EngineOutput};
pub use font::{FontSpec, FontSystem, TextMetrics, WebFontFace};
pub use interactive::{
    FormField, FormSubmission, HistoryUpdate, InteractivePage, NavigationRequest, PageError,
};
pub use js_runtime::{BoaRuntime, JsRuntime, JsRuntimeError};
pub use privacy::{PrivacyDecision, PrivacyPolicy, RequestContext, ResourceKind};
pub use resources::{ResourceCandidate, ResourceClass, ResourcePlan};
pub use url_context::effective_base_url;

pub const ENGINE_NAME: &str = "Quantic Engine";
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");
