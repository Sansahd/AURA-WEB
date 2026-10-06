use std::cell::{Cell, RefCell};
use std::error::Error;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use egui::{LayerId, PaintCallback};
use egui_glow::{CallbackFn, EguiGlow};
use euclid::{Point2D, Rect, Scale, Size2D};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use servo::{
    Code, CompositionEvent, CompositionState, ImeEvent, InputEvent, Key as ServoKey, KeyState,
    KeyboardEvent, LoadStatus, Location, Modifiers as ServoModifiers,
    MouseButton as ServoMouseButton, MouseButtonAction, MouseButtonEvent, MouseLeftViewportEvent,
    MouseMoveEvent, NamedKey as ServoNamedKey, NavigationRequest, OffscreenRenderingContext,
    PermissionRequest, RenderingContext, Servo, ServoBuilder, WebResourceLoad, WebResourceResponse,
    WebView, WebViewBuilder, WheelDelta, WheelEvent, WheelMode, WindowRenderingContext,
};
use embedder_traits::EventLoopWaker;
use url::Url;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey as WinitNamedKey};
use winit::window::Window;

const START_URL: &str = "https://mediumorchid-badger-314305.hostingersite.com";
const QUANTIC_PORTAL: &str = START_URL;
const SEARCH_PREFIX: &str = "https://duckduckgo.com/?q=";
const MAIL_URL: &str = "https://mediumorchid-badger-314305.hostingersite.com/mail/";
const PULSE_URL: &str = "https://mediumorchid-badger-314305.hostingersite.com/pulse/";

const BG: egui::Color32 = egui::Color32::from_rgb(10, 10, 11);
const PANEL: egui::Color32 = egui::Color32::from_rgba_premultiplied(29, 27, 25, 246);
const PANEL_SOFT: egui::Color32 = egui::Color32::from_rgba_premultiplied(43, 39, 34, 236);
const TEXT: egui::Color32 = egui::Color32::from_rgb(247, 244, 238);
const MUTED: egui::Color32 = egui::Color32::from_rgb(158, 153, 145);
const ACCENT: egui::Color32 = egui::Color32::from_rgb(246, 181, 64);
const ACCENT_SOFT: egui::Color32 = egui::Color32::from_rgb(105, 72, 25);
const BORDER: egui::Color32 = egui::Color32::from_rgb(76, 67, 55);

#[derive(Default, Serialize, Deserialize)]
struct BrowserData {
    favorites: Vec<String>,
    history: Vec<String>,
}

fn browser_data_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|root| root.join("Quantic").join("Glide").join("browser-data.json"))
}

fn load_browser_data() -> BrowserData {
    browser_data_path()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_browser_data(data: &BrowserData) {
    let Some(path) = browser_data_path() else { return; };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(raw) = serde_json::to_string_pretty(data) {
        let _ = fs::write(path, raw);
    }
}


const TRACKER_HOSTS: &[&str] = &[
    "doubleclick.net",
    "google-analytics.com",
    "googletagmanager.com",
    "adservice.google.com",
    "connect.facebook.net",
    "facebook.net",
    "hotjar.com",
    "clarity.ms",
    "segment.com",
    "segment.io",
    "scorecardresearch.com",
    "quantserve.com",
];

fn host_matches(host: &str, domain: &str) -> bool {
    host == domain ||
        host
            .strip_suffix(domain)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

fn is_known_tracker(url: &Url) -> bool {
    let Some(host) = url.host_str().map(str::to_ascii_lowercase) else {
        return false;
    };
    TRACKER_HOSTS.iter().any(|domain| host_matches(&host, domain))
}

fn navigation_scheme_allowed(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https" | "about" | "data" | "blob")
}

#[derive(Clone)]
struct Waker(EventLoopProxy<WakeEvent>);

#[derive(Debug)]
struct WakeEvent;

impl EventLoopWaker for Waker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(self.clone())
    }

    fn wake(&self) {
        let _ = self.0.send_event(WakeEvent);
    }
}

struct FusionState {
    window: Window,
    servo: Servo,
    webview: WebView,
    window_context: Rc<WindowRenderingContext>,
    web_context: Rc<OffscreenRenderingContext>,
    egui: RefCell<EguiGlow>,
    dock_input: RefCell<String>,
    current_url: RefCell<String>,
    status: RefCell<String>,
    dock_expanded: Cell<bool>,
    dock_focus_requested: Cell<bool>,
    content_height_points: Cell<f32>,
    cursor_point: Cell<(f32, f32)>,
    modifiers_state: Cell<ModifiersState>,
    blocked_resources: Cell<u64>,
    permissions_denied: Cell<u64>,
    quick_panel_open: Cell<bool>,
    browser_data: RefCell<BrowserData>,
    home_open: Cell<bool>,
}

struct FusionDelegate {
    state: RefCell<Weak<FusionState>>,
}

impl FusionDelegate {
    fn new() -> Self {
        Self {
            state: RefCell::new(Weak::new()),
        }
    }

    fn bind(&self, state: &Rc<FusionState>) {
        *self.state.borrow_mut() = Rc::downgrade(state);
    }

    fn with_state(&self, callback: impl FnOnce(&FusionState)) {
        if let Some(state) = self.state.borrow().upgrade() {
            callback(&state);
        }
    }
}

impl servo::WebViewDelegate for FusionDelegate {
    fn notify_new_frame_ready(&self, _webview: WebView) {
        self.with_state(|state| state.window.request_redraw());
    }

    fn notify_url_changed(&self, _webview: WebView, url: Url) {
        self.with_state(|state| {
            *state.current_url.borrow_mut() = url.to_string();
            state.home_open.set(false);
            if matches!(url.scheme(), "http" | "https") {
                let mut data = state.browser_data.borrow_mut();
                let value = url.to_string();
                data.history.retain(|item| item != &value);
                data.history.insert(0, value);
                data.history.truncate(100);
                save_browser_data(&data);
            }
            if !state.egui.borrow().egui_ctx.memory(|memory| memory.focused().is_some()) {
                *state.dock_input.borrow_mut() = url.to_string();
            }
            state.window.request_redraw();
        });
    }

    fn notify_page_title_changed(&self, _webview: WebView, title: Option<String>) {
        self.with_state(|state| {
            let title = title
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "Glide".into());
            state
                .window
                .set_title(&format!("{title} — Quantic Glide Fusion"));
        });
    }

    fn notify_load_status_changed(&self, _webview: WebView, load_status: LoadStatus) {
        self.with_state(|state| {
            *state.status.borrow_mut() = match load_status {
                LoadStatus::Started => "Chargement…".into(),
                LoadStatus::HeadParsed => "Rendu…".into(),
                LoadStatus::Complete => "Prêt".into(),
            };
            state.window.request_redraw();
        });
    }

    fn notify_crashed(&self, _webview: WebView, reason: String, _backtrace: Option<String>) {
        self.with_state(|state| {
            *state.status.borrow_mut() = format!("Page interrompue · {reason}");
            state.window.request_redraw();
        });
    }

    fn request_permission(&self, _webview: WebView, request: PermissionRequest) {
        self.with_state(|state| {
            state.permissions_denied.set(state.permissions_denied.get() + 1);
            *state.status.borrow_mut() = format!("Permission bloquée · {:?}", request.feature());
            state.window.request_redraw();
        });
        request.deny();
    }

    fn request_navigation(&self, _webview: WebView, request: NavigationRequest) {
        if navigation_scheme_allowed(&request.url) {
            request.allow();
        } else {
            self.with_state(|state| {
                *state.status.borrow_mut() =
                    format!("Navigation externe bloquée · {}", request.url.scheme());
                state.window.request_redraw();
            });
            request.deny();
        }
    }

    fn load_web_resource(&self, _webview: WebView, load: WebResourceLoad) {
        if !load.request.is_for_main_frame && is_known_tracker(&load.request.url) {
            self.with_state(|state| {
                state.blocked_resources.set(state.blocked_resources.get() + 1);
                *state.status.borrow_mut() = format!(
                    "Traqueur bloqué · {}",
                    load.request.url.host_str().unwrap_or("ressource tierce")
                );
                state.window.request_redraw();
            });

            let response = WebResourceResponse::new(load.request.url.clone())
                .status_code(http::StatusCode::NO_CONTENT)
                .status_message(b"Blocked by Quantic Glide".to_vec());
            load.intercept(response).finish();
        }
    }
}

impl FusionState {
    fn normalize_target(raw: &str) -> Option<Url> {
        let value = raw.trim();
        if value.is_empty() {
            return None;
        }
        if let Ok(url) = Url::parse(value) {
            if matches!(url.scheme(), "http" | "https") {
                return Some(url);
            }
        }
        if value.contains('.') && !value.contains(' ') {
            return Url::parse(&format!("https://{value}")).ok();
        }
        Url::parse(&format!("{SEARCH_PREFIX}{}", urlencoding::encode(value))).ok()
    }

    fn execute_dock(&self) {
        let raw = self.dock_input.borrow().trim().to_string();
        if raw.is_empty() {
            return;
        }
        let lower = raw.to_ascii_lowercase();

        if lower == "@mail" || lower.starts_with("@mail ") {
            if let Ok(url) = Url::parse(MAIL_URL) {
                self.webview.load(url);
            }
        } else if lower == "@pulse" || lower.starts_with("@pulse ") {
            if let Ok(url) = Url::parse(PULSE_URL) {
                self.webview.load(url);
            }
        } else if lower == "@quantic" || lower.starts_with("@quantic ") {
            if let Ok(url) = Url::parse(QUANTIC_PORTAL) {
                self.webview.load(url);
            }
        } else if lower == "@aura" || lower.starts_with("@aura ") {
            let prompt = raw.strip_prefix("@aura").unwrap_or("").trim();
            *self.status.borrow_mut() = if prompt.is_empty() {
                "AURA prête".into()
            } else {
                format!("AURA · {prompt}")
            };
        } else if matches!(lower.as_str(), "> accueil" | "> home") {
            self.home_open.set(true);
            *self.status.borrow_mut() = "Accueil Glide".into();
            self.window.request_redraw();
        } else if matches!(lower.as_str(), "> retour" | "> back") {
            self.webview.go_back(1);
        } else if matches!(lower.as_str(), "> avance" | "> forward") {
            self.webview.go_forward(1);
        } else if matches!(lower.as_str(), "> recharger" | "> reload") {
            self.webview.reload();
        } else if let Some(url) = Self::normalize_target(&raw) {
            self.webview.load(url);
        }

        self.dock_expanded.set(false);
        self.servo.spin_event_loop();
        self.window.request_redraw();
    }

    fn draw(&self) {
        let _ = self.window_context.make_current();
        let mut egui = self.egui.borrow_mut();
        let mut execute = false;

        egui.run(&self.window, |ctx| {
            let mut visuals = egui::Visuals::dark();
            visuals.panel_fill = BG;
            visuals.window_fill = BG;
            visuals.override_text_color = Some(TEXT);
            visuals.widgets.inactive.bg_fill = PANEL;
            visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(18);
            visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(55, 48, 39);
            visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.2, TEXT);
            visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(18);
            visuals.widgets.active.bg_fill = ACCENT_SOFT;
            visuals.widgets.active.fg_stroke = egui::Stroke::new(1.4, ACCENT);
            visuals.widgets.active.corner_radius = egui::CornerRadius::same(18);
            visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(18);
            visuals.selection.bg_fill = ACCENT_SOFT;
            visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT);
            ctx.set_visuals(visuals);

            egui::TopBottomPanel::top("fusion_header")
                .exact_height(54.0)
                .frame(
                    egui::Frame::new()
                        .fill(BG)
                        .inner_margin(egui::Margin::symmetric(14, 8))
                )
                .show(ctx, |ui| {
                    ui.horizontal_centered(|ui| {
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), 13.0, ACCENT);
                        ui.painter().circle_filled(rect.center(), 8.5, egui::Color32::from_rgb(34, 27, 18));
                        ui.painter().circle_filled(rect.center() + egui::vec2(3.0, -3.0), 3.0, egui::Color32::from_rgb(255, 221, 145));

                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("GLIDE").strong().size(15.0).color(TEXT));
                        ui.label(egui::RichText::new("FUSION").strong().size(10.0).color(ACCENT));

                        ui.add_space(18.0);
                        egui::Frame::new()
                            .fill(PANEL)
                            .stroke(egui::Stroke::new(1.0, BORDER))
                            .corner_radius(16.0)
                            .inner_margin(egui::Margin::symmetric(14, 6))
                            .show(ui, |ui| {
                                let title = self.current_url.borrow();
                                let display = Url::parse(&title)
                                    .ok()
                                    .and_then(|url| url.host_str().map(str::to_string))
                                    .unwrap_or_else(|| "Navigation privée".into());
                                ui.label(egui::RichText::new(display).size(10.5).color(MUTED));
                            });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(
                                egui::Button::new(egui::RichText::new("A").strong().color(TEXT))
                                    .fill(ACCENT_SOFT)
                                    .corner_radius(16.0)
                                    .min_size(egui::vec2(32.0, 32.0))
                            ).on_hover_text("AURA").clicked() {
                                *self.dock_input.borrow_mut() = "@aura ".to_string();
                                self.dock_expanded.set(true);
                                self.dock_focus_requested.set(true);
                            }
                            ui.add_space(5.0);
                            if ui.add(
                                egui::Button::new(egui::RichText::new("☰").size(15.0).color(TEXT))
                                    .fill(PANEL_SOFT)
                                    .corner_radius(16.0)
                                    .min_size(egui::vec2(32.0, 32.0))
                            ).on_hover_text("Services Quantic").clicked() {
                                self.quick_panel_open.set(!self.quick_panel_open.get());
                            }
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new("● privé").size(9.0).color(MUTED));
                        });
                    });
                });

            if self.quick_panel_open.get() {
                egui::SidePanel::right("quantic_panel")
                    .exact_width(250.0)
                    .frame(
                        egui::Frame::new()
                            .fill(PANEL)
                            .stroke(egui::Stroke::new(1.0, BORDER))
                            .inner_margin(egui::Margin::same(16))
                    )
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("QUANTIC").strong().size(14.0).color(TEXT));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add(egui::Button::new("×").frame(false)).clicked() {
                                    self.quick_panel_open.set(false);
                                }
                            });
                        });
                        ui.label(egui::RichText::new("Accès rapide").size(9.5).color(MUTED));
                        ui.add_space(14.0);

                        for (icon, label, command) in [
                            ("✦", "AURA", "@aura "),
                            ("✉", "Quantic Mail", "@mail"),
                            ("●", "ZOON", "@pulse"),
                            ("⌂", "Accueil Quantic", "> accueil"),
                        ] {
                            if ui.add_sized(
                                [ui.available_width(), 42.0],
                                egui::Button::new(
                                    egui::RichText::new(format!("{icon}   {label}")).strong().size(11.0).color(TEXT)
                                )
                                .fill(PANEL_SOFT)
                                .stroke(egui::Stroke::new(1.0, BORDER))
                                .corner_radius(18.0)
                            ).clicked() {
                                *self.dock_input.borrow_mut() = command.to_string();
                                execute = true;
                                self.quick_panel_open.set(false);
                            }
                            ui.add_space(6.0);
                        }

                        ui.add_space(8.0);
                        if ui.add_sized(
                            [ui.available_width(), 36.0],
                            egui::Button::new(egui::RichText::new("★  Ajouter aux favoris").size(10.0).color(ACCENT))
                                .fill(egui::Color32::from_rgb(35, 30, 23))
                                .stroke(egui::Stroke::new(1.0, ACCENT_SOFT))
                                .corner_radius(16.0)
                        ).clicked() {
                            let url = self.current_url.borrow().clone();
                            let mut data = self.browser_data.borrow_mut();
                            if !data.favorites.contains(&url) {
                                data.favorites.insert(0, url);
                                data.favorites.truncate(24);
                                save_browser_data(&data);
                                *self.status.borrow_mut() = "Ajouté aux favoris".into();
                            }
                        }

                        let data = self.browser_data.borrow();
                        if !data.favorites.is_empty() {
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new("FAVORIS").strong().size(9.0).color(ACCENT));
                            for favorite in data.favorites.iter().take(4) {
                                let label = Url::parse(favorite).ok()
                                    .and_then(|u| u.host_str().map(str::to_string))
                                    .unwrap_or_else(|| favorite.clone());
                                if ui.add(egui::Button::new(egui::RichText::new(label).size(9.5).color(TEXT)).frame(false)).clicked() {
                                    *self.dock_input.borrow_mut() = favorite.clone();
                                    execute = true;
                                }
                            }
                        }
                        drop(data);

                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new("PROTECTION").strong().size(9.0).color(ACCENT));
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(format!("{} ressources bloquées", self.blocked_resources.get())).size(10.0).color(TEXT));
                        ui.label(egui::RichText::new(format!("{} permissions refusées", self.permissions_denied.get())).size(10.0).color(TEXT));
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("Les données techniques restent discrètes et accessibles ici.").size(9.0).color(MUTED));
                    });
            }

            let dock_height = if self.dock_expanded.get() { 126.0 } else { 86.0 };
            egui::TopBottomPanel::bottom("glide_dock")
                .exact_height(dock_height)
                .frame(egui::Frame::new().fill(BG))
                .show(ctx, |ui| {
                    ui.add_space(if self.dock_expanded.get() { 8.0 } else { 12.0 });
                    ui.vertical_centered(|ui| {
                        if self.dock_expanded.get() {
                            ui.horizontal_centered(|ui| {
                                for (label, command) in [
                                    ("AURA", "@aura "),
                                    ("Mail", "@mail"),
                                    ("ZOON", "@pulse"),
                                    ("Accueil", "> accueil"),
                                ] {
                                    if ui.add(
                                        egui::Button::new(egui::RichText::new(label).strong().size(10.0).color(TEXT))
                                            .fill(PANEL_SOFT)
                                            .stroke(egui::Stroke::new(1.0, BORDER))
                                            .corner_radius(14.0)
                                            .min_size(egui::vec2(78.0, 30.0))
                                    ).on_hover_text(match label {
                                        "AURA" => "Ouvrir AURA",
                                        "Mail" => "Ouvrir Quantic Mail",
                                        "ZOON" => "Ouvrir ZOON",
                                        _ => "Retour à l’accueil Quantic",
                                    }).clicked() {
                                        *self.dock_input.borrow_mut() = command.to_string();
                                        execute = true;
                                    }
                                }
                            });
                            ui.add_space(7.0);
                        }

                        let dock_width = ui.available_width().min(980.0);
                        egui::Frame::new()
                            .fill(PANEL)
                            .stroke(egui::Stroke::new(1.0, BORDER))
                            .corner_radius(28.0)
                            .inner_margin(egui::Margin::symmetric(12, 9))
                            .show(ui, |ui| {
                                ui.set_width(dock_width);
                                ui.horizontal(|ui| {
                                    let nav = |ui: &mut egui::Ui, enabled: bool, text: &str| {
                                        ui.add_enabled(
                                            enabled,
                                            egui::Button::new(egui::RichText::new(text).size(17.0).color(TEXT))
                                                .frame(false)
                                                .corner_radius(18.0)
                                                .min_size(egui::vec2(38.0, 38.0))
                                        )
                                    };

                                    if nav(ui, self.webview.can_go_back(), "‹")
                                        .on_hover_text("Retour · Alt+←")
                                        .clicked()
                                    {
                                        self.webview.go_back(1);
                                        *self.status.borrow_mut() = "Retour".into();
                                    }
                                    if nav(ui, self.webview.can_go_forward(), "›")
                                        .on_hover_text("Suivant · Alt+→")
                                        .clicked()
                                    {
                                        self.webview.go_forward(1);
                                        *self.status.borrow_mut() = "Suivant".into();
                                    }
                                    if nav(ui, true, "↻")
                                        .on_hover_text("Recharger · Ctrl+R")
                                        .clicked()
                                    {
                                        self.webview.reload();
                                        *self.status.borrow_mut() = "Actualisation…".into();
                                    }

                                    let edit_width = (dock_width - 250.0).max(260.0);
                                    egui::Frame::new()
                                        .fill(egui::Color32::from_rgb(20, 19, 18))
                                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(63, 57, 49)))
                                        .corner_radius(21.0)
                                        .inner_margin(egui::Margin::symmetric(14, 2))
                                        .show(ui, |ui| {
                                            let mut input = self.dock_input.borrow_mut();
                                            let response = ui.add_sized(
                                                [edit_width, 38.0],
                                                egui::TextEdit::singleline(&mut *input)
                                                    .hint_text("Rechercher ou saisir une adresse…")
                                                    .frame(egui::Frame::NONE),
                                            );
                                            if self.dock_focus_requested.replace(false) {
                                                response.request_focus();
                                            }
                                            if response.gained_focus() {
                                                self.dock_expanded.set(true);
                                            }
                                            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                                execute = true;
                                            }
                                        });

                                    if ui.add(
                                        egui::Button::new(egui::RichText::new("→").strong().size(17.0).color(BG))
                                            .fill(ACCENT)
                                            .corner_radius(20.0)
                                            .min_size(egui::vec2(42.0, 40.0))
                                    ).on_hover_text("Ouvrir · Entrée").clicked() {
                                        execute = true;
                                    }

                                    if ui.add(
                                        egui::Button::new(egui::RichText::new("✦").size(16.0).color(ACCENT))
                                            .fill(PANEL_SOFT)
                                            .corner_radius(20.0)
                                            .min_size(egui::vec2(40.0, 40.0))
                                    ).on_hover_text("AURA · assistant Quantic").clicked() {
                                        *self.dock_input.borrow_mut() = "@aura ".to_string();
                                        self.dock_expanded.set(true);
                                        self.dock_focus_requested.set(true);
                                    }
                                });
                            });

                        ui.add_space(5.0);
                        ui.horizontal_centered(|ui| {
                            ui.label(egui::RichText::new(self.status.borrow().as_str()).size(8.5).color(MUTED));
                            ui.label(egui::RichText::new("•").size(8.0).color(BORDER));
                            ui.label(egui::RichText::new("Protection active").size(8.5).color(MUTED));
                        });
                    });
                });

            let available = ctx.available_rect();
            if self.home_open.get() {
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(BG))
                    .show(ctx, |ui| {
                        ui.add_space((ui.available_height() * 0.18).min(130.0));
                        ui.vertical_centered(|ui| {
                            let (logo, _) = ui.allocate_exact_size(egui::vec2(86.0, 86.0), egui::Sense::hover());
                            ui.painter().circle_filled(logo.center(), 40.0, ACCENT);
                            ui.painter().circle_filled(logo.center(), 29.0, egui::Color32::from_rgb(31, 25, 18));
                            ui.painter().circle_filled(logo.center() + egui::vec2(10.0, -10.0), 7.0, egui::Color32::from_rgb(255, 224, 154));
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new("GLIDE").strong().size(28.0).color(TEXT));
                            ui.label(egui::RichText::new("Le Web, sans Chromium.").size(11.0).color(MUTED));
                            ui.add_space(24.0);

                            if ui.add_sized(
                                [460.0_f32.min(ui.available_width() - 30.0), 48.0],
                                egui::Button::new(
                                    egui::RichText::new("⌕   Rechercher ou saisir une adresse").size(12.0).color(MUTED)
                                )
                                .fill(PANEL)
                                .stroke(egui::Stroke::new(1.0, BORDER))
                                .corner_radius(24.0)
                            ).clicked() {
                                self.dock_expanded.set(true);
                                self.dock_focus_requested.set(true);
                            }

                            ui.add_space(20.0);
                            ui.horizontal_centered(|ui| {
                                for (label, command) in [
                                    ("✦  AURA", "@aura "),
                                    ("✉  Mail", "@mail"),
                                    ("●  ZOON", "@pulse"),
                                    ("◈  Quantic", "@quantic"),
                                ] {
                                    if ui.add(
                                        egui::Button::new(egui::RichText::new(label).size(10.0).color(TEXT))
                                            .fill(PANEL_SOFT)
                                            .stroke(egui::Stroke::new(1.0, BORDER))
                                            .corner_radius(18.0)
                                            .min_size(egui::vec2(92.0, 38.0))
                                    ).clicked() {
                                        *self.dock_input.borrow_mut() = command.into();
                                        execute = true;
                                    }
                                }
                            });

                            let data = self.browser_data.borrow();
                            if !data.favorites.is_empty() {
                                ui.add_space(24.0);
                                ui.label(egui::RichText::new("FAVORIS").strong().size(9.0).color(ACCENT));
                                ui.add_space(5.0);
                                ui.horizontal_centered(|ui| {
                                    for favorite in data.favorites.iter().take(5) {
                                        let label = Url::parse(favorite).ok()
                                            .and_then(|u| u.host_str().map(str::to_string))
                                            .unwrap_or_else(|| favorite.clone());
                                        if ui.add(
                                            egui::Button::new(egui::RichText::new(label).size(9.0).color(TEXT))
                                                .fill(PANEL)
                                                .corner_radius(15.0)
                                        ).clicked() {
                                            *self.dock_input.borrow_mut() = favorite.clone();
                                            execute = true;
                                        }
                                    }
                                });
                            }
                        });
                    });
            }

            self.content_height_points.set(available.height());
            let pixels_per_point = ctx.pixels_per_point();
            let width = (available.width() * pixels_per_point).max(1.0) as u32;
            let height = (available.height() * pixels_per_point).max(1.0) as u32;
            let size = winit::dpi::PhysicalSize::new(width, height);
            if self.web_context.size() != size {
                self.web_context.resize(size);
                self.webview.resize(size);
            }

            if !self.home_open.get() {
                self.webview.paint();
            }

            if !self.home_open.get() {
            if let Some(render_to_parent) = self.web_context.render_to_parent_callback() {
                ctx.layer_painter(LayerId::background()).add(PaintCallback {
                    rect: available,
                    callback: Arc::new(CallbackFn::new(move |info, painter| {
                        let clip = info.viewport_in_pixels();
                        let rect = Rect::new(
                            Point2D::new(clip.left_px, clip.from_bottom_px),
                            Size2D::new(clip.width_px, clip.height_px),
                        );
                        render_to_parent(painter.gl().as_ref(), rect);
                    })),
                });
            }
            }

            if ctx.input(|i| (i.modifiers.ctrl || i.modifiers.command) && i.key_pressed(egui::Key::L)) {
                *self.dock_input.borrow_mut() = self.current_url.borrow().clone();
                self.dock_expanded.set(true);
                self.dock_focus_requested.set(true);
                *self.status.borrow_mut() = "Adresse sélectionnée".into();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.dock_expanded.set(false);
            }
        });

        if execute {
            self.execute_dock();
        }

        self.window_context.prepare_for_rendering();
        egui.paint(&self.window);
        self.window_context.present();
    }

    fn point_in_webview(&self, x: f32, y: f32) -> bool {
        y >= 54.0 && y < 54.0 + self.content_height_points.get() && x >= 0.0
    }

    fn webview_point(&self, x: f32, y: f32) -> servo::DevicePoint {
        let scale = self.window.scale_factor() as f32;
        servo::DevicePoint::new(x * scale, (y - 54.0) * scale)
    }

    fn handle_mouse_move(&self, position: winit::dpi::PhysicalPosition<f64>) {
        let scale = self.window.scale_factor() as f32;
        let x = position.x as f32 / scale;
        let y = position.y as f32 / scale;
        self.cursor_point.set((x, y));

        if self.point_in_webview(x, y) {
            self.webview.notify_input_event(InputEvent::MouseMove(MouseMoveEvent::new(
                self.webview_point(x, y).into(),
            )));
        } else {
            self.webview.notify_input_event(InputEvent::MouseLeftViewport(
                MouseLeftViewportEvent::default(),
            ));
        }
    }

    fn handle_mouse_button(&self, button: MouseButton, state: ElementState) {
        let (x, y) = self.cursor_point.get();
        if !self.point_in_webview(x, y) {
            return;
        }

        let button = match button {
            MouseButton::Left => ServoMouseButton::Primary,
            MouseButton::Right => ServoMouseButton::Secondary,
            MouseButton::Middle => ServoMouseButton::Auxiliary,
            MouseButton::Back => ServoMouseButton::Back,
            MouseButton::Forward => ServoMouseButton::Forward,
            MouseButton::Other(value) => ServoMouseButton::Other(value),
        };
        let action = match state {
            ElementState::Pressed => MouseButtonAction::Down,
            ElementState::Released => MouseButtonAction::Up,
        };

        self.webview.notify_input_event(InputEvent::MouseButton(MouseButtonEvent::new(
            action,
            button,
            self.webview_point(x, y).into(),
        )));
    }

    fn handle_wheel(&self, delta: MouseScrollDelta) {
        let (x, y) = self.cursor_point.get();
        if !self.point_in_webview(x, y) {
            return;
        }

        let (dx, dy) = match delta {
            MouseScrollDelta::LineDelta(x, y) => ((x * 38.0) as f64, (y * 38.0) as f64),
            MouseScrollDelta::PixelDelta(pos) => (pos.x, pos.y),
        };
        self.webview.notify_input_event(InputEvent::Wheel(WheelEvent::new(
            WheelDelta { x: dx, y: dy, z: 0.0, mode: WheelMode::DeltaPixel },
            self.webview_point(x, y).into(),
        )));
    }

    fn servo_modifiers(&self) -> ServoModifiers {
        let modifiers = self.modifiers_state.get();
        let mut result = ServoModifiers::empty();
        if modifiers.shift_key() {
            result |= ServoModifiers::SHIFT;
        }
        if modifiers.control_key() {
            result |= ServoModifiers::CONTROL;
        }
        if modifiers.alt_key() {
            result |= ServoModifiers::ALT;
        }
        if modifiers.super_key() {
            result |= ServoModifiers::META;
        }
        result
    }

    fn send_keyboard_event(
        &self,
        state: ElementState,
        key: ServoKey,
        code: Code,
        repeat: bool,
    ) {
        let state = match state {
            ElementState::Pressed => KeyState::Down,
            ElementState::Released => KeyState::Up,
        };
        self.webview.notify_input_event(InputEvent::Keyboard(
            KeyboardEvent::new_without_event(
                state,
                key,
                code,
                Location::Standard,
                self.servo_modifiers(),
                repeat,
                false,
            ),
        ));
    }

    fn handle_keyboard_input(&self, event: winit::event::KeyEvent) {
        let key = match event.logical_key {
            WinitKey::Character(value) => ServoKey::Character(value.to_string()),
            WinitKey::Named(named) => ServoKey::Named(match named {
                WinitNamedKey::Enter => ServoNamedKey::Enter,
                WinitNamedKey::Tab => ServoNamedKey::Tab,
                WinitNamedKey::Backspace => ServoNamedKey::Backspace,
                WinitNamedKey::Delete => ServoNamedKey::Delete,
                WinitNamedKey::Escape => ServoNamedKey::Escape,
                WinitNamedKey::ArrowLeft => ServoNamedKey::ArrowLeft,
                WinitNamedKey::ArrowRight => ServoNamedKey::ArrowRight,
                WinitNamedKey::ArrowUp => ServoNamedKey::ArrowUp,
                WinitNamedKey::ArrowDown => ServoNamedKey::ArrowDown,
                WinitNamedKey::Home => ServoNamedKey::Home,
                WinitNamedKey::End => ServoNamedKey::End,
                WinitNamedKey::PageUp => ServoNamedKey::PageUp,
                WinitNamedKey::PageDown => ServoNamedKey::PageDown,
                WinitNamedKey::Space => return self.send_keyboard_event(
                    event.state,
                    ServoKey::Character(" ".into()),
                    Code::Space,
                    event.repeat,
                ),
                _ => ServoNamedKey::Unidentified,
            }),
            _ => ServoKey::Named(ServoNamedKey::Unidentified),
        };

        self.send_keyboard_event(
            event.state,
            key,
            Code::Unidentified,
            event.repeat,
        );
    }

    fn handle_ime(&self, ime: Ime) {
        let event = match ime {
            Ime::Enabled => ImeEvent::Composition(CompositionEvent {
                state: CompositionState::Start,
                data: String::new(),
            }),
            Ime::Preedit(text, _) => ImeEvent::Composition(CompositionEvent {
                state: CompositionState::Update,
                data: text,
            }),
            Ime::Commit(text) => ImeEvent::Composition(CompositionEvent {
                state: CompositionState::End,
                data: text,
            }),
            Ime::Disabled => ImeEvent::Dismissed,
        };
        self.webview.notify_input_event(InputEvent::Ime(event));
    }
}

enum App {
    Initial(Waker),
    Running(Rc<FusionState>),
}

impl ApplicationHandler<WakeEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Self::Initial(waker) = self else { return; };

        let window = event_loop.create_window(
            Window::default_attributes()
                .with_title("Quantic Glide Fusion")
                .with_inner_size(winit::dpi::PhysicalSize::new(1440_u32, 900_u32))
                .with_min_inner_size(winit::dpi::PhysicalSize::new(980_u32, 680_u32)),
        ).expect("create Glide window");

        let display_handle = event_loop.display_handle().expect("display handle");
        let window_handle = window.window_handle().expect("window handle");
        let window_context = Rc::new(
            WindowRenderingContext::new(display_handle, window_handle, window.inner_size())
                .expect("window rendering context")
        );
        window_context.make_current().expect("make GL current");

        let initial_web_size = winit::dpi::PhysicalSize::new(
            window.inner_size().width.max(1),
            window.inner_size().height.saturating_sub(140).max(1),
        );
        let web_context = Rc::new(window_context.offscreen_context(initial_web_size));

        let egui = EguiGlow::new(
            event_loop,
            window_context.glow_gl_api(),
            None,
            None,
            false,
        );

        let servo = ServoBuilder::default()
            .event_loop_waker(Box::new(waker.clone()))
            .build();
        servo.setup_logging();

        let delegate = Rc::new(FusionDelegate::new());
        let webview = WebViewBuilder::new(&servo, web_context.clone())
            .url(Url::parse(START_URL).unwrap())
            .hidpi_scale_factor(Scale::new(window.scale_factor() as f32))
            .delegate(delegate.clone())
            .build();

        let state = Rc::new(FusionState {
            window,
            servo,
            webview,
            window_context,
            web_context,
            egui: RefCell::new(egui),
            dock_input: RefCell::new(START_URL.to_string()),
            current_url: RefCell::new(START_URL.to_string()),
            status: RefCell::new("Glide Fusion prêt".into()),
            dock_expanded: Cell::new(false),
            dock_focus_requested: Cell::new(false),
            content_height_points: Cell::new(700.0),
            cursor_point: Cell::new((0.0, 0.0)),
            modifiers_state: Cell::new(ModifiersState::empty()),
            blocked_resources: Cell::new(0),
            permissions_denied: Cell::new(0),
            quick_panel_open: Cell::new(false),
            browser_data: RefCell::new(load_browser_data()),
            home_open: Cell::new(true),
        });
        delegate.bind(&state);

        state.window.request_redraw();
        *self = Self::Running(state);
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: WakeEvent) {
        if let Self::Running(state) = self {
            state.servo.spin_event_loop();
            state.window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Self::Running(state) = self else { return; };

        if matches!(&event, WindowEvent::RedrawRequested) {
            state.servo.spin_event_loop();
            state.draw();
            return;
        }

        let egui_response = state.egui.borrow_mut().on_window_event(&state.window, &event);
        if egui_response.repaint {
            state.window.request_redraw();
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                state.window_context.resize(size);
                state.window.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.handle_mouse_move(position);
            }
            WindowEvent::MouseInput { state: button_state, button, .. } if !egui_response.consumed => {
                state.handle_mouse_button(button, button_state);
            }
            WindowEvent::MouseWheel { delta, .. } if !egui_response.consumed => {
                state.handle_wheel(delta);
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                state.modifiers_state.set(modifiers.state());
            }
            WindowEvent::KeyboardInput { event, .. } if !egui_response.consumed => {
                let modifiers = state.modifiers_state.get();
                let command = modifiers.control_key() || modifiers.super_key();
                let alt = modifiers.alt_key();
                let is_location_shortcut = command
                    && matches!(&event.logical_key, WinitKey::Character(value) if value.eq_ignore_ascii_case("l"));
                let is_reload_shortcut = command
                    && matches!(&event.logical_key, WinitKey::Character(value) if value.eq_ignore_ascii_case("r"));
                let is_back_shortcut = alt && matches!(&event.logical_key, WinitKey::Named(WinitNamedKey::ArrowLeft));
                let is_forward_shortcut = alt && matches!(&event.logical_key, WinitKey::Named(WinitNamedKey::ArrowRight));

                if event.state == ElementState::Pressed {
                    if is_reload_shortcut {
                        state.webview.reload();
                        state.window.request_redraw();
                    } else if is_back_shortcut {
                        state.webview.go_back(1);
                        state.window.request_redraw();
                    } else if is_forward_shortcut {
                        state.webview.go_forward(1);
                        state.window.request_redraw();
                    } else if !is_location_shortcut {
                        state.handle_keyboard_input(event);
                    }
                } else if !is_location_shortcut && !is_reload_shortcut && !is_back_shortcut && !is_forward_shortcut {
                    state.handle_keyboard_input(event);
                }
            }
            WindowEvent::Ime(ime) if !egui_response.consumed => {
                state.handle_ime(ime);
            }
            _ => {}
        }

        state.servo.spin_event_loop();
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // Hard runtime integration check: Ladybird's LibWeb tokenizer is linked into
    // this executable independently from Servo's SpiderMonkey/ICU dependency graph.
    use quantic_ladybird_html::{HtmlTokenizer, TokenType};
    let mut tokenizer = HtmlTokenizer::new(
        "<!doctype html><html><body><main data-engine='ladybird'>Fusion</main></body></html>"
            .encode_utf16()
            .collect(),
    );
    let mut start_tags = 0usize;
    while let Some(token) = tokenizer.next_token(false, false) {
        if token.token_type == TokenType::StartTag {
            start_tags += 1;
        }
        if token.token_type == TokenType::EndOfFile {
            break;
        }
    }
    assert!(start_tags >= 3, "Ladybird HTML Fusion probe failed");

    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("install crypto provider");

    println!("Quantic Glide Fusion");
    for component in [
        "Quantic: shell, privacy policy, orchestration and product UX",
        "Servo 0.7: primary web runtime",
        "Mozilla: SpiderMonkey + Stylo + WebRender through Servo",
        "Ladybird LibWeb: independent HTML tokenizer/conformance path",
        "Mozilla Neqo: isolated QUIC/HTTP3 integration track",
    ] {
        println!("{component}");
    }

    let event_loop = EventLoop::<WakeEvent>::with_user_event().build()?;
    let mut app = App::Initial(Waker(event_loop.create_proxy()));
    event_loop.run_app(&mut app)?;
    Ok(())
}
