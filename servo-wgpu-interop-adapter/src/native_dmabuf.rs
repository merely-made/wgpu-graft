// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

//! Servo's pre-present GL snapshot enters the shared native frame boundary.
use super::*;
use grafting::{NeutralNativeFrame, surfman_gl::PendingDmaBufFrame};

/// Linux producer that exports completed DMA-BUF snapshots without importing
/// on a host device or reading pixels through the CPU. Each acquisition polls
/// once; incomplete GPU work remains private to this producer.
pub struct ServoDmaBufAdapter {
    context: Rc<NativeRenderingContext>,
}
impl ServoDmaBufAdapter {
    pub fn new(device: &wgpu::Device, size: PhysicalSize<u32>) -> Result<Self, InteropError> {
        let inner = Rc::new(ServoWgpuRenderingContext::new_for_device(size, device)?);
        Ok(Self {
            context: Rc::new(NativeRenderingContext {
                inner,
                pending: RefCell::new(None),
                error: RefCell::new(None),
            }),
        })
    }
    pub fn rendering_context(&self) -> Rc<dyn RenderingContext> {
        self.context.clone()
    }
    pub fn has_pending_frame(&self) -> bool {
        self.context.pending.borrow().is_some()
    }
    pub fn try_acquire_frame(&self) -> Result<Option<NeutralNativeFrame>, InteropError> {
        if let Some(error) = self.context.error.borrow_mut().take() {
            return Err(error);
        }
        let mut pending = self.context.pending.borrow_mut();
        let result = match pending.as_mut() {
            Some(p) => p.try_take_frame()?,
            None => return Ok(None),
        };
        if result.is_some() {
            pending.take();
        }
        Ok(result)
    }
}
struct NativeRenderingContext {
    inner: Rc<ServoWgpuRenderingContext>,
    pending: RefCell<Option<PendingDmaBufFrame>>,
    error: RefCell<Option<InteropError>>,
}
impl RenderingContext for NativeRenderingContext {
    fn prepare_for_rendering(&self) {
        self.inner.prepare_for_rendering();
    }
    fn read_to_image(&self, region: DeviceIntRect) -> Option<image::RgbaImage> {
        self.inner.read_to_image(region)
    }
    fn size(&self) -> PhysicalSize<u32> {
        self.inner.size()
    }
    fn resize(&self, size: PhysicalSize<u32>) {
        self.pending.borrow_mut().take();
        self.inner.resize(size);
    }
    fn present(&self) {
        self.pending.borrow_mut().take();
        self.error.borrow_mut().take();
        match self.inner.frame_producer.borrow().export_dmabuf_frame() {
            Ok(frame) => *self.pending.borrow_mut() = Some(frame),
            Err(error) => *self.error.borrow_mut() = Some(error),
        }
        self.inner.present_without_wait();
    }
    fn make_current(&self) -> Result<(), surfman::Error> {
        self.inner.make_current()
    }
    fn gleam_gl_api(&self) -> Rc<dyn gleam::gl::Gl> {
        self.inner.gleam_gl_api()
    }
    fn glow_gl_api(&self) -> std::sync::Arc<glow::Context> {
        self.inner.glow_gl_api()
    }
    fn create_texture(&self, surface: Surface) -> Option<(SurfaceTexture, u32, Size2D<i32>)> {
        self.inner.create_texture(surface)
    }
    fn destroy_texture(&self, texture: SurfaceTexture) -> Option<Surface> {
        self.inner.destroy_texture(texture)
    }
    fn connection(&self) -> Option<Connection> {
        self.inner.connection()
    }
}
