// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Direct `MTLTexture` → `wgpu::Texture` import path for Metal producers.
//!
//! Unlike [`crate::raw_gl::metal`], which imports a GL framebuffer through
//! IOSurface, this path wraps a raw `MTLTexture` pointer directly. The safe
//! path transfers a retained texture into [`crate::MetalTextureRef`], which is
//! consumed by import. The explicitly unsafe borrowed-pointer escape hatch
//! retains the producer-owned texture internally for the import.

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::MTLTexture;
#[cfg(any(feature = "wgpu-29", feature = "wgpu-30"))]
use objc2_metal::MTLTextureType;

#[cfg(all(
    feature = "wgpu-28",
    not(feature = "wgpu-29"),
    not(feature = "wgpu-30")
))]
use foreign_types_shared::ForeignType;

use crate::{HostWgpuContext, InteropBackend, InteropError, MetalTextureRef};

/// Import a borrowed `MTLTexture *` without transferring the producer's
/// retain.
///
/// Prefer [`import_metal_texture_ref`], whose safe frame carries a retained
/// texture. This escape hatch exists for producers that only expose a raw
/// Objective-C pointer.
///
/// # Safety
///
/// `raw_metal_texture` must be a non-null `MTLTexture *` that remains valid
/// until this function has retained it. The caller retains all ownership and
/// synchronization responsibilities outside this call.
pub unsafe fn import_metal_texture_borrowed(
    raw_metal_texture: *mut std::ffi::c_void,
    metadata: crate::FrameMetadata,
    host: &HostWgpuContext,
) -> Result<wgpu::Texture, InteropError> {
    let texture =
        unsafe { Retained::retain(raw_metal_texture.cast::<ProtocolObject<dyn MTLTexture>>()) }
            .ok_or_else(|| InteropError::InvalidFrame("raw_metal_texture is null"))?;
    import_metal_texture_ref(MetalTextureRef::new(metadata, texture), host)
}

pub fn import_metal_texture_ref(
    frame: MetalTextureRef,
    host: &HostWgpuContext,
) -> Result<wgpu::Texture, InteropError> {
    if host.backend != InteropBackend::Metal {
        return Err(InteropError::BackendMismatch {
            expected: "Metal",
            actual: "non-Metal",
        });
    }

    let metadata = frame.metadata();
    let texture = unsafe {
        // The safe frame owns one Objective-C retain. Move that exact retain
        // into wgpu rather than borrowing a raw pointer across this boundary.
        let retained = frame.raw_metal_texture;
        #[cfg(all(
            feature = "wgpu-28",
            not(feature = "wgpu-29"),
            not(feature = "wgpu-30")
        ))]
        let metal_texture = metal::Texture::from_ptr(Retained::into_raw(retained).cast());
        #[cfg(any(feature = "wgpu-29", feature = "wgpu-30"))]
        let metal_texture = retained;

        let copy_size = wgpu::hal::CopyExtent {
            width: metadata.size.width,
            height: metadata.size.height,
            depth: 1,
        };
        // hal 30 added a `drop_callback` parameter. The moved retain gives
        // wgpu its texture lifetime; no callback is needed for this transfer.
        #[cfg(feature = "wgpu-30")]
        let hal_texture = wgpu::hal::metal::Device::texture_from_raw(
            metal_texture,
            metadata.format,
            MTLTextureType::Type2D,
            1, // array_layers
            1, // mip_levels
            copy_size,
            None, // drop_callback
        );
        #[cfg(all(feature = "wgpu-29", not(feature = "wgpu-30")))]
        let hal_texture = wgpu::hal::metal::Device::texture_from_raw(
            metal_texture,
            metadata.format,
            MTLTextureType::Type2D,
            1, // array_layers
            1, // mip_levels
            copy_size,
        );
        #[cfg(all(
            feature = "wgpu-28",
            not(feature = "wgpu-29"),
            not(feature = "wgpu-30")
        ))]
        let hal_texture = wgpu::hal::metal::Device::texture_from_raw(
            metal_texture,
            metadata.format,
            metal::MTLTextureType::D2,
            1, // array_layers
            1, // mip_levels
            copy_size,
        );

        crate::wgpu_compat::create_texture_from_hal::<wgpu_hal::api::Metal>(
            &host.device,
            hal_texture,
            &wgpu::TextureDescriptor {
                label: Some("metal-texture-ref-import"),
                size: wgpu::Extent3d {
                    width: metadata.size.width,
                    height: metadata.size.height,
                    depth_or_array_layers: 1,
                },
                format: metadata.format,
                dimension: wgpu::TextureDimension::D2,
                mip_level_count: 1,
                sample_count: 1,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            },
            wgpu::TextureUses::RESOURCE,
        )
    };

    Ok(texture)
}
