use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

use euclid::Scale;
use servo::{
    InputEvent, RenderingContext, Servo, ServoBuilder, WebView, WebViewBuilder, WheelDelta,
    WheelEvent, WheelMode, WindowRenderingContext,
};
use servo_embedder_traits::EventLoopWaker;
use tracing::warn;
use url::Url;
use webrender_api::units::DevicePoint;
use winit::application::ApplicationHandler;
use winit::event::{MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::Window;

fn main() -> Result<(), Box<dyn Error>> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("failed to install crypto provider");

    println!("Quantic Fusion");
    for component in quantic_engine::fusion::active_runtime_components() {
        println!(
            "  {:?}: {} — {}",
            component.family, component.name, component.role
        );
    }
    println!(
        "  forbidden: {}",
        quantic_engine::fusion::forbidden_runtime_families().join(", ")
    );

    let event_loop = EventLoop::<WakeEvent>::with_user_event().build()?;
    let mut app = FusionApp::Initial(Waker(event_loop.create_proxy()));
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct AppState {
    window: Window,
    servo: Servo,
    rendering_context: Rc<WindowRenderingContext>,
    webviews: RefCell<Vec<WebView>>,
}

impl servo::WebViewDelegate for AppState {
    fn notify_new_frame_ready(&self, _: WebView) {
        self.window.request_redraw();
    }

    fn notify_url_changed(&self, _: WebView, url: Url) {
        self.window
            .set_title(&format!("Quantic Fusion — {}", url.as_str()));
    }

    fn notify_page_title_changed(&self, _: WebView, title: Option<String>) {
        if let Some(title) = title.filter(|title| !title.trim().is_empty()) {
            self.window.set_title(&format!("{} — Quantic Fusion", title));
        }
    }
}

enum FusionApp {
    Initial(Waker),
    Running(Rc<AppState>),
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
        if let Err(error) = self.0.send_event(WakeEvent) {
            warn!(?error, "failed to wake Quantic Fusion event loop");
        }
    }
}

impl ApplicationHandler<WakeEvent> for FusionApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Self::Initial(waker) = self else {
            return;
        };

        let display_handle = event_loop.display_handle().expect("display handle");
        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Quantic Fusion — Servo + Mozilla + Ladybird")
                    .with_inner_size(winit::dpi::PhysicalSize::new(1440_u32, 900_u32)),
            )
            .expect("window");
        let window_handle = window.window_handle().expect("window handle");

        let rendering_context = Rc::new(
            WindowRenderingContext::new(display_handle, window_handle, window.inner_size())
                .expect("rendering context"),
        );
        let _ = rendering_context.make_current();

        let servo = ServoBuilder::default()
            .event_loop_waker(Box::new(waker.clone()))
            .build();
        servo.setup_logging();

        let state = Rc::new(AppState {
            window,
            servo,
            rendering_context,
            webviews: RefCell::new(Vec::new()),
        });

        let url = std::env::args()
            .nth(1)
            .and_then(|value| Url::parse(&value).ok())
            .unwrap_or_else(|| Url::parse("https://servo.org/").unwrap());

        let webview = WebViewBuilder::new(&state.servo, state.rendering_context.clone())
            .url(url)
            .hidpi_scale_factor(Scale::new(state.window.scale_factor() as f32))
            .delegate(state.clone())
            .build();

        state.webviews.borrow_mut().push(webview);
        *self = Self::Running(state);
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: WakeEvent) {
        if let Self::Running(state) = self {
            state.servo.spin_event_loop();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if let Self::Running(state) = self {
            state.servo.spin_event_loop();
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                if let Self::Running(state) = self {
                    if let Some(webview) = state.webviews.borrow().last() {
                        webview.paint();
                        state.rendering_context.present();
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let Self::Running(state) = self {
                    if let Some(webview) = state.webviews.borrow().last() {
                        let (x, y, mode) = match delta {
                            MouseScrollDelta::LineDelta(dx, dy) => {
                                ((dx * 76.0) as f64, (dy * 76.0) as f64, WheelMode::DeltaLine)
                            }
                            MouseScrollDelta::PixelDelta(delta) => {
                                (delta.x, delta.y, WheelMode::DeltaPixel)
                            }
                        };
                        webview.notify_input_event(InputEvent::Wheel(WheelEvent::new(
                            WheelDelta { x, y, z: 0.0, mode },
                            DevicePoint::default().into(),
                        )));
                    }
                }
            }
            WindowEvent::Resized(size) => {
                if let Self::Running(state) = self {
                    if let Some(webview) = state.webviews.borrow().last() {
                        webview.resize(size);
                    }
                }
            }
            _ => {}
        }
    }
}
