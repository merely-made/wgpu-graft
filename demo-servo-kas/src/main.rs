// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Demo embedding Servo in a Kas application, zero-copy.
//!
//! Kas's wgpu front end takes a *custom pipe*: application code that gets
//! Kas's own wgpu device (and queue, on resize) and draws inside Kas's render
//! pass. Servo is started on that device; each frame grafting imports Servo's
//! output as a `wgpu::Texture`, and the pipe draws it as a textured quad over
//! the [`ServoView`] widget's rectangle. No pixels reach the CPU.
//!
//! Usage:
//!   cargo run -p demo-servo-kas -- https://example.com
//!   cargo run -p demo-servo-kas            # opens built-in fixture page

mod keyutils;

use std::cell::RefCell;
use std::rc::Rc;

use euclid::Scale;
use kas::draw::{DrawIface, PassId};
use kas::event::{FocusSource, GrabMode, MouseButton, ScrollDelta};
use kas::prelude::*;
use kas::widgets::Label;
use kas_wgpu::draw::{CustomPipe, CustomPipeBuilder, CustomWindow, DrawCustom, DrawPipe};
use kas_wgpu::wgpu;
use rustls::crypto::aws_lc_rs;
use servo::{
    DevicePoint, EventLoopWaker, InputEvent, MouseButton as ServoMouseButton, MouseButtonAction,
    MouseButtonEvent, MouseMoveEvent, Servo, ServoBuilder, WebView, WebViewBuilder,
    WebViewDelegate, WheelDelta, WheelEvent, WheelMode,
};
use servo_wgpu_interop_adapter::ServoWgpuInteropAdapter;
use url::Url;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;

// ── Servo, shared by the pipe (rendering) and the widget (input) ─────────────

struct ServoState {
    servo: Servo,
    webview: WebView,
    interop: ServoWgpuInteropAdapter,
    viewport: PhysicalSize<u32>,
    scale: f32,
}

#[derive(Default)]
struct Shared {
    servo: Option<ServoState>,
    failed: bool,
}

type SharedServo = Rc<RefCell<Shared>>;

impl Shared {
    fn start(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: PhysicalSize<u32>,
        url: Url,
    ) {
        if self.servo.is_some() || self.failed {
            return;
        }
        let interop = match ServoWgpuInteropAdapter::new(device.clone(), queue.clone(), size) {
            Ok(interop) => interop,
            Err(err) => {
                eprintln!("[demo] Servo GPU import unavailable: {err:?}");
                self.failed = true;
                return;
            },
        };
        let servo = ServoBuilder::default()
            .event_loop_waker(Box::new(NoopWaker))
            .build();
        servo.setup_logging();
        let webview = WebViewBuilder::new(&servo, interop.rendering_context())
            .url(url)
            .delegate(Rc::new(DemoDelegate))
            .build();
        println!("[demo] Servo embedded on Kas's wgpu device (GPU import)");
        self.servo = Some(ServoState {
            servo,
            webview,
            interop,
            viewport: size,
            scale: 1.0,
        });
    }
}

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
}

// ── Custom pipe: import and draw ─────────────────────────────────────────────

/// Kas's common vertex uniform (`offset`, `scale`, from pixels to clip space)
/// at group 0, Servo's frame at group 1. Kas renders into `Bgra8UnormSrgb`,
/// so the sRGB-encoded page is decoded to linear before it is written.
const SHADER: &str = r"
struct VertexCommon { offset: vec2<f32>, scale: vec2<f32> };
@group(0) @binding(0) var<uniform> vc: VertexCommon;
@group(1) @binding(0) var page: texture_2d<f32>;
@group(1) @binding(1) var page_sampler: sampler;

struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>) -> VOut {
    var out: VOut;
    out.position = vec4<f32>(vc.scale * (pos + vc.offset), 0.0, 1.0);
    out.uv = uv;
    return out;
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let c = textureSample(page, page_sampler, in.uv);
    return vec4<f32>(to_linear(c.rgb), c.a);
}
";

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

struct PipeBuilder {
    shared: SharedServo,
    url: Url,
}

impl CustomPipeBuilder for PipeBuilder {
    type Pipe = Pipe;

    fn build(
        &mut self,
        device: &wgpu::Device,
        bgl_common: &wgpu::BindGroupLayout,
        tex_format: wgpu::TextureFormat,
    ) -> Pipe {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("servo page"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let bgl_page = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("servo page"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("servo page"),
            bind_group_layouts: &[bgl_common, &bgl_page],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("servo page"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                }],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: tex_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("servo page"),
            ..Default::default()
        });
        Pipe {
            shared: self.shared.clone(),
            url: self.url.clone(),
            pipeline,
            bgl_page,
            sampler,
        }
    }
}

struct Pipe {
    shared: SharedServo,
    url: Url,
    pipeline: wgpu::RenderPipeline,
    bgl_page: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

/// Per-window state: where the view was drawn this frame and what to draw.
#[derive(Default)]
struct PipeWindow {
    /// The view's pass and rectangle (physical pixels), and the scale factor.
    target: Option<(usize, Rect, f32)>,
    frame: Option<(wgpu::Texture, wgpu::BindGroup)>,
    vertices: Option<wgpu::Buffer>,
}

impl CustomWindow for PipeWindow {
    type Param = f32;

    fn invoke(&mut self, pass: PassId, rect: Rect, scale: f32) {
        self.target = Some((pass.pass(), rect, scale));
    }
}

impl CustomPipe for Pipe {
    type Window = PipeWindow;

    fn resize(
        &self,
        _window: &mut PipeWindow,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: Size,
    ) {
        // The queue only reaches the pipe here, so Servo starts on the first
        // resize, at the window size; the view's own size follows next frame.
        let size = PhysicalSize::new(size.0.max(1) as u32, size.1.max(1) as u32);
        self.shared
            .borrow_mut()
            .start(device, queue, size, self.url.clone());
    }

    fn prepare(
        &self,
        window: &mut PipeWindow,
        device: &wgpu::Device,
        _: &mut wgpu::util::StagingBelt,
        _: &mut wgpu::CommandEncoder,
    ) {
        let mut shared = self.shared.borrow_mut();
        let (Some(ss), Some((_, rect, scale))) = (shared.servo.as_mut(), window.target) else {
            return;
        };

        let size = PhysicalSize::new(rect.size.0.max(1) as u32, rect.size.1.max(1) as u32);
        if (scale - ss.scale).abs() > f32::EPSILON {
            ss.scale = scale;
            ss.webview.set_hidpi_scale_factor(Scale::new(scale));
        }
        if size != ss.viewport {
            ss.viewport = size;
            ss.webview.resize(size);
        }

        ss.servo.spin_event_loop();
        ss.webview.paint();
        // The importing context imports inside present(), before the GL swap
        // chain flips.
        ss.interop.rendering_context().present();
        if let Some(imported) = ss.interop.take_imported_texture() {
            let view = imported
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("servo page"),
                layout: &self.bgl_page,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            window.frame = Some((imported.texture, bind_group));
        }

        let (x0, y0) = (rect.pos.0 as f32, rect.pos.1 as f32);
        let (x1, y1) = (x0 + rect.size.0 as f32, y0 + rect.size.1 as f32);
        let quad = [
            Vertex {
                pos: [x0, y0],
                uv: [0.0, 0.0],
            },
            Vertex {
                pos: [x1, y0],
                uv: [1.0, 0.0],
            },
            Vertex {
                pos: [x0, y1],
                uv: [0.0, 1.0],
            },
            Vertex {
                pos: [x0, y1],
                uv: [0.0, 1.0],
            },
            Vertex {
                pos: [x1, y0],
                uv: [1.0, 0.0],
            },
            Vertex {
                pos: [x1, y1],
                uv: [1.0, 1.0],
            },
        ];
        window.vertices = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("servo page"),
                contents: bytemuck::cast_slice(&quad),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        );
    }

    fn render_pass<'a>(
        &'a self,
        window: &'a mut PipeWindow,
        _: &wgpu::Device,
        pass: usize,
        rpass: &mut wgpu::RenderPass<'a>,
        bg_common: &'a wgpu::BindGroup,
    ) {
        let (Some((target_pass, _, _)), Some((_, bind_group)), Some(vertices)) = (
            window.target,
            window.frame.as_ref(),
            window.vertices.as_ref(),
        ) else {
            return;
        };
        if target_pass != pass {
            return;
        }
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, bg_common, &[]);
        rpass.set_bind_group(1, bind_group, &[]);
        rpass.set_vertex_buffer(0, vertices.slice(..));
        rpass.draw(0..6, 0..1);
    }
}

// ── The Servo view widget: layout, drawing, input ────────────────────────────

#[impl_self]
mod ServoView {
    #[widget]
    struct ServoView {
        core: widget_core!(),
        shared: SharedServo,
        scale: f32,
        /// Last pointer position, for scroll events (which carry none).
        pointer: Coord,
    }

    impl Self {
        fn new(shared: SharedServo) -> Self {
            ServoView {
                core: Default::default(),
                shared,
                scale: 1.0,
                pointer: Coord::ZERO,
            }
        }

        /// A window coordinate as a Servo point in this view.
        fn servo_point(&self, coord: Coord) -> DevicePoint {
            let rect = self.rect();
            DevicePoint::new((coord.0 - rect.pos.0) as f32, (coord.1 - rect.pos.1) as f32)
        }

        fn notify(&self, event: InputEvent) {
            if let Some(ss) = self.shared.borrow().servo.as_ref() {
                ss.webview.notify_input_event(event);
            }
        }
    }

    impl Layout for Self {
        fn size_rules(&mut self, cx: &mut SizeCx, axis: AxisInfo) -> SizeRules {
            // Ideal size is the shared demo window size less the label row
            // (about 24 logical px); Kas sizes the window from it.
            let width = demo_support::WINDOW_WIDTH as f32;
            let height = demo_support::WINDOW_HEIGHT as f32 - 24.0;
            cx.logical(width / 4.0, height / 4.0)
                .with_ideal_factor(4.0)
                .with_stretch(Stretch::High)
                .build(axis)
        }

        fn set_rect(&mut self, cx: &mut SizeCx, rect: Rect, _: AlignHints) {
            self.core.set_rect(rect);
            self.scale = cx.scale_factor();
        }

        fn draw(&self, mut draw: DrawCx) {
            let draw = draw.draw();
            // Live web content: draw every frame.
            draw.animate();
            let pass = draw.get_pass();
            let draw = DrawIface::<DrawPipe<Pipe>>::downcast_from(draw).unwrap();
            draw.draw.custom(pass, self.rect(), self.scale);
        }
    }

    impl Tile for Self {
        fn role(&self, _: &mut dyn RoleCx) -> Role<'_> {
            Role::Unknown
        }

        fn navigable(&self) -> bool {
            true
        }
    }

    impl Events for Self {
        type Data = ();

        fn handle_event(&mut self, cx: &mut EventCx, _: &(), event: Event) -> IsUsed {
            match event {
                Event::PointerMove { press } => {
                    self.pointer = press.coord;
                    let pt = self.servo_point(press.coord);
                    self.notify(InputEvent::MouseMove(MouseMoveEvent::new(pt.into())));
                    Unused
                },
                Event::PressStart(press) => {
                    let Some(button) = press.source.mouse_button().and_then(map_button) else {
                        return Unused;
                    };
                    let pt = self.servo_point(press.coord());
                    self.notify(InputEvent::MouseButton(MouseButtonEvent::new(
                        MouseButtonAction::Down,
                        button,
                        pt.into(),
                    )));
                    cx.request_key_focus(self.id(), FocusSource::Pointer);
                    press.grab(self.id(), GrabMode::Grab).complete(cx)
                },
                Event::PressMove { press, .. } => {
                    self.pointer = press.coord;
                    let pt = self.servo_point(press.coord);
                    self.notify(InputEvent::MouseMove(MouseMoveEvent::new(pt.into())));
                    Used
                },
                Event::PressEnd { press, .. } => {
                    if let Some(button) = press.source.mouse_button().and_then(map_button) {
                        let pt = self.servo_point(press.coord);
                        self.notify(InputEvent::MouseButton(MouseButtonEvent::new(
                            MouseButtonAction::Up,
                            button,
                            pt.into(),
                        )));
                    }
                    Used
                },
                Event::Scroll(delta) => {
                    let (dx, dy, mode) = match delta {
                        ScrollDelta::Lines(x, y) => (
                            f64::from(x) * 38.0,
                            f64::from(y) * 38.0,
                            WheelMode::DeltaLine,
                        ),
                        ScrollDelta::PixelDelta(d) => {
                            (f64::from(d.0), f64::from(d.1), WheelMode::DeltaPixel)
                        },
                    };
                    let pt = self.servo_point(self.pointer);
                    self.notify(InputEvent::Wheel(WheelEvent::new(
                        WheelDelta {
                            x: dx,
                            y: dy,
                            z: 0.0,
                            mode,
                        },
                        pt.into(),
                    )));
                    Used
                },
                Event::Key(key_event, _) => {
                    let kbd = keyutils::keyboard_event_from_winit(key_event, cx.modifiers());
                    self.notify(InputEvent::Keyboard(kbd));
                    Used
                },
                _ => Unused,
            }
        }
    }
}

#[impl_self]
mod DemoUi {
    #[widget]
    #[layout(column![self.label, self.view])]
    struct DemoUi {
        core: widget_core!(),
        #[widget]
        label: Label<&'static str>,
        #[widget]
        view: ServoView,
    }

    impl Events for Self {
        type Data = ();
    }
}

fn map_button(button: MouseButton) -> Option<ServoMouseButton> {
    Some(match button {
        MouseButton::Left => ServoMouseButton::Primary,
        MouseButton::Right => ServoMouseButton::Secondary,
        MouseButton::Middle => ServoMouseButton::Auxiliary,
        MouseButton::Back => ServoMouseButton::Back,
        MouseButton::Forward => ServoMouseButton::Forward,
        _ => return None,
    })
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() -> kas::runner::Result<()> {
    aws_lc_rs::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    // On Windows the import opens ANGLE's shared texture on a DX12 device.
    // Kas picks its wgpu backends from KAS_BACKENDS (not WGPU_BACKEND), and
    // defaults to the low-power GPU; ANGLE could not be matched to the
    // integrated GPU on a hybrid laptop, so ask for the high-performance one,
    // as the Iced and Bevy demos do.
    // SAFETY: set before any wgpu or thread initialisation.
    #[cfg(target_os = "windows")]
    unsafe {
        std::env::set_var("KAS_BACKENDS", "DX12");
        std::env::set_var("KAS_POWER_PREFERENCE", "HighPerformance");
    }

    let url = demo_support::resolve_initial_url(env!("CARGO_MANIFEST_DIR"))
        .expect("failed to resolve initial URL");
    let shared = SharedServo::default();

    let ui = DemoUi {
        core: Default::default(),
        label: Label::new("Servo in Kas — drawn through a Kas custom pipe (GPU import)"),
        view: ServoView::new(shared.clone()),
    };
    let window = Window::new(ui, "demo-servo-kas");

    let mut runner = kas::runner::Runner::with_wgpu_pipe(PipeBuilder { shared, url })
        .with_theme(kas::theme::FlatTheme::new())
        .build(())?;
    let _ = runner.config_mut().theme.set_active_scheme("dark");
    runner.with(window).run()
}
