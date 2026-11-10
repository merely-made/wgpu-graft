// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Linux zero-copy path: Servo's frame, imported by grafting onto GPUI's own
//! wgpu device, handed to bite-gpui's wgpu renderer as an imported texture.
//!
//! GPUI lends its device and queue to a `gpu_canvas` paint, so the demo paints
//! one boot frame to borrow them before Servo starts. grafting imports each
//! frame on that device (GL framebuffer to a normalized `Rgba8Unorm` texture,
//! no CPU copy). bite-gpui samples imported textures as sRGB and checks the
//! texture's own format, so each frame is copied on the GPU into a reused
//! `Rgba8UnormSrgb` texture. The copy is on GPUI's queue, so GPUI's draw is
//! ordered after it with no wait.

use std::rc::Rc;
use std::sync::Arc;

use gpui::{GpuCanvasContext, ImportedTextureHandle};
use gpui_linux::ImportedTextureExt as _;
use servo::RenderingContext;
use servo_wgpu_interop_adapter::ServoWgpuInteropAdapter;
use winit::dpi::PhysicalSize;

/// GPUI's wgpu device and queue, borrowed by the boot frame.
pub type Host = (Arc<wgpu::Device>, Arc<wgpu::Queue>);

/// `None` when GPUI's renderer is not the wgpu one.
pub fn host(canvas: &mut GpuCanvasContext) -> Option<Host> {
    canvas.try_device::<gpui_wgpu::WgpuRenderer>()
}

struct Target {
    texture: wgpu::Texture,
    handle: ImportedTextureHandle,
}

/// Servo's frames, imported by grafting and handed to GPUI each frame.
pub struct GpuImport {
    interop: ServoWgpuInteropAdapter,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    target: Option<Target>,
}

impl GpuImport {
    pub fn new(size: PhysicalSize<u32>, (device, queue): Host) -> Result<Self, String> {
        let interop =
            ServoWgpuInteropAdapter::new(device.as_ref().clone(), queue.as_ref().clone(), size)
                .map_err(|err| format!("{err:?}"))?;
        Ok(Self {
            interop,
            device,
            queue,
            target: None,
        })
    }

    pub fn rendering_context(&self) -> Rc<dyn RenderingContext> {
        self.interop.rendering_context()
    }

    /// Present Servo's painted frame and import it. Returns the frame size
    /// when a new frame arrived.
    pub fn present(&mut self) -> Result<Option<(u32, u32)>, String> {
        // The importing context imports inside present(), before the GL
        // swap chain flips.
        self.interop.rendering_context().present();
        let imported = match self.interop.take_imported_texture_result() {
            Ok(Some(imported)) => imported,
            Ok(None) => return Ok(None),
            Err(err) => return Err(format!("{err:?}")),
        };
        let extent = imported.texture.size();
        let target = self.target(extent)?;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("demo-servo-gpui srgb copy"),
            });
        encoder.copy_texture_to_texture(
            imported.texture.as_image_copy(),
            target.as_image_copy(),
            extent,
        );
        self.queue.submit([encoder.finish()]);
        Ok(Some((extent.width, extent.height)))
    }

    /// The sRGB texture GPUI samples, reallocated when the frame size changes.
    fn target(&mut self, extent: wgpu::Extent3d) -> Result<wgpu::Texture, String> {
        if let Some(target) = &self.target
            && target.texture.size() == extent
        {
            return Ok(target.texture.clone());
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("demo-servo-gpui frame"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let handle = texture
            .create_view(&wgpu::TextureViewDescriptor::default())
            .to_imported_handle()
            .map_err(|err| format!("{err:#}"))?;
        self.target = Some(Target {
            texture: texture.clone(),
            handle,
        });
        Ok(texture)
    }

    pub fn latest(&self) -> Option<ImportedTextureHandle> {
        self.target.as_ref().map(|target| target.handle.clone())
    }
}
