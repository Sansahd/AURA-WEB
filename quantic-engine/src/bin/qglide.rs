use std::time::{Duration, Instant};

use eframe::egui;
use quantic_engine::{BrowserSession, Engine};

const VIEWPORT_WIDTH: u32 = 1240;
const VIEWPORT_HEIGHT: u32 = 780;

const SEARCH_PREFIX: &str = "https://html.duckduckgo.com/html/?q=";
const QUANTIC_PORTAL: &str = "https://mediumorchid-badger-314305.hostingersite.com";
const MAIL_URL: &str = "https://mediumorchid-badger-314305.hostingersite.com/mail/";
const PULSE_URL: &str = "https://mediumorchid-badger-314305.hostingersite.com/pulse/";

const BG: egui::Color32 = egui::Color32::from_rgb(13, 12, 11);
const PANEL: egui::Color32 = egui::Color32::from_rgb(24, 22, 20);
const PANEL_HOVER: egui::Color32 = egui::Color32::from_rgb(34, 31, 27);
const TEXT: egui::Color32 = egui::Color32::from_rgb(242, 238, 228);
const MUTED: egui::Color32 = egui::Color32::from_rgb(156, 151, 142);
const ACCENT: egui::Color32 = egui::Color32::from_rgb(242, 177, 52);
const ACCENT_SOFT: egui::Color32 = egui::Color32::from_rgb(74, 55, 22);
const BORDER: egui::Color32 = egui::Color32::from_rgb(53, 49, 44);

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
}

impl GlideApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let engine = Engine::new().with_viewport_width(VIEWPORT_WIDTH);
        let session = BrowserSession::new(engine).with_viewport_height(VIEWPORT_HEIGHT);

        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = BG;
        visuals.window_fill = BG;
        visuals.extreme_bg_color = egui::Color32::from_rgb(18, 17, 15);
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
            status: "Glide Native 0.6.1 prêt".to_string(),
            texture: None,
            last_async_poll: Instant::now(),
            home: true,
            error: None,
            home_query: String::new(),
            editor: None,
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
        self.status = "Accueil Glide".to_string();
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
        egui::TopBottomPanel::top("glide_top")
            .exact_height(84.0)
            .frame(egui::Frame::new().fill(BG))
            .show(ctx, |ui| {
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    let (mark_rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
                    ui.painter().circle_filled(mark_rect.center(), 12.0, ACCENT);
                    ui.painter().circle_filled(mark_rect.center(), 5.0, BG);

                    ui.label(
                        egui::RichText::new("GLIDE")
                            .size(20.0)
                            .strong()
                            .color(TEXT),
                    );

                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("NATIVE")
                            .size(10.0)
                            .strong()
                            .color(ACCENT),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new("ZERO CHROMIUM")
                                .size(10.0)
                                .strong()
                                .color(MUTED),
                        );
                    });
                });

                ui.add_space(7.0);

                ui.horizontal(|ui| {
                    ui.add_space(10.0);

                    let back = ui.add_enabled(
                        !self.home && self.session.can_go_back(),
                        egui::Button::new("←").min_size(egui::vec2(36.0, 32.0)),
                    );
                    if back.clicked() {
                        self.back(ctx);
                    }

                    let forward = ui.add_enabled(
                        !self.home && self.session.can_go_forward(),
                        egui::Button::new("→").min_size(egui::vec2(36.0, 32.0)),
                    );
                    if forward.clicked() {
                        self.forward(ctx);
                    }

                    if ui
                        .add(egui::Button::new("↻").min_size(egui::vec2(36.0, 32.0)))
                        .clicked()
                    {
                        if self.home {
                            self.status = "Accueil Glide".to_string();
                        } else {
                            self.reload(ctx);
                        }
                    }

                    if ui
                        .add(egui::Button::new("⌂").min_size(egui::vec2(36.0, 32.0)))
                        .clicked()
                    {
                        self.go_home();
                    }

                    ui.add_space(6.0);

                    let available = (ui.available_width() - 118.0).max(220.0);
                    let response = ui.add_sized(
                        [available, 34.0],
                        egui::TextEdit::singleline(&mut self.address)
                            .hint_text("Rechercher ou saisir une adresse"),
                    );

                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let value = self.address.clone();
                        self.navigate(ctx, &value);
                    }

                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("Aller").strong().color(BG),
                            )
                            .fill(ACCENT)
                            .min_size(egui::vec2(64.0, 34.0)),
                        )
                        .clicked()
                    {
                        let value = self.address.clone();
                        self.navigate(ctx, &value);
                    }

                    ui.add_space(10.0);
                });
            });
    }

    fn native_rail(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("native_rail")
            .exact_width(68.0)
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(17, 16, 14)))
            .show(ctx, |ui| {
                ui.add_space(18.0);
                ui.vertical_centered(|ui| {
                    if rail_button(ui, "⌕", "Accueil / recherche") {
                        self.go_home();
                    }
                    ui.add_space(10.0);

                    if rail_button(ui, "M", "Quantic Mail") {
                        self.navigate(ctx, MAIL_URL);
                    }
                    ui.add_space(10.0);

                    if rail_button(ui, "P", "Quantic Pulse") {
                        self.navigate(ctx, PULSE_URL);
                    }
                    ui.add_space(10.0);

                    if rail_button(ui, "A", "AURA Career") {
                        self.status = "AURA Career natif est installé avec Glide".to_string();
                    }

                    ui.add_space(18.0);
                    ui.separator();
                    ui.add_space(12.0);

                    if rail_button(ui, "Q", "Portail Quantic") {
                        self.navigate(ctx, QUANTIC_PORTAL);
                    }
                });
            });
    }

    fn home_view(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.add_space(70.0);

        ui.vertical_centered(|ui| {
            let (logo_rect, _) = ui.allocate_exact_size(egui::vec2(92.0, 92.0), egui::Sense::hover());
            ui.painter().circle_filled(logo_rect.center(), 42.0, ACCENT);
            ui.painter().circle_filled(logo_rect.center(), 19.0, BG);

            ui.add_space(16.0);
            ui.label(
                egui::RichText::new("GLIDE")
                    .size(44.0)
                    .strong()
                    .color(TEXT),
            );
            ui.label(
                egui::RichText::new("Le web, sans Chromium.")
                    .size(17.0)
                    .color(MUTED),
            );

            ui.add_space(30.0);

            let search_width = ui.available_width().min(720.0);
            let response = ui.add_sized(
                [search_width, 46.0],
                egui::TextEdit::singleline(&mut self.home_query)
                    .hint_text("Rechercher sur le web ou saisir une adresse"),
            );

            let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

            ui.add_space(10.0);
            let search_clicked = ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("Rechercher")
                            .strong()
                            .color(BG),
                    )
                    .fill(ACCENT)
                    .min_size(egui::vec2(150.0, 38.0)),
                )
                .clicked();

            if enter || search_clicked {
                let value = self.home_query.clone();
                if !value.trim().is_empty() {
                    self.address = value.clone();
                    self.navigate(ctx, &value);
                }
            }

            ui.add_space(42.0);

            ui.horizontal(|ui| {
                let total = 4.0 * 150.0 + 3.0 * 14.0;
                ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));

                if home_card(ui, "WEB", "Recherche privée", "HTML léger") {
                    let value = if self.home_query.trim().is_empty() {
                        "Quantic".to_string()
                    } else {
                        self.home_query.clone()
                    };
                    self.navigate(ctx, &value);
                }
                ui.add_space(14.0);

                if home_card(ui, "MAIL", "Quantic Mail", "Communication") {
                    self.navigate(ctx, MAIL_URL);
                }
                ui.add_space(14.0);

                if home_card(ui, "PULSE", "Quantic Pulse", "Réseau social") {
                    self.navigate(ctx, PULSE_URL);
                }
                ui.add_space(14.0);

                if home_card(ui, "AURA", "AURA Career", "Agent emploi") {
                    self.status = "AURA Career natif est installé avec Glide".to_string();
                }
            });

            ui.add_space(34.0);
            ui.label(
                egui::RichText::new("Rust · Quantic Engine · Boa JS · aucun moteur tiers")
                    .size(11.0)
                    .color(MUTED),
            );
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

        egui::Window::new("Saisie Glide")
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

    fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(28.0)
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(17, 16, 14)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(&self.status).size(10.0).color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("Chromium 0 · Electron 0 · CEF 0")
                                .size(10.0)
                                .strong()
                                .color(ACCENT),
                        );
                    });
                });
            });
    }
}

impl eframe::App for GlideApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_async(ctx);
        self.keyboard_shortcuts(ctx);

        self.top_bar(ctx);
        self.status_bar(ctx);
        self.native_rail(ctx);

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
            .with_title("Quantic Glide Native")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([980.0, 680.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Quantic Glide Native",
        options,
        Box::new(|cc| Ok(Box::new(GlideApp::new(cc)))),
    )
}
