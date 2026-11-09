// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! macOS zero-copy path: Servo's frame, imported by grafting, handed to
//! bite-gpui's Metal renderer as an imported texture.
//!
//! GPUI's macOS renderer is Metal, not wgpu, so it has no wgpu device to lend.
//! macOS hands the one `MTLDevice` to every creator, so a wgpu device made
//! here lands on the same `MTLDevice` GPUI draws with. grafting imports each
//! Servo frame onto that device (IOSurface to a normalized `Rgba8Unorm`
//! texture, no CPU copy), and GPUI samples the texture's own `id<MTLTexture>`
//! through an sRGB view: Servo's bytes are already sRGB-encoded, and bite-gpui
//! decodes the sample and re-encodes it into its target.

use std::collections::VecDeque;

use foreign_types::ForeignTypeRef as _;
use gpui::ImportedTextureHandle;
use gpui_macos::MetalTextureExt as _;
use metal::MTLPixelFormat;

/// How many imported frames stay alive. bite-gpui's handle carries the
/// `id<MTLTexture>` by pointer, so the texture must outlive the frame that
/// samples it; keeping the previous one too covers a frame still in flight.
const FRAMES_KEPT: usize = 2;

/// A wgpu device on the Metal device GPUI's renderer uses.
pub fn wgpu_device() -> Result<(wgpu::Device, wgpu::Queue), String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::METAL,
        flags: wgpu::InstanceFlags::default(),
        backend_options: wgpu::BackendOptions::default(),
        memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
        display: None,
    });
    // Low power matches the device bite-gpui's MetalRenderer picks on a
    // machine with two GPUs; on one GPU every preference lands on it.
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        ..Default::default()
    }))
    .map_err(|err| format!("no Metal adapter: {err}"))?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("demo-servo-gpui"),
        ..Default::default()
    }))
    .map_err(|err| format!("no wgpu device: {err}"))
}

struct Frame {
    // Field order is drop order: the sRGB view goes before the texture it views.
    _view: metal::Texture,
    _texture: wgpu::Texture,
    handle: ImportedTextureHandle,
}

/// The recent imported frames, newest last.
#[derive(Default)]
pub struct MetalFrames {
    frames: VecDeque<Frame>,
}

impl MetalFrames {
    /// Wrap a grafting-imported texture for bite-gpui.
    ///
    /// Waits for the import's GPU work to finish: GPUI samples on its own
    /// command queue, which nothing orders after wgpu's. The wait is GPU to
    /// GPU completion only; no pixels cross to the CPU.
    pub fn push(&mut self, device: &wgpu::Device, texture: wgpu::Texture) -> Result<(), String> {
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|err| format!("waiting for the import: {err}"))?;

        // SAFETY: the texture was created by this wgpu device on the Metal
        // backend; the view retains its own reference to the MTLTexture.
        let view = unsafe {
            let hal = texture
                .as_hal::<wgpu::hal::api::Metal>()
                .ok_or("the imported texture has no Metal handle")?;
            let raw = metal::TextureRef::from_ptr(hal.raw_handle() as *const _ as *mut _);
            // An sRGB reinterpretation of an RGBA8 texture needs no
            // pixel-format-view usage on Metal.
            raw.new_texture_view(MTLPixelFormat::RGBA8Unorm_sRGB)
        };
        let handle = view.to_imported_handle().map_err(|err| format!("{err:#}"))?;

        self.frames.push_back(Frame {
            _view: view,
            _texture: texture,
            handle,
        });
        while self.frames.len() > FRAMES_KEPT {
            self.frames.pop_front();
        }
        Ok(())
    }

    /// The newest frame's handle, if any frame has arrived.
    pub fn latest(&self) -> Option<ImportedTextureHandle> {
        self.frames.back().map(|frame| frame.handle.clone())
    }
}
