use image::RgbaImage;
use url::Url;

use crate::privacy::ResourceKind;

use crate::{
    compositor::ViewportCompositor, Engine, EngineOutput, HistoryUpdate, InteractivePage,
    NavigationRequest, PageError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HitTarget {
    pub node_id: usize,
    pub tag: String,
    pub element_id: String,
    pub href: String,
    pub text: String,
    pub input_type: String,
    pub value: String,
    pub placeholder: String,
    pub form_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileUpload {
    pub field_name: String,
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

pub struct BrowserSession {
    engine: Engine,
    page: Option<InteractivePage>,
    history: Vec<HistoryEntry>,
    cursor: Option<usize>,
    viewport_height: u32,
    scroll_y: u32,
    same_document_start: usize,
    same_document_end: usize,
}

impl BrowserSession {
    pub fn new(engine: Engine) -> Self {
        Self {
            engine,
            page: None,
            history: Vec::new(),
            cursor: None,
            viewport_height: 768,
            scroll_y: 0,
            same_document_start: 0,
            same_document_end: 0,
        }
    }

    pub fn with_viewport_height(mut self, height: u32) -> Self {
        self.viewport_height = height.max(160);
        self
    }

    pub fn open(&mut self, address: &str) -> Result<(), PageError> {
        let page = self.engine.load_interactive_url(address)?;
        self.commit_page(page, address, false);
        Ok(())
    }

    pub fn open_html(&mut self, source: &str) -> Result<(), PageError> {
        let page = self.engine.load_interactive_html(source)?;
        self.commit_page(page, "about:blank", false);
        Ok(())
    }

    pub fn open_html_with_base(&mut self, source: &str, base: &str) -> Result<(), PageError> {
        let page = self.engine.load_interactive_html_with_base(source, base)?;
        self.commit_page(page, base, false);
        Ok(())
    }

    pub fn navigate(&mut self, request: &NavigationRequest) -> Result<(), PageError> {
        let page = self.engine.follow_navigation(request)?;
        self.commit_page(page, &request.url, request.replace);
        Ok(())
    }

    pub fn replace(&mut self, request: &NavigationRequest) -> Result<(), PageError> {
        let page = self.engine.follow_navigation(request)?;
        self.commit_page(page, &request.url, true);
        Ok(())
    }

    pub fn reload(&mut self) -> Result<bool, PageError> {
        let Some(url) = self.current_url().map(str::to_string) else {
            return Ok(false);
        };
        let page = self.engine.load_interactive_url(&url)?;
        self.commit_page(page, &url, true);
        Ok(true)
    }

    pub fn back(&mut self) -> Result<bool, PageError> {
        let Some(current) = self.cursor else {
            return Ok(false);
        };
        if current == 0 {
            return Ok(false);
        }
        let target = current - 1;
        if target >= self.same_document_start && target <= self.same_document_end {
            self.page_mut()?.execute_script("history.back()")?;
            self.sync_history_updates()?;
            return Ok(self.cursor == Some(target));
        }
        self.load_history_index(target)?;
        Ok(true)
    }

    pub fn forward(&mut self) -> Result<bool, PageError> {
        let Some(current) = self.cursor else {
            return Ok(false);
        };
        let target = current + 1;
        if target >= self.history.len() {
            return Ok(false);
        }
        if target >= self.same_document_start && target <= self.same_document_end {
            self.page_mut()?.execute_script("history.forward()")?;
            self.sync_history_updates()?;
            return Ok(self.cursor == Some(target));
        }
        self.load_history_index(target)?;
        Ok(true)
    }

    pub fn current_page(&self) -> Option<&InteractivePage> {
        self.page.as_ref()
    }

    pub fn current_page_mut(&mut self) -> Option<&mut InteractivePage> {
        self.page.as_mut()
    }

    pub fn current_url(&self) -> Option<&str> {
        self.cursor
            .and_then(|index| self.history.get(index))
            .map(|entry| entry.url.as_str())
    }

    pub fn history(&self) -> &[HistoryEntry] {
        &self.history
    }

    pub fn can_go_back(&self) -> bool {
        self.cursor.is_some_and(|index| index > 0)
    }

    pub fn can_go_forward(&self) -> bool {
        self.cursor
            .is_some_and(|index| index + 1 < self.history.len())
    }

    pub fn snapshot(&self) -> Option<EngineOutput> {
        self.page.as_ref().map(InteractivePage::snapshot)
    }

    pub fn click_link(&mut self, element_id: &str) -> Result<bool, PageError> {
        let request = {
            let page = self.page_mut()?;
            page.link_navigation(element_id)?
        };
        self.sync_history_updates()?;
        let Some(request) = request else {
            return Ok(false);
        };
        self.navigate(&request)?;
        Ok(true)
    }

    pub fn submit_form(&mut self, form_id: &str) -> Result<bool, PageError> {
        let request = {
            let page = self.page_mut()?;
            page.form_navigation(form_id)?
        };
        self.sync_history_updates()?;
        let Some(request) = request else {
            return Ok(false);
        };
        self.navigate(&request)?;
        Ok(true)
    }

    pub fn submit_form_with_files(
        &mut self,
        form_id: &str,
        files: &[FileUpload],
    ) -> Result<bool, PageError> {
        if files.is_empty() {
            return self.submit_form(form_id);
        }

        let (submission, top_level) = {
            let page = self.page_mut()?;
            let top_level = page
                .document_url()
                .cloned()
                .ok_or_else(|| PageError::Protocol("file upload requires a network page".to_string()))?;
            let submission = page.submit_form(form_id)?;
            (submission, top_level)
        };

        if submission.prevented {
            return Ok(false);
        }
        if submission.method.eq_ignore_ascii_case("GET") {
            return Err(PageError::Protocol(
                "file upload requires a non-GET form method".to_string(),
            ));
        }

        let target = Url::parse(&submission.action).map_err(PageError::InvalidUrl)?;
        let boundary = multipart_boundary(&submission.fields, files);
        let body = encode_multipart(&boundary, &submission.fields, files);
        let content_type = format!("multipart/form-data; boundary={boundary}");

        let response = self
            .engine
            .fetch_resource_bytes(
                &top_level,
                &target,
                ResourceKind::Document,
                &submission.method,
                Some(&body),
                Some(&content_type),
            )
            .map_err(PageError::Network)?;

        let final_url = response.final_url.to_string();
        let source = String::from_utf8_lossy(&response.body).into_owned();
        let page = InteractivePage::from_remote_source(
            self.engine.clone(),
            response.final_url,
            source,
        )?;
        self.commit_page(page, &final_url, false);
        Ok(true)
    }

    pub fn set_value(&mut self, element_id: &str, value: &str) -> Result<(), PageError> {
        self.page_mut()?.set_value(element_id, value)?;
        self.sync_history_updates()
    }

    pub fn execute_script(&mut self, source: &str) -> Result<(), PageError> {
        self.page_mut()?.execute_script(source)?;
        self.sync_history_updates()
    }

    pub fn poll_async_events(&mut self) -> Result<usize, PageError> {
        let delivered = self.page_mut()?.poll_async_events()?;
        self.sync_history_updates()?;
        Ok(delivered)
    }

    pub fn take_pending_navigation(&mut self) -> Result<Option<NavigationRequest>, PageError> {
        Ok(self.page_mut()?.take_navigation())
    }

    pub fn follow_pending_navigation(&mut self) -> Result<bool, PageError> {
        let Some(request) = self.take_pending_navigation()? else {
            return Ok(false);
        };
        self.navigate(&request)?;
        Ok(true)
    }

    pub fn scroll_y(&self) -> u32 {
        self.scroll_y
    }

    pub fn scroll_to(&mut self, y: u32) -> u32 {
        let max = self.max_scroll_y();
        self.scroll_y = y.min(max);
        self.scroll_y
    }

    pub fn scroll_by(&mut self, delta: i32) -> u32 {
        let next = if delta.is_negative() {
            self.scroll_y.saturating_sub(delta.unsigned_abs())
        } else {
            self.scroll_y.saturating_add(delta as u32)
        };
        self.scroll_to(next)
    }

    pub fn max_scroll_y(&self) -> u32 {
        self.snapshot()
            .map(|output| output.content_height.saturating_sub(self.viewport_height))
            .unwrap_or(0)
    }

    pub fn rasterize_viewport(&self) -> Option<RgbaImage> {
        let output = self.snapshot()?;
        let full_page = self.engine.rasterize(&output);
        let compositor = ViewportCompositor::new(output.viewport_width, self.viewport_height);
        Some(compositor.compose(&full_page, self.scroll_y))
    }

    pub fn hit_test(&self, x: u32, y: u32) -> Option<HitTarget> {
        let output = self.snapshot()?;
        let document = &output.document;
        let document_y = y.saturating_add(self.scroll_y);

        let node_id = output
            .display_list
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                x >= item.x
                    && x < item.x.saturating_add(item.width.max(1))
                    && document_y >= item.y
                    && document_y < item.y.saturating_add(item.height.max(1))
            })
            .max_by_key(|(index, item)| (item.depth, *index))
            .map(|(_, item)| item.node_id)?;

        let mut current = Some(node_id);
        while let Some(id) = current {
            let tag = document.element_tag(id).unwrap_or_default().to_ascii_lowercase();
            if matches!(tag.as_str(), "a" | "button" | "input" | "textarea" | "select") {
                let form_id = std::iter::once(id)
                    .chain(document.ancestors(id))
                    .find(|candidate| document.element_tag(*candidate) == Some("form"))
                    .and_then(|form| document.attribute(form, "id"))
                    .unwrap_or_default()
                    .to_string();

                return Some(HitTarget {
                    node_id: id,
                    tag,
                    element_id: document.attribute(id, "id").unwrap_or_default().to_string(),
                    href: document.attribute(id, "href").unwrap_or_default().to_string(),
                    text: document.text_content(id).split_whitespace().collect::<Vec<_>>().join(" "),
                    input_type: document.attribute(id, "type").unwrap_or_default().to_ascii_lowercase(),
                    value: document.attribute(id, "value").unwrap_or_default().to_string(),
                    placeholder: document.attribute(id, "placeholder").unwrap_or_default().to_string(),
                    form_id,
                });
            }
            current = document.node(id).and_then(|node| node.parent);
        }
        None
    }

    pub fn activate_at(&mut self, x: u32, y: u32) -> Result<Option<HitTarget>, PageError> {
        let Some(target) = self.hit_test(x, y) else {
            return Ok(None);
        };

        if target.element_id.is_empty() {
            return Ok(Some(target));
        }

        match target.tag.as_str() {
            "a" => {
                let _ = self.click_link(&target.element_id)?;
            }
            "button" => {
                let allowed = {
                    let page = self.page_mut()?;
                    page.click(&target.element_id)?
                };
                self.sync_history_updates()?;
                let navigated = self.follow_pending_navigation()?;
                if allowed && !navigated && !target.form_id.is_empty()
                    && (target.input_type.is_empty() || target.input_type == "submit")
                {
                    let _ = self.submit_form(&target.form_id)?;
                }
            }
            "input" if target.input_type == "submit" => {
                let allowed = {
                    let page = self.page_mut()?;
                    page.click(&target.element_id)?
                };
                self.sync_history_updates()?;
                let navigated = self.follow_pending_navigation()?;
                if allowed && !navigated && !target.form_id.is_empty() {
                    let _ = self.submit_form(&target.form_id)?;
                }
            }
            _ => {}
        }

        Ok(Some(target))
    }

    fn page_mut(&mut self) -> Result<&mut InteractivePage, PageError> {
        self.page
            .as_mut()
            .ok_or_else(|| PageError::Protocol("no page loaded".to_string()))
    }

    fn commit_page(&mut self, page: InteractivePage, requested: &str, replace: bool) {
        let initial_url = page
            .initial_document_url()
            .map(|url| url.as_str().to_string())
            .unwrap_or_else(|| requested.to_string());

        if replace {
            if let Some(index) = self.cursor {
                if let Some(entry) = self.history.get_mut(index) {
                    entry.url = initial_url;
                }
            } else {
                self.history.push(HistoryEntry { url: initial_url });
                self.cursor = Some(0);
            }
        } else {
            if let Some(index) = self.cursor {
                self.history.truncate(index + 1);
            } else {
                self.history.clear();
            }
            self.history.push(HistoryEntry { url: initial_url });
            self.cursor = Some(self.history.len() - 1);
        }

        self.page = Some(page);
        self.scroll_y = 0;
        let current = self.cursor.unwrap_or(0);
        self.same_document_start = current;
        self.same_document_end = current;
        let _ = self.sync_history_updates();
    }

    fn sync_history_updates(&mut self) -> Result<(), PageError> {
        loop {
            let update = {
                let page = self.page_mut()?;
                page.take_history_update()
            };
            let Some(update) = update else {
                return Ok(());
            };
            self.apply_history_update(update);
        }
    }

    fn apply_history_update(&mut self, update: HistoryUpdate) {
        if let Some(delta) = update.delta {
            let Some(current) = self.cursor else {
                return;
            };
            let target = if delta.is_negative() {
                current.saturating_sub(delta.unsigned_abs() as usize)
            } else {
                current.saturating_add(delta as usize)
            };
            if target >= self.same_document_start
                && target <= self.same_document_end
                && target < self.history.len()
            {
                self.cursor = Some(target);
                if let Some(entry) = self.history.get_mut(target) {
                    entry.url = update.url;
                }
            }
            return;
        }

        if update.replace {
            if let Some(index) = self.cursor {
                if let Some(entry) = self.history.get_mut(index) {
                    entry.url = update.url;
                }
            } else {
                self.history.push(HistoryEntry { url: update.url });
                self.cursor = Some(0);
                self.same_document_start = 0;
                self.same_document_end = 0;
            }
            return;
        }

        if let Some(index) = self.cursor {
            self.history.truncate(index + 1);
        } else {
            self.history.clear();
        }
        self.history.push(HistoryEntry { url: update.url });
        let current = self.history.len() - 1;
        self.cursor = Some(current);
        self.same_document_end = current;
    }

    fn load_history_index(&mut self, index: usize) -> Result<(), PageError> {
        let url = self
            .history
            .get(index)
            .map(|entry| entry.url.clone())
            .ok_or_else(|| PageError::Protocol("history entry missing".to_string()))?;
        let page = self.engine.load_interactive_url(&url)?;
        self.page = Some(page);
        self.cursor = Some(index);
        self.scroll_y = 0;
        self.same_document_start = index;
        self.same_document_end = index;
        Ok(())
    }
}

fn multipart_boundary(
    fields: &[crate::interactive::FormField],
    files: &[FileUpload],
) -> String {
    let size = files.iter().map(|file| file.bytes.len()).sum::<usize>();
    format!("----QuanticQ05Boundary{}{}{}", fields.len(), files.len(), size)
}

fn push_text_part(body: &mut Vec<u8>, boundary: &str, name: &str, value: &str) {
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"{}\"\r\n\r\n", escape_header(name))
            .as_bytes(),
    );
    body.extend_from_slice(value.as_bytes());
    body.extend_from_slice(b"\r\n");
}

fn push_file_part(body: &mut Vec<u8>, boundary: &str, file: &FileUpload) {
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n",
            escape_header(&file.field_name),
            escape_header(&file.filename)
        )
        .as_bytes(),
    );
    body.extend_from_slice(
        format!("Content-Type: {}\r\n\r\n", file.content_type).as_bytes(),
    );
    body.extend_from_slice(&file.bytes);
    body.extend_from_slice(b"\r\n");
}

fn encode_multipart(
    boundary: &str,
    fields: &[crate::interactive::FormField],
    files: &[FileUpload],
) -> Vec<u8> {
    let mut body = Vec::new();
    for field in fields {
        push_text_part(&mut body, boundary, &field.name, &field.value);
    }
    for file in files {
        push_file_part(&mut body, boundary, file);
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

fn escape_header(value: &str) -> String {
    value.replace('\r', "").replace('\n', "").replace('"', "%22")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multipart_body_preserves_binary_file_bytes() {
        let fields = vec![crate::interactive::FormField {
            name: "name".into(),
            value: "AURA".into(),
        }];
        let files = vec![FileUpload {
            field_name: "cv".into(),
            filename: "cv.pdf".into(),
            content_type: "application/pdf".into(),
            bytes: vec![0, 1, 2, 255],
        }];
        let boundary = multipart_boundary(&fields, &files);
        let body = encode_multipart(&boundary, &fields, &files);
        assert!(body.windows(4).any(|window| window == [0, 1, 2, 255]));
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("name=\"cv\"; filename=\"cv.pdf\""));
        assert!(text.contains("application/pdf"));
    }


    #[test]
    fn back_and_forward_traverse_pushstate_without_network_reload() {
        let mut session = BrowserSession::new(Engine::new());
        session
            .open_html_with_base(
                "<main id='marker'>history</main>",
                "https://app.example/start",
            )
            .unwrap();
        session
            .execute_script(
                "history.pushState({}, '', '/one'); history.pushState({}, '', '/two');",
            )
            .unwrap();

        assert_eq!(session.current_url(), Some("https://app.example/two"));
        assert!(session.back().unwrap());
        assert_eq!(session.current_url(), Some("https://app.example/one"));
        assert!(session.forward().unwrap());
        assert_eq!(session.current_url(), Some("https://app.example/two"));
        assert!(session
            .current_page()
            .and_then(|page| page.document().find_by_id("marker"))
            .is_some());
    }

    #[test]
    fn history_updates_from_javascript_are_reflected() {
        let mut session = BrowserSession::new(Engine::new());
        session
            .open_html_with_base(
                "<main>history</main>",
                "https://app.example/start",
            )
            .unwrap();

        session
            .execute_script(
                "history.pushState({}, '', '/next'); history.replaceState({}, '', '/final');",
            )
            .unwrap();

        assert_eq!(session.history().len(), 2);
        assert_eq!(session.history()[0].url, "https://app.example/start");
        assert_eq!(session.history()[1].url, "https://app.example/final");
        assert_eq!(session.current_url(), Some("https://app.example/final"));
    }

    #[test]
    fn empty_session_has_no_navigation_state() {
        let session = BrowserSession::new(Engine::new()).with_viewport_height(600);
        assert_eq!(session.current_url(), None);
        assert!(!session.can_go_back());
        assert!(!session.can_go_forward());
        assert_eq!(session.scroll_y(), 0);
    }
}
