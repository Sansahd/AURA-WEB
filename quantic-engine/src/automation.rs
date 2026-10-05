use std::path::Path;

use serde::Serialize;
use url::Url;

use crate::{
    dom::{Document, NodeKind},
    BrowserSession, Engine, FileUpload, PageError,
};

#[derive(Debug, Clone, Serialize)]
pub struct AutomationElement {
    pub node_id: usize,
    pub tag: String,
    pub id: String,
    pub name: String,
    pub input_type: String,
    pub placeholder: String,
    pub href: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AutomationField {
    pub node_id: usize,
    pub tag: String,
    pub id: String,
    pub name: String,
    pub input_type: String,
    pub placeholder: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AutomationForm {
    pub node_id: usize,
    pub id: String,
    pub action: String,
    pub method: String,
    pub has_file: bool,
    pub fields: Vec<AutomationField>,
}

pub struct AutomationBridge {
    session: BrowserSession,
}

impl Default for AutomationBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl AutomationBridge {
    pub fn new() -> Self {
        Self {
            session: BrowserSession::new(Engine::new()),
        }
    }

    pub fn open(&mut self, address: &str) -> Result<(), PageError> {
        self.session.open(address)
    }

    pub fn current_url(&self) -> Option<&str> {
        self.session.current_url()
    }

    pub fn back(&mut self) -> Result<bool, PageError> {
        self.session.back()
    }

    pub fn forward(&mut self) -> Result<bool, PageError> {
        self.session.forward()
    }

    pub fn reload(&mut self) -> Result<bool, PageError> {
        self.session.reload()
    }

    pub fn poll_async_events(&mut self) -> Result<usize, PageError> {
        self.session.poll_async_events()
    }

    pub fn execute_script(&mut self, source: &str) -> Result<(), PageError> {
        self.session.execute_script(source)
    }

    pub fn set_value(&mut self, element_id: &str, value: &str) -> Result<(), PageError> {
        self.session.set_value(element_id, value)
    }

    pub fn click(&mut self, element_id: &str) -> Result<bool, PageError> {
        let allowed = {
            let page = self
                .session
                .current_page_mut()
                .ok_or_else(|| PageError::Protocol("no page loaded".to_string()))?;
            page.click(element_id)?
        };
        let _ = self.session.follow_pending_navigation()?;
        Ok(allowed)
    }

    pub fn submit_form(&mut self, form_id: &str) -> Result<bool, PageError> {
        self.session.submit_form(form_id)
    }

    pub fn submit_form_with_file(
        &mut self,
        form_id: &str,
        field_name: &str,
        path: impl AsRef<Path>,
        content_type: &str,
    ) -> Result<bool, String> {
        let path = path.as_ref();
        let bytes = std::fs::read(path)
            .map_err(|error| format!("cannot read upload file {}: {error}", path.display()))?;
        let filename = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("upload.bin")
            .to_string();
        let file = FileUpload {
            field_name: field_name.to_string(),
            filename,
            content_type: content_type.to_string(),
            bytes,
        };
        self.session
            .submit_form_with_files(form_id, &[file])
            .map_err(|error| error.to_string())
    }

    pub fn page_text(&self) -> String {
        let Some(page) = self.session.current_page() else {
            return String::new();
        };
        let document = page.document();
        let root = document
            .nodes
            .iter()
            .enumerate()
            .find_map(|(id, _)| (document.element_tag(id) == Some("body")).then_some(id))
            .unwrap_or(document.root);
        normalize_text(&text_content(document, root))
    }

    pub fn elements(&self) -> Vec<AutomationElement> {
        let Some(page) = self.session.current_page() else {
            return Vec::new();
        };
        let document = page.document();
        document
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(node_id, node)| {
                let NodeKind::Element { tag, .. } = &node.kind else {
                    return None;
                };
                Some(AutomationElement {
                    node_id,
                    tag: tag.clone(),
                    id: attr(document, node_id, "id"),
                    name: attr(document, node_id, "name"),
                    input_type: attr(document, node_id, "type"),
                    placeholder: attr(document, node_id, "placeholder"),
                    href: attr(document, node_id, "href"),
                    text: normalize_text(&text_content(document, node_id)),
                })
            })
            .collect()
    }

    pub fn links(&self) -> Vec<AutomationElement> {
        self.elements()
            .into_iter()
            .filter(|element| element.tag.eq_ignore_ascii_case("a") && !element.href.is_empty())
            .collect()
    }

    pub fn forms(&self) -> Vec<AutomationForm> {
        let Some(page) = self.session.current_page() else {
            return Vec::new();
        };
        let document = page.document();
        document
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(node_id, node)| {
                let NodeKind::Element { tag, .. } = &node.kind else {
                    return None;
                };
                if !tag.eq_ignore_ascii_case("form") {
                    return None;
                }
                let fields = document
                    .descendants(node_id)
                    .into_iter()
                    .filter_map(|field_id| {
                        let tag = document.element_tag(field_id)?.to_ascii_lowercase();
                        if !matches!(tag.as_str(), "input" | "textarea" | "select") {
                            return None;
                        }
                        Some(AutomationField {
                            node_id: field_id,
                            tag,
                            id: attr(document, field_id, "id"),
                            name: attr(document, field_id, "name"),
                            input_type: attr(document, field_id, "type"),
                            placeholder: attr(document, field_id, "placeholder"),
                        })
                    })
                    .collect::<Vec<_>>();
                let has_file = fields
                    .iter()
                    .any(|field| field.input_type.eq_ignore_ascii_case("file"));
                Some(AutomationForm {
                    node_id,
                    id: attr(document, node_id, "id"),
                    action: attr(document, node_id, "action"),
                    method: attr(document, node_id, "method"),
                    has_file,
                    fields,
                })
            })
            .collect()
    }

    pub fn resolved_links(&self) -> Vec<(String, String)> {
        let base = self
            .current_url()
            .and_then(|value| Url::parse(value).ok());
        self.links()
            .into_iter()
            .filter_map(|link| {
                let href = link.href.trim();
                if href.is_empty() || href.starts_with("javascript:") {
                    return None;
                }
                let resolved = match &base {
                    Some(base) => base
                        .join(href)
                        .map(|url| url.to_string())
                        .unwrap_or_else(|_| href.to_string()),
                    None => href.to_string(),
                };
                Some((resolved, link.text))
            })
            .collect()
    }

    pub fn save_viewport(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let image = self
            .session
            .rasterize_viewport()
            .ok_or_else(|| "no rendered page".to_string())?;
        image.save(path).map_err(|error| error.to_string())
    }
}

fn attr(document: &Document, node_id: usize, name: &str) -> String {
    document
        .attribute(node_id, name)
        .unwrap_or_default()
        .to_string()
}

fn text_content(document: &Document, node_id: usize) -> String {
    let Some(node) = document.node(node_id) else {
        return String::new();
    };
    match &node.kind {
        NodeKind::Text(text) => text.clone(),
        _ => node
            .children
            .iter()
            .map(|child| text_content(document, *child))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
