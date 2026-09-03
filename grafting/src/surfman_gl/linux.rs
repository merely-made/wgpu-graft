// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use crate::{
    GlFramebufferSource, HostWgpuContext, ImportOptions, ImportedTexture, InteropError,
    SyncMechanism, TextureOrigin,
};

use super::SurfmanGlFrameSource;

pub(super) fn import_current_frame(
    source: &SurfmanGlFrameSource,
    frame: &GlFramebufferSource,
    host: &HostWgpuContext,
    _options: &ImportOptions,
) -> Result<ImportedTexture, InteropError> {
    let device = &source.context.device.borrow();
    let mut context = source.context.context.borrow_mut();

    let surface = device
        .unbind_surface_from_context(&mut context)
        .map_err(|err| InteropError::Surfman(format!("{err:?}")))?
        .ok_or(InteropError::InvalidFrame("no surfman surface available"))?;

    device
        .make_context_current(&mut context)
        .map_err(|err| InteropError::Surfman(format!("{err:?}")))?;

    let surface_info = device.surface_info(&surface);
    let source_fbo = surface_info
        .framebuffer_object
        .map(|fb| fb.0.get())
        .unwrap_or(0);

    let result = crate::raw_gl::linux::import_gl_framebuffer_vulkan(
        &source.context.glow_gl,
        &|name| device.get_proc_address(&context, name),
        source_fbo,
        source.size,
        host,
    );

    // Always attempt to rebind the surface so the surfman context stays valid
    // for the next frame, even if the import failed.
    let rebind = device
        .bind_surface_to_context(&mut context, surface)
        .map_err(|(err, mut surface)| {
            let _ = device.destroy_surface(&mut context, &mut surface);
            InteropError::Surfman(format!("rebind after import failed: {err:?}"))
        });

    let texture = result?;
    rebind?;

    Ok(ImportedTexture {
        texture,
        format: wgpu::TextureFormat::Rgba8Unorm,
        size: frame.size(),
        origin: TextureOrigin::TopLeft,
        generation: source.generation,
        consumer_sync: SyncMechanism::ImplicitGlFlush,
    })
}
