use std::time::{Duration, Instant};

use eframe::egui;
use quantic_engine::{BrowserSession, Engine};

const VIEWPORT_WIDTH: u32 = 1240;
const VIEWPORT_HEIGHT: u32 = 780;

const SEARCH_PREFIX: &str = "https://html.duckduckgo.com/html/?q=";
const QUANTIC_PORTAL: &str = "https://mediumorchid-badger-314305.hostingersite.com";
const MAIL_URL: &str = "https://mediumorchid-badger-314305.hostingersite.com/mail/";
const PULSE_URL: &str = "https://mediumorchid-badger-314305.hostingersite.com/pulse/";

const BG: egui::Color32 = egui::Color32::from_rgb(5, 5, 5);
const PANEL: egui::Color32 = egui::Color32::from_rgb(21, 21, 21);
const PANEL_HOVER: egui::Color32 = egui::Color32::from_rgb(43, 43, 43);
const TEXT: egui::Color32 = egui::Color32::from_rgb(250, 250, 250);
const MUTED: egui::Color32 = egui::Color32::from_rgb(178, 178, 178);
const ACCENT: egui::Color32 = egui::Color32::WHITE;
const ACCENT_SOFT: egui::Color32 = egui::Color32::from_rgb(50, 50, 50);
const BORDER: egui::Color32 = egui::Color32::from_rgb(57, 57, 57);


/// One parked tab owns one independent native Quantic Engine session.
struct ParkedTab {
    session: BrowserSession,
    address: String,
    texture: Option<egui::TextureHandle>,
    home: bool,
    error: Option<String>,
    title: String,
}

fn favorites_file() -> std::path::PathBuf {
    let root = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(std::path::PathBuf::from))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    root.join("GEKKO").join("Native").join("favorites.json")
}

fn load_favorites() -> Vec<String> {
    let Ok(bytes) = std::fs::read_to_string(favorites_file()) else { return Vec::new(); };
    serde_json::from_str::<Vec<String>>(&bytes)
        .unwrap_or_default().into_iter().filter(|url| {
            (url.starts_with("http://") || url.starts_with("https://")) && url.len() < 2048
        }).take(50).collect()
}

fn save_favorites(urls: &[String]) -> std::io::Result<()> {
    let file = favorites_file();
    if let Some(parent) = file.parent() { std::fs::create_dir_all(parent)?; }
    let data = serde_json::to_vec_pretty(urls)?;
    std::fs::write(file, data)
}

/// Minimal vector renderer for the approved GEKKO monochrome mark.
/// The stored SVG uses only polygon moves and straight folds, no filters or gradient.
fn gecko_icon(ui: &mut egui::Ui, side: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.circle_filled(rect.center(), side * 0.49, egui::Color32::BLACK);
    let svg = include_str!("../../assets/gekko-origami-noir.svg");
    if let Some(part) = svg.split(" d=\"").nth(1).and_then(|s| s.split('"').next()) {
        for command in part.split('M').skip(1) {
            let mut points = Vec::new();
            for item in command.replace('Z', " ").split_whitespace() {
                let Some((x, y)) = item.trim_start_matches('L').split_once(',') else { continue; };
                if let (Ok(x), Ok(y)) = (x.parse::<f32>(), y.parse::<f32>()) {
                    points.push(egui::pos2(
                        rect.min.x + side * x / 512.0,
                        rect.min.y + side * y / 512.0,
                    ));
                }
            }
            if points.len() >= 3 {
                painter.add(egui::Shape::convex_polygon(points, egui::Color32::WHITE, egui::Stroke::NONE));
            }
        }
    }
    response
}

#[derive(Clone)]
struct FieldEditor {
    element_id: String,
    label: String,
    value: String,
    multiline: bool,
}

struct GlideApp {
    session: BrowserSession,
    address: String,
    status: String,
    texture: Option<egui::TextureHandle>,
    last_async_poll: Instant,
    home: bool,
    error: Option<String>,
    home_query: String,
    editor: Option<FieldEditor>,
    tabs: Vec<Option<ParkedTab>>,
    active_tab: usize,
    favorites: Vec<String>,
    maximized: bool,
    show_apps: bool,
}

impl GlideApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let engine = Engine::new().with_viewport_width(VIEWPORT_WIDTH);
        let session = BrowserSession::new(engine).with_viewport_height(VIEWPORT_HEIGHT);

        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = BG;
        visuals.window_fill = BG;
        visuals.extreme_bg_color = egui::Color32::from_rgb(16, 16, 16);
        visuals.faint_bg_color = PANEL;
        visuals.override_text_color = Some(TEXT);
        visuals.widgets.inactive.bg_fill = PANEL;
        visuals.widgets.hovered.bg_fill = PANEL_HOVER;
        visuals.widgets.active.bg_fill = ACCENT_SOFT;
        visuals.widgets.inactive.fg_stroke.color = TEXT;
        visuals.widgets.hovered.fg_stroke.color = TEXT;
        visuals.widgets.active.fg_stroke.color = ACCENT;
        cc.egui_ctx.set_visuals(visuals);

        Self {
            session,
            address: String::new(),
            status: "GEKKO NOIR — moteur natif expérimental · zéro Chromium".to_string(),
            texture: None,
            last_async_poll: Instant::now(),
            home: true,
            error: None,
            home_query: String::new(),
            editor: None,
            tabs: vec![None],
            active_tab: 0,
            favorites: load_favorites(),
            maximized: false,
            show_apps: false,
        }
    }


    fn fresh_session() -> BrowserSession {
        BrowserSession::new(Engine::new().with_viewport_width(VIEWPORT_WIDTH))
            .with_viewport_height(VIEWPORT_HEIGHT)
    }

    fn tab_label(&self, i: usize) -> String {
        let url = if i == self.active_tab {
            if self.home { return "Accueil".to_string(); }
            self.address.as_str()
        } else {
            let Some(Some(tab)) = self.tabs.get(i) else { return "Accueil".to_string(); };
            if tab.home { return "Accueil".to_string(); }
            tab.address.as_str()
        };
        url::Url::parse(url).ok()
            .and_then(|u| u.host_str().map(str::to_string))
            .unwrap_or_else(|| "Nouvel onglet".to_string())
    }

    fn switch_tab(&mut self, index: usize) {
        if index >= self.tabs.len() || index == self.active_tab { return; }
        let old = ParkedTab {
            session: std::mem::replace(&mut self.session, Self::fresh_session()),
            address: std::mem::take(&mut self.address),
            texture: self.texture.take(),
            home: self.home,
            error: self.error.take(),
            title: self.status.clone(),
        };
        self.tabs[self.active_tab] = Some(old);
        self.active_tab = index;
        match self.tabs[index].take() {
            Some(tab) => {
                self.session = tab.session;
                self.address = tab.address;
                self.texture = tab.texture;
                self.home = tab.home;
                self.error = tab.error;
                self.status = tab.title;
            }
            None => self.go_home(),
        }
        self.editor = None;
    }

    fn new_tab(&mut self) {
        let next = self.tabs.len();
        self.tabs.push(None);
        self.switch_tab(next);
    }

    fn close_tab(&mut self, index: usize) {
        if index >= self.tabs.len() { return; }
        if self.tabs.len() == 1 { self.go_home(); return; }
        if index == self.active_tab {
            self.switch_tab(if index == 0 { 1 } else { index - 1 });
        }
        self.tabs.remove(index);
        if self.active_tab > index { self.active_tab -= 1; }
    }

    fn toggle_favorite(&mut self) {
        if self.home || !self.address.starts_with("http") { return; }
        if let Some(i) = self.favorites.iter().position(|u| u == &self.address) {
            self.favorites.remove(i);
        } else {
            self.favorites.insert(0, self.address.clone());
            self.favorites.truncate(50);
        }
        if let Err(error) = save_favorites(&self.favorites) {
            self.status = format!("Favoris : sauvegarde impossible ({error})");
        }
    }

    fn normalize_address(raw: &str) -> String {
        let value = raw.trim();
        if value.is_empty() {
            return String::new();
        }
        if value.starts_with("http://") || value.starts_with("https://") {
            return value.to_string();
        }
        if value.contains('.') && !value.contains(' ') {
            return format!("https://{value}");
        }
        format!("{SEARCH_PREFIX}{}", urlencoding::encode(value))
    }

    fn go_home(&mut self) {
        self.home = true;
        self.error = None;
        self.texture = None;
        self.address.clear();
        self.status = "Accueil GEKKO".to_string();
    }

    fn navigate(&mut self, ctx: &egui::Context, raw: &str) {
        let target = Self::normalize_address(raw);
        if target.is_empty() {
            self.go_home();
            return;
        }

        self.home = false;
        self.error = None;
        self.status = "Chargement…".to_string();

        match self.session.open(&target) {
            Ok(()) => {
                self.address = self.session.current_url().unwrap_or(&target).to_string();
                self.prepare_page();
                self.refresh_texture(ctx);
                self.status = host_label(&self.address);
            }
            Err(error) => {
                self.address = target;
                self.texture = None;
                self.error = Some(error.to_string());
                self.status = "Chargement impossible".to_string();
            }
        }
    }

    fn prepare_page(&mut self) {
        let _ = self.session.execute_script(
            r#"(() => {
                let n = 0;
                document.querySelectorAll('a[href],button,input,textarea,select,form').forEach((el) => {
                    if (!el.id) el.id = 'qglide-interactive-' + (++n);
                });
            })();"#,
        );
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

    fn refresh_after_navigation(&mut self, ctx: &egui::Context) {
        self.address = self.session.current_url().unwrap_or_default().to_string();
        self.prepare_page();
        self.refresh_texture(ctx);
        self.error = None;
        self.status = host_label(&self.address);
    }

    fn back(&mut self, ctx: &egui::Context) {
        match self.session.back() {
            Ok(true) => self.refresh_after_navigation(ctx),
            Ok(false) => {}
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn forward(&mut self, ctx: &egui::Context) {
        match self.session.forward() {
            Ok(true) => self.refresh_after_navigation(ctx),
            Ok(false) => {}
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn reload(&mut self, ctx: &egui::Context) {
        match self.session.reload() {
            Ok(true) => self.refresh_after_navigation(ctx),
            Ok(false) => {}
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn scroll(&mut self, ctx: &egui::Context, delta: i32) {
        self.session.scroll_by(delta);
        self.refresh_texture(ctx);
    }

    fn activate_page_at(&mut self, ctx: &egui::Context, x: u32, y: u32) {
        match self.session.activate_at(x, y) {
            Ok(Some(target)) => {
                if matches!(target.tag.as_str(), "a" | "button") {
                    self.refresh_after_navigation(ctx);
                } else if matches!(target.tag.as_str(), "input" | "textarea") {
                    self.editor = Some(FieldEditor {
                        element_id: target.element_id.clone(),
                        label: if target.placeholder.is_empty() {
                            if target.text.is_empty() { "Saisie".to_string() } else { target.text.clone() }
                        } else {
                            target.placeholder.clone()
                        },
                        value: target.value.clone(),
                        multiline: target.tag == "textarea",
                    });
                    self.status = "Saisie native".to_string();
                }
            }
            Ok(None) => {}
            Err(error) => {
                self.error = Some(error.to_string());
                self.status = "Interaction impossible".to_string();
            }
        }
    }

    fn poll_async(&mut self, ctx: &egui::Context) {
        if self.home || self.last_async_poll.elapsed() < Duration::from_millis(550) {
            return;
        }
        self.last_async_poll = Instant::now();
        if let Ok(delivered) = self.session.poll_async_events() {
            if delivered > 0 {
                let navigated = self.session.follow_pending_navigation().unwrap_or(false);
                if navigated {
                    self.refresh_after_navigation(ctx);
                } else {
                    self.refresh_texture(ctx);
                }
            }
        }
    }

    fn keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::T)) { self.new_tab(); }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::W)) { self.close_tab(self.active_tab); }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::L)) {
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("gekko-native-address")));
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F5)) {
            self.reload(ctx);
        }
        if ctx.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::ArrowLeft)) {
            self.back(ctx);
        }
        if ctx.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::ArrowRight)) {
            self.forward(ctx);
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::R)) {
            self.reload(ctx);
        }
    }

    fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("gekko_noir_dock")
            .exact_height(65.0)
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(8,8,8))
                .stroke(egui::Stroke::new(1.0, BORDER)))
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 5.0;
                    let can_back = !self.home && self.session.can_go_back();
                    let can_forward = !self.home && self.session.can_go_forward();
                    if ui.add_enabled(can_back, egui::Button::new("‹")
                        .min_size(egui::vec2(36.0,42.0))).clicked() { self.back(ctx); }
                    if ui.add_enabled(can_forward, egui::Button::new("›")
                        .min_size(egui::vec2(36.0,42.0))).clicked() { self.forward(ctx); }
                    if ui.add(egui::Button::new("⟳").min_size(egui::vec2(36.0,42.0))).clicked() {
                        if !self.home { self.reload(ctx); }
                    }
                    if ui.add(egui::Button::new("⌂").min_size(egui::vec2(36.0,42.0))).clicked() {
                        self.go_home();
                    }

                    // Origami identity stays completely native: no Chromium WebView.
                    gecko_icon(ui, 39.0);

                    let reserved = 184.0; // New tab + window controls.
                    let free = (ui.available_width() - reserved).max(170.0);
                    let search_width = (free * 0.48).clamp(180.0, 420.0);
                    let address = ui.add_sized(
                        [search_width, 40.0],
                        egui::TextEdit::singleline(&mut self.address)
                            .id(egui::Id::new("gekko-native-address"))
                            .hint_text("Rechercher ou saisir une adresse…"),
                    );
                    if address.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.navigate(ctx, &self.address.clone());
                    }
                    if ui.add(egui::Button::new("→").min_size(egui::vec2(34.0,40.0))).clicked() {
                        self.navigate(ctx, &self.address.clone());
                    }
                    if ui.add(egui::Button::new(if self.favorites.contains(&self.address) {"♥"} else {"♡"})
                        .min_size(egui::vec2(35.0,40.0)))
                        .on_hover_text("Ajouter / retirer des favoris").clicked() {
                        self.toggle_favorite();
                    }
                    let tabs_width = (ui.available_width() - reserved).max(75.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(tabs_width, 44.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            egui::ScrollArea::horizontal()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let mut selected = None;
                                        let mut closed = None;
                                        for i in 0..self.tabs.len() {
                                            let title = self.tab_label(i);
                                            let current = i == self.active_tab;
                                            let label = format!("{}{}", if current {"◉ "} else {"○ "}, title);
                                            let button = egui::Button::new(egui::RichText::new(label).size(13.0))
                                                .fill(if current {egui::Color32::from_rgb(49,49,49)} else {PANEL})
                                                .stroke(egui::Stroke::new(1.0, if current {TEXT} else {BORDER}))
                                                .min_size(egui::vec2(153.0,42.0));
                                            if ui.add(button).on_hover_text(self.tab_label(i)).clicked() {
                                                selected = Some(i);
                                            }
                                            if ui.add(egui::Button::new("×").min_size(egui::vec2(22.0,42.0)))
                                                .on_hover_text("Fermer l'onglet").clicked() { closed = Some(i); }
                                        }
                                        if let Some(i) = selected { self.switch_tab(i); }
                                        if let Some(i) = closed { self.close_tab(i); }
                                    });
                                });
                        },
                    );
                    if ui.add(egui::Button::new("+").min_size(egui::vec2(40.0,42.0)))
                        .on_hover_text("Nouvel onglet · Ctrl+T").clicked() { self.new_tab(); }
                    if ui.add(egui::Button::new("−").min_size(egui::vec2(40.0,42.0)))
                        .on_hover_text("Réduire").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                    if ui.add(egui::Button::new("□").min_size(egui::vec2(40.0,42.0)))
                        .on_hover_text("Agrandir / restaurer").clicked() {
                        self.maximized = !self.maximized;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(self.maximized));
                    }
                    if ui.add(egui::Button::new("×").min_size(egui::vec2(40.0,42.0))
                        .fill(egui::Color32::from_rgb(26,26,26)))
                        .on_hover_text("Fermer GEKKO").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
    }

    fn home_view(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.add_space((ui.available_height() * 0.17).clamp(28.0, 150.0));
        ui.vertical_centered(|ui| {
            gecko_icon(ui, 132.0);
            ui.add_space(11.0);
            ui.label(egui::RichText::new("G E K K O").size(47.0).strong().color(TEXT));
            ui.add_space(0.0);
            ui.label(egui::RichText::new("B R O W S E   F R E E L Y")
                .size(10.0).strong().color(MUTED));
            ui.add_space(30.0);
            let search_width = ui.available_width().min(690.0).max(200.0);
            let response = ui.add_sized(
                [search_width, 55.0],
                egui::TextEdit::singleline(&mut self.home_query)
                    .hint_text("⌕  Rechercher ou saisir une adresse…"),
            );
            let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if enter {
                let value = self.home_query.clone();
                self.navigate(ctx, &value);
            }
            ui.add_space(23.0);
            ui.horizontal(|ui| {
                let centered = ((ui.available_width() - search_width) * 0.5).max(0.0);
                ui.add_space(centered);
                ui.label(egui::RichText::new("MES FAVORIS").size(12.0).strong().color(TEXT));
                ui.add_space(10.0);
                ui.label(egui::RichText::new("· Stockés sur ce PC").size(11.0).color(MUTED));
            });
            ui.add_space(8.0);
            if self.favorites.is_empty() {
                ui.label(egui::RichText::new("Ajoute une page avec ♡ dans la barre.")
                    .size(13.0).color(MUTED));
            } else {
                let items = self.favorites.clone();
                egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(8.0,8.0);
                        for url in items.iter().take(12) {
                            let title = host_label(url);
                            let button = egui::Button::new(egui::RichText::new(title).size(12.0))
                                .min_size(egui::vec2(148.0,55.0))
                                .fill(PANEL)
                                .stroke(egui::Stroke::new(1.0,BORDER));
                            if ui.add(button).on_hover_text(url).clicked() {
                                self.navigate(ctx, url);
                            }
                        }
                    });
                });
            }
        });
    }

    fn error_view(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, error: &str) {
        ui.add_space(90.0);
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("Impossible d’afficher cette page")
                    .size(28.0)
                    .strong()
                    .color(TEXT),
            );
            ui.add_space(10.0);
            ui.label(
                egui::RichText::new("Quantic Engine a arrêté le chargement proprement.")
                    .size(15.0)
                    .color(MUTED),
            );
            ui.add_space(20.0);
            ui.label(
                egui::RichText::new(error)
                    .size(12.0)
                    .color(egui::Color32::from_rgb(220, 112, 95)),
            );
            ui.add_space(24.0);
            ui.horizontal(|ui| {
                if ui.button("Réessayer").clicked() {
                    let value = self.address.clone();
                    self.navigate(ctx, &value);
                }
                if ui.button("Retour à l’accueil").clicked() {
                    self.go_home();
                }
            });
        });
    }

    fn page_view(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let Some(texture) = self.texture.as_ref() else {
            let error = self
                .error
                .clone()
                .unwrap_or_else(|| "Aucun rendu disponible".to_string());
            self.error_view(ctx, ui, &error);
            return;
        };

        let available = ui.available_size();
        let source = texture.size_vec2();
        let scale = (available.x / source.x).min(1.0).max(0.25);
        let size = source * scale;

        let response = ui.add(
            egui::Image::new((texture.id(), size))
                .sense(egui::Sense::click()),
        );

        if response.hovered() {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.1 {
                let delta = (-scroll * 2.2) as i32;
                self.scroll(ctx, delta);
            }

            if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                if response.rect.contains(pos) {
                    let x = ((pos.x - response.rect.min.x) / scale)
                        .clamp(0.0, source.x - 1.0) as u32;
                    let y = ((pos.y - response.rect.min.y) / scale)
                        .clamp(0.0, source.y - 1.0) as u32;
                    if let Some(target) = self.session.hit_test(x, y) {
                        if matches!(target.tag.as_str(), "a" | "button") {
                            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                        } else if matches!(target.tag.as_str(), "input" | "textarea") {
                            ctx.set_cursor_icon(egui::CursorIcon::Text);
                        }
                    }
                }
            }
        }

        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let x = ((pos.x - response.rect.min.x) / scale)
                    .clamp(0.0, source.x - 1.0) as u32;
                let y = ((pos.y - response.rect.min.y) / scale)
                    .clamp(0.0, source.y - 1.0) as u32;
                self.activate_page_at(ctx, x, y);
            }
        }
    }

    fn field_editor(&mut self, ctx: &egui::Context) {
        let Some(mut editor) = self.editor.clone() else {
            return;
        };

        let mut keep_open = true;
        let mut save = false;

        egui::Window::new("Saisie GEKKO")
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(&editor.label)
                        .size(12.0)
                        .color(MUTED),
                );
                ui.add_space(8.0);

                let response = if editor.multiline {
                    ui.add_sized(
                        [430.0, 120.0],
                        egui::TextEdit::multiline(&mut editor.value),
                    )
                } else {
                    ui.add_sized(
                        [430.0, 36.0],
                        egui::TextEdit::singleline(&mut editor.value),
                    )
                };

                let enter = !editor.multiline
                    && response.lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter));

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("Valider").strong().color(BG),
                            )
                            .fill(ACCENT),
                        )
                        .clicked()
                        || enter
                    {
                        save = true;
                    }

                    if ui.button("Annuler").clicked() {
                        keep_open = false;
                    }
                });
            });

        if save {
            match self.session.set_value(&editor.element_id, &editor.value) {
                Ok(()) => {
                    self.editor = None;
                    self.refresh_texture(ctx);
                    self.status = "Champ mis à jour".to_string();
                }
                Err(error) => {
                    self.error = Some(error.to_string());
                    self.editor = None;
                }
            }
        } else if keep_open {
            self.editor = Some(editor);
        } else {
            self.editor = None;
        }
    }


}

impl eframe::App for GlideApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_async(ctx);
        self.keyboard_shortcuts(ctx);

        self.top_bar(ctx);
        // Single top dock: no duplicate SideStage and no hidden Chromium surface.

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG))
            .show(ctx, |ui| {
                if self.home {
                    self.home_view(ctx, ui);
                } else if let Some(error) = self.error.clone() {
                    self.error_view(ctx, ui, &error);
                } else {
                    self.page_view(ctx, ui);
                }
            });

        self.field_editor(ctx);
        ctx.request_repaint_after(Duration::from_millis(250));
    }
}

fn rail_button(ui: &mut egui::Ui, label: &str, tooltip: &str) -> bool {
    let response = ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .size(13.0)
                .strong()
                .color(TEXT),
        )
        .fill(PANEL)
        .min_size(egui::vec2(42.0, 42.0)),
    );
    response.on_hover_text(tooltip).clicked()
}

fn home_card(ui: &mut egui::Ui, code: &str, title: &str, subtitle: &str) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(150.0, 104.0), egui::Sense::click());

    let fill = if response.hovered() { PANEL_HOVER } else { PANEL };
    ui.painter().rect_filled(rect, 12.0, fill);
    ui.painter().rect_stroke(
        rect,
        12.0,
        egui::Stroke::new(1.0, if response.hovered() { ACCENT } else { BORDER }),
        egui::StrokeKind::Inside,
    );

    ui.painter().text(
        rect.left_top() + egui::vec2(14.0, 16.0),
        egui::Align2::LEFT_TOP,
        code,
        egui::FontId::proportional(11.0),
        ACCENT,
    );

    ui.painter().text(
        rect.left_top() + egui::vec2(14.0, 46.0),
        egui::Align2::LEFT_TOP,
        title,
        egui::FontId::proportional(15.0),
        TEXT,
    );

    ui.painter().text(
        rect.left_top() + egui::vec2(14.0, 72.0),
        egui::Align2::LEFT_TOP,
        subtitle,
        egui::FontId::proportional(10.5),
        MUTED,
    );

    response.clicked()
}

fn host_label(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .unwrap_or_else(|| "Quantic Engine".to_string())
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("GEKKO NOIR — Native")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([980.0, 680.0])
            .with_decorations(false),
        ..Default::default()
    };

    eframe::run_native(
        "Quantic Glide Native",
        options,
        Box::new(|cc| Ok(Box::new(GlideApp::new(cc)))),
    )
}


#[cfg(test)]
mod native_noir_tests {
    use super::*;
    #[test]
    fn native_search_never_opens_a_chromium_fallback() {
        assert_eq!(GlideApp::normalize_address("example.com"), "https://example.com");
        assert!(GlideApp::normalize_address("bonjour").starts_with(SEARCH_PREFIX));
    }
    #[test]
    fn monochrome_mark_has_no_gradient_or_external_url() {
        let icon = include_str!("../../assets/gekko-origami-noir.svg");
        assert!(icon.contains("fill=\"#000\""));
        assert!(icon.contains("fill=\"#fff\""));
        assert!(!icon.contains("linearGradient"));
    }
}
