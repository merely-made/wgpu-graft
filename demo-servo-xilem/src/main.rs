// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Demo embedding Servo in a Xilem app with input forwarding.
//!
//! The Servo viewport is an [`external_surface`] widget, which Masonry paints
//! as an external layer: it records only the widget's bounds. The demo's
//! vendored `masonry_winit` copies a registered texture into that rectangle
//! each redraw, before presenting. On the GPU path that texture is Servo's
//! frame, imported by grafting onto Masonry's own wgpu device; no pixels reach
//! the CPU. If the import cannot be set up, the demo reads frames back and
//! uploads them into the same texture slot instead.
//!
//! The Xilem UI provides a URL bar and a Go button above the viewport. Mouse,
//! scroll, and keyboard events in the viewport are forwarded to Servo.
//!
//! Usage:
//!   cargo run -p demo-servo-xilem -- https://example.com
//!   cargo run -p demo-servo-xilem          # opens built-in fixture page

mod external_surface;
mod keyutils;

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use demo_support::{DemoStatus, RenderPath};
use euclid::Scale;
use external_surface::external_surface;
use masonry::theme::default_property_set;
use masonry_winit::app::{AppDriver, MasonryState, MasonryUserEvent};
use rustls::crypto::aws_lc_rs;
use servo::{
    DevicePoint, EventLoopWaker, InputEvent, MouseButton as ServoMouseButton, MouseButtonAction,
    MouseButtonEvent, MouseLeftViewportEvent, MouseMoveEvent, RenderingContext, Servo,
    ServoBuilder, WebView, WebViewBuilder, WebViewDelegate, WheelDelta, WheelEvent, WheelMode,
};
use servo_wgpu_interop_adapter::{
    CapturingRenderingContext, ServoWgpuInteropAdapter, ServoWgpuRenderingContext,
};
use tokio::sync::watch;
use url::Url;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::error::EventLoopError;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::keyboard::ModifiersState;
use xilem::core::fork;
use xilem::view::{FlexExt, flex_col, flex_row, label, task_raw, text_button, text_input};
use xilem::{EventLoop, WidgetView, WindowOptions, Xilem};

// ── App state ────────────────────────────────────────────────────────────────

struct AppState {
    /// The URL currently shown in the navigation bar input.
    nav_input: String,
    /// The status line, updated from the frame loop.
    status: String,
}

// ── View logic ────────────────────────────────────────────────────────────────

fn app_logic(
    state: &mut AppState,
    status_rx: watch::Receiver<String>,
    nav_request: Arc<Mutex<Option<String>>>,
) -> impl WidgetView<AppState> + use<> {
    let nav_for_enter = nav_request.clone();
    let nav_for_go = nav_request.clone();
    let nav_bar = flex_row((
        text_input(
            state.nav_input.clone(),
            |state: &mut AppState, new_text: String| {
                state.nav_input = new_text;
            },
        )
        .on_enter(move |state: &mut AppState, _text: String| {
            *nav_for_enter.lock().unwrap() = Some(state.nav_input.clone());
        })
        .flex(1.0),
        text_button("Go", move |state: &mut AppState| {
            *nav_for_go.lock().unwrap() = Some(state.nav_input.clone());
        }),
    ));

    // `task_raw` must take `Fn`, not `FnOnce`. Since watch::Receiver: Clone,
    // we clone it inside the closure body.
    fork(
        flex_col((
            nav_bar,
            label(state.status.clone()),
            external_surface().flex(1.0),
        )),
        task_raw(
            move |proxy, _shutdown| {
                let mut rx = status_rx.clone();
                async move {
                    while rx.changed().await.is_ok() {
                        let status = rx.borrow_and_update().clone();
                        if proxy.message(status).is_err() {
                            break;
                        }
                    }
                }
            },
            |state: &mut AppState, status: String| {
                state.status = status;
            },
        ),
    )
}

// ── Servo ─────────────────────────────────────────────────────────────────────

/// How Servo's frames reach the external layer.
enum Pipeline {
    /// Imported by grafting onto Masonry's device, texture handed over as is.
    Gpu(ServoWgpuInteropAdapter),
    /// Read back to the CPU and uploaded into a texture on Masonry's device.
    Cpu(Rc<CapturingRenderingContext>),
}

struct ServoState {
    servo: Servo,
    webview: WebView,
    pipeline: Pipeline,
    device: wgpu::Device,
    queue: wgpu::Queue,
    viewport: PhysicalSize<u32>,
    status: DemoStatus,
}

impl ServoState {
    fn init(
        device: wgpu::Device,
        queue: wgpu::Queue,
        viewport: PhysicalSize<u32>,
        scale_factor: f64,
        initial_url: Url,
    ) -> Result<Self, String> {
        let (pipeline, context, status) =
            match ServoWgpuInteropAdapter::new(device.clone(), queue.clone(), viewport) {
                Ok(interop) => {
                    let context = interop.rendering_context();
                    let status = DemoStatus::new(RenderPath::GpuImport);
                    (Pipeline::Gpu(interop), context, status)
                },
                Err(err) => {
                    eprintln!("[demo] GPU import unavailable, using CPU readback: {err:?}");
                    let render_ctx = Rc::new(
                        ServoWgpuRenderingContext::new(viewport)
                            .map_err(|e| format!("surfman error: {e:?}"))?,
                    );
                    let capture = Rc::new(CapturingRenderingContext::new(render_ctx));
                    let context: Rc<dyn RenderingContext> = capture.clone();
                    (
                        Pipeline::Cpu(capture),
                        context,
                        DemoStatus::new(RenderPath::CpuReadback),
                    )
                },
            };

        let servo = ServoBuilder::default()
            .event_loop_waker(Box::new(NoopWaker))
            .build();
        servo.setup_logging();

        let webview = WebViewBuilder::new(&servo, context)
            .url(initial_url)
            .hidpi_scale_factor(Scale::new(scale_factor as f32))
            .delegate(Rc::new(DemoDelegate))
            .build();

        Ok(Self {
            servo,
            webview,
            pipeline,
            device,
            queue,
            viewport,
            status,
        })
    }

    /// Present Servo's painted frame and return it as a texture on Masonry's
    /// device, if a new one arrived.
    fn take_frame(&mut self) -> Option<wgpu::Texture> {
        match &self.pipeline {
            Pipeline::Gpu(interop) => {
                // The importing context imports inside present(), before the
                // GL swap chain flips.
                interop.rendering_context().present();
                let imported = interop.take_imported_texture()?;
                self.status.set_frame(
                    RenderPath::GpuImport,
                    imported.size.width,
                    imported.size.height,
                );
                Some(imported.texture)
            },
            Pipeline::Cpu(capture) => {
                capture.present();
                let rgba = capture.take_frame()?;
                let (width, height) = rgba.dimensions();
                self.status
                    .set_frame(RenderPath::CpuReadback, width, height);
                let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("servo readback"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                self.queue.write_texture(
                    texture.as_image_copy(),
                    rgba.as_raw(),
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(width * 4),
                        rows_per_image: Some(height),
                    },
                    texture.size(),
                );
                Some(texture)
            },
        }
    }

    /// Follow the external layer's size. `webview.resize` drives the Servo
    /// side (see the winit demo).
    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size != self.viewport {
            self.viewport = size;
            if let Pipeline::Cpu(capture) = &self.pipeline {
                capture.resize(size);
            }
            self.webview.resize(size);
        }
    }
}

/// A no-op Servo event loop waker: in Poll mode we don't need explicit waking.
#[derive(Clone)]
struct NoopWaker;

impl EventLoopWaker for NoopWaker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(self.clone())
    }
    fn wake(&self) {}
}

struct DemoDelegate;

impl WebViewDelegate for DemoDelegate {
    fn notify_url_changed(&self, _webview: WebView, url: Url) {
        eprintln!("[servo] URL changed: {url}");
    }

    fn notify_closed(&self, _webview: WebView) {
        eprintln!("[servo] WebView closed by page (window.close)");
    }

    fn notify_crashed(&self, _webview: WebView, reason: String, backtrace: Option<String>) {
        eprintln!("[servo] CRASH: {reason}");
        if let Some(bt) = backtrace {
            eprintln!("{bt}");
        }
    }
}

// ── ApplicationHandler ────────────────────────────────────────────────────────

struct ServoXilemApp {
    masonry_state: MasonryState,
    app_driver: Box<dyn AppDriver>,
    servo_state: Option<ServoState>,
    /// Sends the status line to the Xilem task_raw view.
    status_tx: watch::Sender<String>,
    /// Navigation requests from the Xilem UI.
    nav_request: Arc<Mutex<Option<String>>>,
    /// URL to load on start.
    initial_url: Url,
    /// Servo failed to start; don't retry every frame.
    servo_failed: bool,

    // ── Input tracking ───────────────────────────────────────────────────
    /// Last known cursor position in physical window coordinates.
    cursor_position: PhysicalPosition<f64>,
    /// Current keyboard modifier state (Ctrl, Shift, Alt, Super).
    modifiers: ModifiersState,
    /// Whether the cursor is currently inside the Servo viewport.
    cursor_in_viewport: bool,
}

impl ServoXilemApp {
    /// The cursor as a Servo [`DevicePoint`] if it is inside the viewport
    /// (the external layer's rectangle).
    fn viewport_point(&self) -> Option<DevicePoint> {
        let [x, y, width, height] = self.masonry_state.external_rect()?;
        let px = self.cursor_position.x - f64::from(x);
        let py = self.cursor_position.y - f64::from(y);
        (px >= 0.0 && py >= 0.0 && px < f64::from(width) && py < f64::from(height))
            .then(|| DevicePoint::new(px as f32, py as f32))
    }

    /// Start Servo once Masonry has a device and has laid out the viewport.
    fn ensure_servo(&mut self) {
        if self.servo_state.is_some() || self.servo_failed {
            return;
        }
        let (Some((device, queue)), Some([_, _, width, height])) = (
            self.masonry_state.wgpu_device(),
            self.masonry_state.external_rect(),
        ) else {
            return;
        };
        let scale_factor = self.masonry_state.scale_factor().unwrap_or(1.0);
        match ServoState::init(
            device,
            queue,
            PhysicalSize::new(width, height),
            scale_factor,
            self.initial_url.clone(),
        ) {
            Ok(state) => self.servo_state = Some(state),
            Err(err) => {
                eprintln!("Servo init failed: {err}");
                self.servo_failed = true;
            },
        }
    }
}

impl ApplicationHandler<MasonryUserEvent> for ServoXilemApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.masonry_state
            .handle_resumed(event_loop, &mut *self.app_driver);
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        self.masonry_state.handle_suspended(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        // ── Forward relevant events to Servo ─────────────────────────────
        //
        // Masonry still receives every event for its own UI (nav bar, etc.);
        // the viewport widget doesn't consume pointer events.

        if let WindowEvent::ModifiersChanged(mods) = &event {
            self.modifiers = mods.state();
        }

        if let Some(ss) = &self.servo_state {
            match &event {
                // ── Cursor tracking ──────────────────────────────────────
                WindowEvent::CursorMoved { position, .. } => {
                    self.cursor_position = *position;
                    let was_in = self.cursor_in_viewport;
                    let point = self.viewport_point();
                    self.cursor_in_viewport = point.is_some();

                    if let Some(pt) = point {
                        ss.webview
                            .notify_input_event(InputEvent::MouseMove(MouseMoveEvent::new(
                                pt.into(),
                            )));
                    } else if was_in {
                        ss.webview.notify_input_event(InputEvent::MouseLeftViewport(
                            MouseLeftViewportEvent::default(),
                        ));
                    }
                },
                WindowEvent::CursorLeft { .. } => {
                    if self.cursor_in_viewport {
                        ss.webview.notify_input_event(InputEvent::MouseLeftViewport(
                            MouseLeftViewportEvent::default(),
                        ));
                        self.cursor_in_viewport = false;
                    }
                },

                // ── Mouse buttons ────────────────────────────────────────
                WindowEvent::MouseInput { state, button, .. } => {
                    if let Some(pt) = self.viewport_point() {
                        let action = match state {
                            ElementState::Pressed => MouseButtonAction::Down,
                            ElementState::Released => MouseButtonAction::Up,
                        };
                        let btn = match button {
                            winit::event::MouseButton::Left => ServoMouseButton::Primary,
                            winit::event::MouseButton::Right => ServoMouseButton::Secondary,
                            winit::event::MouseButton::Middle => ServoMouseButton::Auxiliary,
                            winit::event::MouseButton::Back => ServoMouseButton::Back,
                            winit::event::MouseButton::Forward => ServoMouseButton::Forward,
                            winit::event::MouseButton::Other(v) => ServoMouseButton::Other(*v),
                        };
                        ss.webview.notify_input_event(InputEvent::MouseButton(
                            MouseButtonEvent::new(action, btn, pt.into()),
                        ));
                    }
                },

                // ── Scroll wheel ─────────────────────────────────────────
                WindowEvent::MouseWheel { delta, .. } => {
                    if let Some(pt) = self.viewport_point() {
                        let (dx, dy, mode) = match delta {
                            MouseScrollDelta::LineDelta(x, y) => {
                                ((*x as f64) * 38.0, (*y as f64) * 38.0, WheelMode::DeltaLine)
                            },
                            MouseScrollDelta::PixelDelta(d) => (d.x, d.y, WheelMode::DeltaPixel),
                        };
                        ss.webview
                            .notify_input_event(InputEvent::Wheel(WheelEvent::new(
                                WheelDelta {
                                    x: dx,
                                    y: dy,
                                    z: 0.0,
                                    mode,
                                },
                                pt.into(),
                            )));
                    }
                },

                // ── Keyboard ─────────────────────────────────────────────
                //
                // Keyboard events are forwarded regardless of cursor position.
                // In a production app you would track focus (e.g. only forward
                // when the viewport is focused, not the URL bar).
                WindowEvent::KeyboardInput {
                    event: key_event, ..
                } => {
                    let servo_event =
                        keyutils::keyboard_event_from_winit(key_event, self.modifiers);
                    ss.webview
                        .notify_input_event(InputEvent::Keyboard(servo_event));
                },

                _ => {},
            }
        }

        // ── Always forward to Masonry for its own UI ─────────────────────
        self.masonry_state
            .handle_window_event(event_loop, window_id, event, &mut *self.app_driver);
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        self.masonry_state
            .handle_device_event(event_loop, device_id, event, &mut *self.app_driver);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: MasonryUserEvent) {
        self.masonry_state
            .handle_user_event(event_loop, event, &mut *self.app_driver);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Poll);
        self.ensure_servo();

        if let Some(ss) = &mut self.servo_state {
            // Handle any pending navigation from the URL bar.
            if let Some(raw_url) = self.nav_request.lock().unwrap().take() {
                // Auto-prefix https:// when no scheme is provided.
                let url =
                    Url::parse(&raw_url).or_else(|_| Url::parse(&format!("https://{raw_url}")));
                match url {
                    Ok(url) => ss.webview.load(url),
                    Err(_) => eprintln!("invalid URL: {raw_url}"),
                }
            }

            if let Some([_, _, width, height]) = self.masonry_state.external_rect() {
                ss.resize(PhysicalSize::new(width, height));
            }

            ss.servo.spin_event_loop();
            ss.webview.paint();
            if let Some(texture) = ss.take_frame() {
                self.masonry_state.set_external_texture(Some(texture));
                self.masonry_state.request_redraw();
                let summary = ss.status.summary();
                self.status_tx.send_if_modified(|status| {
                    let changed = *status != summary;
                    if changed {
                        status.clone_from(&summary);
                    }
                    changed
                });
            }
        }

        self.masonry_state.handle_about_to_wait(event_loop);
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: winit::event::StartCause) {
        self.masonry_state.handle_new_events(event_loop, cause);
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        self.masonry_state.handle_exiting(event_loop);
    }

    fn memory_warning(&mut self, event_loop: &ActiveEventLoop) {
        self.masonry_state.handle_memory_warning(event_loop);
    }
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() -> Result<(), EventLoopError> {
    aws_lc_rs::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    let initial_url = demo_support::resolve_initial_url(env!("CARGO_MANIFEST_DIR"))
        .expect("failed to resolve initial URL");

    // Masonry picks its wgpu backends from WGPU_BACKEND. On Windows the import
    // opens ANGLE's shared texture on a DX12 device, so force DX12 there.
    // SAFETY: set before any wgpu or thread initialisation.
    #[cfg(target_os = "windows")]
    unsafe {
        std::env::set_var("WGPU_BACKEND", "dx12");
    }

    // Watch channel: about_to_wait sends the status line; task_raw forwards it.
    let (status_tx, status_rx) = watch::channel(String::from("Servo loading…"));
    // Shared slot: UI writes a URL here on "Go" click; about_to_wait reads it.
    let nav_request: Arc<Mutex<Option<String>>> = Default::default();

    let app_state = AppState {
        nav_input: initial_url.to_string(),
        status: String::from("Servo loading…"),
    };

    let window_options = WindowOptions::new("servo-xilem demo")
        .with_initial_inner_size(LogicalSize::new(
            demo_support::WINDOW_WIDTH,
            demo_support::WINDOW_HEIGHT,
        ))
        .with_min_inner_size(LogicalSize::new(800.0, 600.0));

    let nav_request_for_view = nav_request.clone();
    let xilem = Xilem::new_simple(
        app_state,
        move |state: &mut AppState| {
            app_logic(state, status_rx.clone(), nav_request_for_view.clone())
        },
        window_options,
    );

    let event_loop = EventLoop::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let (driver, windows) =
        xilem.into_driver_and_windows(move |event| proxy.send_event(event).map_err(|e| e.0));

    let masonry_state =
        MasonryState::new(event_loop.create_proxy(), windows, default_property_set());

    let mut app = ServoXilemApp {
        masonry_state,
        app_driver: Box::new(driver),
        servo_state: None,
        status_tx,
        nav_request,
        initial_url,
        servo_failed: false,
        cursor_position: PhysicalPosition::new(0.0, 0.0),
        modifiers: ModifiersState::default(),
        cursor_in_viewport: false,
    };

    event_loop.run_app(&mut app)
}
