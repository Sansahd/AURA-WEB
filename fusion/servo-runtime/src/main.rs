use std::cell::{Cell, RefCell};
use std::error::Error;
use std::rc::Rc;
use std::sync::Arc;

use eframe::egui;
use egui::{Id, LayerId, Order, PaintCallback};
use egui_glow::{CallbackFn, EguiGlow};
use euclid::{Point2D, Rect, Scale, Size2D};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use servo::{
    InputEvent, MouseButton as ServoMouseButton, MouseButtonAction, MouseButtonEvent,
    MouseLeftViewportEvent, MouseMoveEvent, OffscreenRenderingContext, RenderingContext, Servo,
    ServoBuilder, WebView, WebViewBuilder, WheelDelta, WheelEvent, WheelMode,
    WindowRenderingContext,
};
use servo_embedder_traits::EventLoopWaker;
use url::Url;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::window::Window;

const START_URL: &str = "https://servo.org/";
const SEARCH_PREFIX: &str = "https://duckduckgo.com/?q=";
const MAIL_URL: &str = "https://mediumorchid-badger-314305.hostingersite.com/mail/";
const PULSE_URL: &str = "https://mediumorchid-badger-314305.hostingersite.com/pulse/";

const BG: egui::Color32 = egui::Color32::from_rgb(13, 12, 11);
const PANEL: egui::Color32 = egui::Color32::from_rgb(28, 26, 23);
const TEXT: egui::Color32 = egui::Color32::from_rgb(242, 238, 228);
const MUTED: egui::Color32 = egui::Color32::from_rgb(151, 146, 137);
const ACCENT: egui::Color32 = egui::Color32::from_rgb(242, 177, 52);
const BORDER: egui::Color32 = egui::Color32::from_rgb(58, 53, 47);

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
}

impl servo::WebViewDelegate for FusionState {
    fn notify_new_frame_ready(&self, _webview: WebView) {
        self.window.request_redraw();
    }

    fn notify_url_changed(&self, _webview: WebView, url: Url) {
        *self.current_url.borrow_mut() = url.to_string();
        if !self.egui.borrow().egui_ctx.memory(|memory| memory.focused().is_some()) {
            *self.dock_input.borrow_mut() = url.to_string();
        }
        self.window.request_redraw();
    }

    fn notify_page_title_changed(&self, _webview: WebView, title: Option<String>) {
        let title = title.filter(|value| !value.trim().is_empty()).unwrap_or_else(|| "Glide".into());
        self.window.set_title(&format!("{title} — Quantic Glide Fusion"));
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
        } else if lower == "@aura" || lower.starts_with("@aura ") {
            let prompt = raw.strip_prefix("@aura").unwrap_or("").trim();
            *self.status.borrow_mut() = if prompt.is_empty() {
                "AURA prête".into()
            } else {
                format!("AURA · {prompt}")
            };
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
            visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(38, 34, 29);
            visuals.widgets.active.bg_fill = egui::Color32::from_rgb(73, 54, 22);
            ctx.set_visuals(visuals);

            egui::TopBottomPanel::top("fusion_header")
                .exact_height(34.0)
                .frame(egui::Frame::new().fill(BG))
                .show(ctx, |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.add_space(10.0);
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), 8.0, ACCENT);
                        ui.painter().circle_filled(rect.center(), 3.0, BG);
                        ui.label(egui::RichText::new("GLIDE").strong().size(13.0));
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("FUSION").strong().size(9.0).color(ACCENT));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new("SERVO · MOZILLA · LADYBIRD · QUANTIC").size(8.5).color(MUTED));
                        });
                    });
                });

            let dock_height = if self.dock_expanded.get() { 108.0 } else { 66.0 };
            egui::TopBottomPanel::bottom("glide_dock")
                .exact_height(dock_height)
                .frame(egui::Frame::new().fill(BG))
                .show(ctx, |ui| {
                    ui.add_space(if self.dock_expanded.get() { 6.0 } else { 9.0 });
                    ui.vertical_centered(|ui| {
                        if self.dock_expanded.get() {
                            ui.label(
                                egui::RichText::new("@aura   @mail   @pulse   > retour   > recharger")
                                    .size(9.5)
                                    .color(MUTED),
                            );
                            ui.add_space(4.0);
                        }

                        let dock_width = ui.available_width().min(920.0);
                        egui::Frame::new()
                            .fill(egui::Color32::from_rgb(28, 26, 23))
                            .stroke(egui::Stroke::new(1.0, BORDER))
                            .corner_radius(22.0)
                            .inner_margin(egui::Margin::symmetric(10, 7))
                            .show(ui, |ui| {
                                ui.set_width(dock_width);
                                ui.horizontal(|ui| {
                                    if ui.add_enabled(self.webview.can_go_back(), egui::Button::new("←").frame(false)).clicked() {
                                        self.webview.go_back(1);
                                    }
                                    if ui.add_enabled(self.webview.can_go_forward(), egui::Button::new("→").frame(false)).clicked() {
                                        self.webview.go_forward(1);
                                    }

                                    let edit_width = (dock_width - 175.0).max(260.0);
                                    let mut input = self.dock_input.borrow_mut();
                                    let response = ui.add_sized(
                                        [edit_width, 36.0],
                                        egui::TextEdit::singleline(&mut *input)
                                            .hint_text("Rechercher, saisir une adresse ou une commande…")
                                            .frame(false),
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

                                    if ui.add(
                                        egui::Button::new(egui::RichText::new("↵").strong().color(BG))
                                            .fill(ACCENT)
                                            .min_size(egui::vec2(38.0, 34.0))
                                    ).clicked() {
                                        execute = true;
                                    }

                                    if ui.add(
                                        egui::Button::new("A")
                                            .fill(egui::Color32::from_rgb(73, 54, 22))
                                            .min_size(egui::vec2(34.0, 34.0))
                                    ).on_hover_text("AURA").clicked() {
                                        *input = "@aura ".to_string();
                                        self.dock_expanded.set(true);
                                        self.dock_focus_requested.set(true);
                                    }
                                });
                            });

                        ui.add_space(3.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(self.status.borrow().as_str()).size(9.0).color(MUTED));
                            ui.separator();
                            ui.label(egui::RichText::new("Chromium 0").size(9.0).strong().color(egui::Color32::from_rgb(129, 191, 142)));
                        });
                    });
                });

            let available = ctx.available_rect_before_wrap();
            self.content_height_points.set(available.height());
            let pixels_per_point = ctx.pixels_per_point();
            let width = (available.width() * pixels_per_point).max(1.0) as u32;
            let height = (available.height() * pixels_per_point).max(1.0) as u32;
            let size = winit::dpi::PhysicalSize::new(width, height);
            if self.web_context.size() != size {
                self.web_context.resize(size);
                self.webview.resize(size);
            }

            self.webview.paint();

            if let Some(render_to_parent) = self.web_context.render_to_parent_callback() {
                ctx.layer_painter(LayerId::background()).add(PaintCallback {
                    rect: available,
                    callback: Arc::new(CallbackFn::new(move |info, painter| {
                        let clip = info.viewport_in_pixels();
                        let rect = Rect::new(
                            Point2D::new(clip.left_px, clip.from_bottom_px),
                            Size2D::new(clip.width_px, clip.height_px),
                        );
                        render_to_parent(painter.gl(), rect);
                    })),
                });
            }

            if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::L)) {
                self.dock_expanded.set(true);
                self.dock_focus_requested.set(true);
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
        y >= 34.0 && y < 34.0 + self.content_height_points.get() && x >= 0.0
    }

    fn webview_point(&self, x: f32, y: f32) -> servo::DevicePoint {
        let scale = self.window.scale_factor() as f32;
        servo::DevicePoint::new(x * scale, (y - 34.0) * scale)
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
            window.inner_size().height.saturating_sub(100).max(1),
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

        let placeholder = Rc::new_cyclic(|weak| {
            let delegate: Rc<dyn servo::WebViewDelegate> = weak.upgrade().expect("Fusion delegate");
            let webview = WebViewBuilder::new(&servo, web_context.clone())
                .url(Url::parse(START_URL).unwrap())
                .hidpi_scale_factor(Scale::new(window.scale_factor() as f32))
                .delegate(delegate)
                .build();

            FusionState {
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
            }
        });

        placeholder.window.request_redraw();
        *self = Self::Running(placeholder);
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

        if event == WindowEvent::RedrawRequested {
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
            _ => {}
        }

        state.servo.spin_event_loop();
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("install crypto provider");

    println!("Quantic Glide Fusion");
    for component in quantic_engine::fusion::active_runtime_components() {
        println!("{:?}: {} — {}", component.family, component.name, component.role);
    }

    let event_loop = EventLoop::<WakeEvent>::with_user_event().build()?;
    let mut app = App::Initial(Waker(event_loop.create_proxy()));
    event_loop.run_app(&mut app)?;
    Ok(())
}
