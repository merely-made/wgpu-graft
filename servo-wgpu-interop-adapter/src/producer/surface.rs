// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

use std::{collections::HashMap, rc::Rc};

use grafting::TextureOrigin;
use servo::{
    DevicePoint, InputEvent, Key, KeyState, Location, Modifiers, MouseButtonAction,
    MouseButtonEvent, MouseMoveEvent, RenderingContext, WebView, WebViewPoint, WheelDelta,
    WheelEvent, WheelMode,
};
use url::Url;
use winit::dpi::PhysicalSize;

use super::{
    delegate::Delegate,
    keys::logical_key,
    process::ProcessRoot,
    types::{
        ServoCursor, ServoEvent, ServoFrame, ServoFramePayload, ServoImportedFrame, ServoKeyEvent,
        ServoMouseButton, ServoMouseEvent, ServoMouseKind, ServoProducerConfig, ServoProducerError,
    },
};

#[cfg(target_os = "linux")]
pub(super) struct Interop(crate::ServoDmaBufAdapter);
#[cfg(not(target_os = "linux"))]
pub(super) struct Interop(crate::ServoWgpuInteropAdapter);

impl Interop {
    #[cfg_attr(target_os = "linux", allow(unused_variables))]
    pub(super) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: PhysicalSize<u32>,
        config: &ServoProducerConfig,
    ) -> Result<Self, ServoProducerError> {
        #[cfg(target_os = "linux")]
        let adapter = crate::ServoDmaBufAdapter::new(device, size);
        #[cfg(not(target_os = "linux"))]
        let adapter = crate::ServoWgpuInteropAdapter::new_with_diagnostic_sync(
            device.clone(),
            queue.clone(),
            size,
            config.gpu_sync,
        );
        adapter
            .map(Self)
            .map_err(|error| ServoProducerError::Spawn(error.to_string()))
    }

    pub(super) fn rendering_context(&self) -> Rc<dyn RenderingContext> {
        self.0.rendering_context()
    }
}

/// One Servo view rendering offscreen onto the host device. Not `Send`: it
/// owns Servo's non-`Send` GL context, and every view sharing the process
/// root is driven from the UI thread.
pub struct ServoSurface {
    pub(super) view: Option<WebView>,
    pub(super) interop: Option<Interop>,
    pub(super) delegate: Rc<Delegate>,
    pub(super) root: Rc<ProcessRoot>,
    pub(super) frame_epoch: u64,
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    pub(super) pressed_keys: HashMap<u32, Key>,
}

impl Drop for ServoSurface {
    fn drop(&mut self) {
        self.delegate.deactivate_accessibility();
        if let Some(view) = &self.view {
            view.set_accessibility_active(false);
        }
        drop(self.view.take());
        self.root.servo.spin_event_loop();
        drop(self.interop.take());
        self.root.views.set(self.root.views.get() - 1);
    }
}

impl ServoSurface {
    fn view(&self) -> &WebView {
        self.view.as_ref().expect("live Servo surface has a view")
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.view()
            .resize(PhysicalSize::new(width.max(1), height.max(1)));
    }

    /// Pump Servo and return the latest composited frame, if a new one is
    /// ready. `Ok(None)` when nothing new this tick.
    ///
    /// # Errors
    /// Import or normalization failures, and frames that disagree with their
    /// own metadata.
    #[cfg(target_os = "linux")]
    pub fn acquire_frame(&mut self) -> Result<Option<ServoFrame>, ServoProducerError> {
        let frame_error = |e: grafting::InteropError| ServoProducerError::Frame(e.to_string());
        self.root.servo.spin_event_loop();
        let Some(Interop(interop)) = self.interop.as_ref() else {
            return Ok(None);
        };
        let mut native = interop.try_acquire_frame().map_err(frame_error)?;
        if native.is_none() {
            if interop.has_pending_frame() {
                return Ok(None);
            }
            if !self.delegate.ready.replace(false) && !self.delegate.animating.get() {
                return Ok(None);
            }
            self.view().paint();
            interop.rendering_context().present();
            native = interop.try_acquire_frame().map_err(frame_error)?;
        }
        let Some(frame) = native else {
            return Ok(None);
        };
        let metadata = frame.metadata();
        self.frame_epoch = metadata.generation;
        Ok(Some(ServoFrame {
            width: metadata.size.width,
            height: metadata.size.height,
            resource_epoch: metadata.generation,
            payload: ServoFramePayload::DmaBuf(frame),
        }))
    }

    /// Pump Servo and return the latest composited frame, if a new one is
    /// ready. `Ok(None)` when nothing new this tick.
    ///
    /// # Errors
    /// Import or normalization failures, and frames that disagree with their
    /// own metadata.
    #[cfg(not(target_os = "linux"))]
    pub fn acquire_frame(&mut self) -> Result<Option<ServoFrame>, ServoProducerError> {
        let frame_error = |detail: String| ServoProducerError::Frame(detail);
        self.root.servo.spin_event_loop();
        if !self.delegate.ready.replace(false) && !self.delegate.animating.get() {
            return Ok(None);
        }
        self.view().paint();
        let interop = &self.interop.as_ref().expect("live Servo surface").0;
        // Upstream WebView::paint draws without presenting. Present through
        // the importing context so its hook runs before the GL buffer swap.
        interop.rendering_context().present();
        let imported = interop
            .take_imported_texture_result()
            .map_err(|error| frame_error(error.to_string()))?
            .ok_or_else(|| frame_error("Servo pre-present import produced no frame".into()))?;
        let actual_size = imported.texture.size();
        if imported.origin != TextureOrigin::TopLeft
            || imported.format != wgpu::TextureFormat::Rgba8Unorm
            || imported.texture.format() != imported.format
            || imported.texture.dimension() != wgpu::TextureDimension::D2
            || imported.size.width == 0
            || imported.size.height == 0
            || (
                actual_size.width,
                actual_size.height,
                actual_size.depth_or_array_layers,
            ) != (imported.size.width, imported.size.height, 1)
            || !imported
                .texture
                .usage()
                .contains(wgpu::TextureUsages::TEXTURE_BINDING)
        {
            return Err(frame_error(
                "Servo import disagrees with its normalized texture metadata".into(),
            ));
        }
        self.frame_epoch = self
            .frame_epoch
            .checked_add(1)
            .ok_or_else(|| frame_error("Servo frame epoch exhausted".into()))?;
        let (width, height) = (imported.size.width, imported.size.height);
        Ok(Some(ServoFrame {
            payload: ServoFramePayload::Imported(ServoImportedFrame {
                imported,
                device: self.device.clone(),
                queue: self.queue.clone(),
                resource_epoch: self.frame_epoch,
            }),
            width,
            height,
            resource_epoch: self.frame_epoch,
        }))
    }

    /// # Errors
    /// The URL does not parse.
    pub fn load_url(&mut self, url: &str) -> Result<(), ServoProducerError> {
        let url =
            Url::parse(url).map_err(|error| ServoProducerError::Navigation(error.to_string()))?;
        *self.delegate.requested_url.borrow_mut() = url.to_string();
        self.view().load(url);
        Ok(())
    }

    pub fn reload(&mut self) {
        self.view().reload();
    }

    pub fn go_back(&mut self) {
        self.view().go_back(1);
    }

    pub fn go_forward(&mut self) {
        self.view().go_forward(1);
    }

    #[must_use]
    pub fn can_go_back(&self) -> bool {
        self.view().can_go_back()
    }

    #[must_use]
    pub fn can_go_forward(&self) -> bool {
        self.view().can_go_forward()
    }

    pub fn send_mouse(&mut self, event: ServoMouseEvent) {
        let point = WebViewPoint::Device(DevicePoint::new(event.x, event.y));
        let event = match event.kind {
            ServoMouseKind::Moved => InputEvent::MouseMove(MouseMoveEvent::new(point)),
            ServoMouseKind::Button { button, pressed } => {
                let button = match button {
                    ServoMouseButton::Left => servo::MouseButton::Primary,
                    ServoMouseButton::Middle => servo::MouseButton::Auxiliary,
                    ServoMouseButton::Right => servo::MouseButton::Secondary,
                    ServoMouseButton::Back => servo::MouseButton::Back,
                    ServoMouseButton::Forward => servo::MouseButton::Forward,
                };
                let action = if pressed {
                    MouseButtonAction::Down
                } else {
                    MouseButtonAction::Up
                };
                InputEvent::MouseButton(MouseButtonEvent::new(action, button, point))
            },
            ServoMouseKind::ScrollPixels { dx, dy } | ServoMouseKind::ScrollLines { dx, dy } => {
                let mode = if matches!(event.kind, ServoMouseKind::ScrollLines { .. }) {
                    WheelMode::DeltaLine
                } else {
                    WheelMode::DeltaPixel
                };
                InputEvent::Wheel(WheelEvent::new(
                    WheelDelta {
                        x: f64::from(dx),
                        y: f64::from(dy),
                        z: 0.0,
                        mode,
                    },
                    point,
                ))
            },
        };
        self.view().notify_input_event(event);
    }

    pub fn send_key(&mut self, event: &ServoKeyEvent) {
        let key = logical_key(event, &mut self.pressed_keys);
        let mut modifiers = Modifiers::empty();
        modifiers.set(Modifiers::SHIFT, event.modifiers.shift);
        modifiers.set(Modifiers::CONTROL, event.modifiers.ctrl);
        modifiers.set(Modifiers::ALT, event.modifiers.alt);
        modifiers.set(Modifiers::META, event.modifiers.meta);
        self.view().notify_input_event(InputEvent::Keyboard(
            servo::KeyboardEvent::new_without_event(
                if event.pressed {
                    KeyState::Down
                } else {
                    KeyState::Up
                },
                key,
                servo::Code::Unidentified,
                Location::Standard,
                modifiers,
                false,
                false,
            ),
        ));
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.view().set_focused(focused);
    }

    /// Drain the next event in upstream callback order.
    pub fn poll_event(&mut self) -> Option<ServoEvent> {
        self.delegate.events.borrow_mut().pop_front()
    }

    pub fn poll_cursor(&mut self) -> Option<ServoCursor> {
        self.delegate.cursors.borrow_mut().pop_front()
    }

    /// Request native zoom and return the effective factor Servo reports.
    pub fn set_page_zoom(&mut self, factor: f32) -> f32 {
        self.view().set_page_zoom(factor);
        self.view().page_zoom()
    }

    /// Activate or deactivate semantic export. Activation returns the guest
    /// root tree identity as its 128-bit UUID value; callbacks are queued even
    /// if upstream invokes the delegate before this returns, so the host
    /// admits the returned root before draining them.
    ///
    /// # Errors
    /// Upstream refused activation.
    pub fn set_accessibility_active(
        &mut self,
        active: bool,
    ) -> Result<Option<u128>, ServoProducerError> {
        if !active {
            self.delegate.deactivate_accessibility();
            self.view().set_accessibility_active(false);
            return Ok(None);
        }
        self.delegate.accessibility_active.set(true);
        match self.view().set_accessibility_active(true) {
            Some(tree) => Ok(Some(tree.0.as_u128())),
            None => {
                self.delegate.deactivate_accessibility();
                Err(ServoProducerError::Unsupported(
                    "accessibility activation".into(),
                ))
            },
        }
    }

    /// Reactivate to request initial trees. Upstream has no resend API;
    /// reactivation creates a new wrapper tree id, which is returned.
    ///
    /// # Errors
    /// Upstream refused reactivation.
    pub fn request_accessibility_resync(&mut self) -> Result<u128, ServoProducerError> {
        self.set_accessibility_active(false)?;
        self.set_accessibility_active(true)?.ok_or_else(|| {
            ServoProducerError::Unsupported("accessibility resynchronization".into())
        })
    }

    /// Next accessibility update with the focus policy applied, serialized to
    /// its AccessKit serde form so hosts on another AccessKit version can
    /// deserialize it. A conversion failure retires the forest: a dropped
    /// delta would leave the host applying later deltas to the wrong tree.
    pub fn poll_accessibility_update(&mut self) -> Option<serde_json::Value> {
        let update = self
            .delegate
            .poll_projected_accessibility_update(self.root.config.a11y_focus)?;
        match serde_json::to_value(&update) {
            Ok(value) => Some(value),
            Err(error) => {
                tracing::error!(target: "grafting_servo::a11y",
                    %error, "Servo accessibility update did not serialize");
                self.delegate.deactivate_accessibility();
                None
            },
        }
    }
}
