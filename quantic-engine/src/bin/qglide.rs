use std::time::{Duration, Instant};

use eframe::egui;
use quantic_engine::{dom::NodeKind, BrowserSession, Engine};

const HOME: &str = "https://search.brave.com/";
const VIEWPORT_WIDTH: u32 = 1180;
const VIEWPORT_HEIGHT: u32 = 760;

#[derive(Clone)]
struct LinkItem {
    id: String,
    text: String,
    href: String,
}

struct GlideApp {
    session: BrowserSession,
    address: String,
    status: String,
    texture: Option<egui::TextureHandle>,
    links: Vec<LinkItem>,
    last_async_poll: Instant,
    dark: bool,
}

impl GlideApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let engine = Engine::new().with_viewport_width(VIEWPORT_WIDTH);
        let session = BrowserSession::new(engine).with_viewport_height(VIEWPORT_HEIGHT);

        let mut app = Self {
            session,
            address: String::new(),
            status: "Quantic Engine Q0.5 · zéro Chromium".to_string(),
            texture: None,
            links: Vec::new(),
            last_async_poll: Instant::now(),
            dark: true,
        };

        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        app
    }

    fn normalize_address(raw: &str) -> String {
        let value = raw.trim();
        if value.is_empty() {
            return HOME.to_string();
        }
        if value.starts_with("http://") || value.starts_with("https://") {
            return value.to_string();
        }
        if value.contains('.') && !value.contains(' ') {
            return format!("https://{value}");
        }
        format!(
            "https://search.brave.com/search?q={}",
            urlencoding::encode(value)
        )
    }

    fn navigate(&mut self, ctx: &egui::Context, raw: &str) {
        let target = Self::normalize_address(raw);
        self.status = format!("Chargement de {target}…");
        match self.session.open(&target) {
            Ok(()) => {
                self.address = self.session.current_url().unwrap_or(&target).to_string();
                self.prepare_page();
                self.refresh_texture(ctx);
                self.status = "Prêt · Quantic Engine natif".to_string();
            }
            Err(error) => {
                self.status = format!("Erreur: {error}");
                self.texture = None;
                self.links.clear();
            }
        }
    }

    fn prepare_page(&mut self) {
        let _ = self.session.execute_script(
            r#"(() => {
                let n = 0;
                document.querySelectorAll('a[href]').forEach((el) => {
                    if (!el.id) el.id = 'qglide-link-' + (++n);
                });
            })();"#,
        );
        self.links = self.extract_links();
    }

    fn extract_links(&self) -> Vec<LinkItem> {
        let Some(page) = self.session.current_page() else {
            return Vec::new();
        };
        let document = page.document();
        let mut links = Vec::new();

        for (node_id, node) in document.nodes.iter().enumerate() {
            let NodeKind::Element { tag, .. } = &node.kind else {
                continue;
            };
            if !tag.eq_ignore_ascii_case("a") {
                continue;
            }
            let id = document.attribute(node_id, "id").unwrap_or_default().to_string();
            let href = document.attribute(node_id, "href").unwrap_or_default().to_string();
            if id.is_empty() || href.is_empty() {
                continue;
            }
            let text = normalize_text(&text_content(document, node_id));
            links.push(LinkItem {
                id,
                text: if text.is_empty() { href.clone() } else { text },
                href,
            });
            if links.len() >= 120 {
                break;
            }
        }

        links
    }

    fn refresh_texture(&mut self, ctx: &egui::Context) {
        let Some(image) = self.session.rasterize_viewport() else {
            self.texture = None;
            return;
        };
        let size = [image.width() as usize, image.height() as usize];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
        self.texture = Some(ctx.load_texture(
            "quantic-page",
            color,
            egui::TextureOptions::LINEAR,
        ));
    }

    fn back(&mut self, ctx: &egui::Context) {
        match self.session.back() {
            Ok(true) => {
                self.address = self.session.current_url().unwrap_or_default().to_string();
                self.prepare_page();
                self.refresh_texture(ctx);
            }
            Ok(false) => {}
            Err(error) => self.status = format!("Retour impossible: {error}"),
        }
    }

    fn forward(&mut self, ctx: &egui::Context) {
        match self.session.forward() {
            Ok(true) => {
                self.address = self.session.current_url().unwrap_or_default().to_string();
                self.prepare_page();
                self.refresh_texture(ctx);
            }
            Ok(false) => {}
            Err(error) => self.status = format!("Avance impossible: {error}"),
        }
    }

    fn reload(&mut self, ctx: &egui::Context) {
        match self.session.reload() {
            Ok(true) => {
                self.address = self.session.current_url().unwrap_or_default().to_string();
                self.prepare_page();
                self.refresh_texture(ctx);
            }
            Ok(false) => {}
            Err(error) => self.status = format!("Rechargement impossible: {error}"),
        }
    }

    fn scroll(&mut self, ctx: &egui::Context, delta: i32) {
        self.session.scroll_by(delta);
        self.refresh_texture(ctx);
    }

    fn open_link(&mut self, ctx: &egui::Context, id: &str) {
        match self.session.click_link(id) {
            Ok(true) => {
                self.address = self.session.current_url().unwrap_or_default().to_string();
                self.prepare_page();
                self.refresh_texture(ctx);
            }
            Ok(false) => self.status = "Lien non navigable".to_string(),
            Err(error) => self.status = format!("Lien: {error}"),
        }
    }

    fn poll_async(&mut self, ctx: &egui::Context) {
        if self.last_async_poll.elapsed() < Duration::from_millis(500) {
            return;
        }
        self.last_async_poll = Instant::now();
        if let Ok(delivered) = self.session.poll_async_events() {
            if delivered > 0 {
                let _ = self.session.follow_pending_navigation();
                self.address = self.session.current_url().unwrap_or_default().to_string();
                self.prepare_page();
                self.refresh_texture(ctx);
            }
        }
    }
}

impl eframe::App for GlideApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_async(ctx);

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("◉ Glide");
                if ui.add_enabled(self.session.can_go_back(), egui::Button::new("←")).clicked() {
                    self.back(ctx);
                }
                if ui.add_enabled(self.session.can_go_forward(), egui::Button::new("→")).clicked() {
                    self.forward(ctx);
                }
                if ui.button("↻").clicked() {
                    self.reload(ctx);
                }
                if ui.button("⌂").clicked() {
                    self.navigate(ctx, HOME);
                }

                let response = ui.add_sized(
                    [ui.available_width() - 190.0, 30.0],
                    egui::TextEdit::singleline(&mut self.address)
                        .hint_text("Adresse ou recherche"),
                );
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let value = self.address.clone();
                    self.navigate(ctx, &value);
                }

                if ui.button("Aller").clicked() {
                    let value = self.address.clone();
                    self.navigate(ctx, &value);
                }

                if ui.button(if self.dark { "☾" } else { "☀" }).clicked() {
                    self.dark = !self.dark;
                    ctx.set_visuals(if self.dark {
                        egui::Visuals::dark()
                    } else {
                        egui::Visuals::light()
                    });
                }
            });
            ui.small(&self.status);
        });

        egui::SidePanel::right("links")
            .default_width(250.0)
            .min_width(180.0)
            .show(ctx, |ui| {
                ui.heading("Liens de la page");
                ui.small("Navigation native Q0.5");
                ui.separator();

                if self.links.is_empty() {
                    ui.label("Aucun lien détecté.");
                } else {
                    let mut selected: Option<String> = None;
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for link in &self.links {
                            let label = if link.text.len() > 60 {
                                format!("{}…", &link.text[..57])
                            } else {
                                link.text.clone()
                            };
                            if ui.button(label).on_hover_text(&link.href).clicked() {
                                selected = Some(link.id.clone());
                            }
                        }
                    });
                    if let Some(id) = selected {
                        self.open_link(ctx, &id);
                    }
                }
            });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "Scroll {}/{}",
                    self.session.scroll_y(),
                    self.session.max_scroll_y()
                ));
                if ui.button("▲").clicked() {
                    self.scroll(ctx, -420);
                }
                if ui.button("▼").clicked() {
                    self.scroll(ctx, 420);
                }
                ui.separator();
                ui.label("Rust + Boa JS + Quantic Engine");
                ui.separator();
                ui.strong("Chromium: 0 · Electron: 0 · CEF: 0");
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(texture) = &self.texture {
                let available = ui.available_size();
                let source = texture.size_vec2();
                let scale = (available.x / source.x).min(1.0).max(0.2);
                let size = source * scale;
                ui.add(egui::Image::new((texture.id(), size)));
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(120.0);
                    ui.heading("Quantic Glide");
                    ui.label("Navigateur natif propulsé par Quantic Engine Q0.5");
                    ui.add_space(12.0);
                    ui.strong("Aucun Chromium · Aucun Electron · Aucun CEF");
                    ui.add_space(18.0);
                    if ui.button("Ouvrir Brave Search").clicked() {
                        self.navigate(ctx, HOME);
                    }
                });
            }
        });

        ctx.request_repaint_after(Duration::from_millis(250));
    }
}

fn text_content(document: &quantic_engine::dom::Document, node_id: usize) -> String {
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

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Quantic Glide — Native")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([900.0, 640.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Quantic Glide — Native",
        options,
        Box::new(|cc| Ok(Box::new(GlideApp::new(cc)))),
    )
}
