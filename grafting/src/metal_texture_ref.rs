//! Direct `MTLTexture` → `wgpu::Texture` import path for Metal producers.
//!
//! Unlike [`crate::raw_gl::metal`], which imports a GL framebuffer through
//! IOSurface, this path wraps a raw `MTLTexture` pointer directly. The
//! producer retains ownership of the underlying texture — the importer takes
//! a +1 retain count and hands it to wgpu via `texture_from_raw`.

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

pub fn import_metal_texture_ref(
    frame: &MetalTextureRef,
    host: &HostWgpuContext,
) -> Result<wgpu::Texture, InteropError> {
    if frame.raw_metal_texture.is_null() {
        return Err(InteropError::InvalidFrame("raw_metal_texture is null"));
    }
    if host.backend != InteropBackend::Metal {
        return Err(InteropError::BackendMismatch {
            expected: "Metal",
            actual: "non-Metal",
        });
    }

    let texture = unsafe {
        // Retain the caller's MTLTexture so that wgpu can take ownership
        // of the reference we hand it without invalidating the caller's copy.
        let obj_ptr = frame.raw_metal_texture as *mut ProtocolObject<dyn MTLTexture>;
        let retained = Retained::retain(obj_ptr)
            .ok_or_else(|| InteropError::Metal("failed to retain Metal texture".into()))?;
        #[cfg(all(
            feature = "wgpu-28",
            not(feature = "wgpu-29"),
            not(feature = "wgpu-30")
        ))]
        let metal_texture = metal::Texture::from_ptr(Retained::into_raw(retained).cast());
        #[cfg(any(feature = "wgpu-29", feature = "wgpu-30"))]
        let metal_texture = retained;

        let copy_size = wgpu::hal::CopyExtent {
            width: frame.size.width,
            height: frame.size.height,
            depth: 1,
        };
        // hal 30 added a `drop_callback` parameter. None preserves the old
        // behavior: the caller keeps ownership of the underlying MTLTexture,
        // and the retain above is what keeps it alive for wgpu's copy.
        #[cfg(feature = "wgpu-30")]
        let hal_texture = wgpu::hal::metal::Device::texture_from_raw(
            metal_texture,
            frame.format,
            MTLTextureType::Type2D,
            1, // array_layers
            1, // mip_levels
            copy_size,
            None, // drop_callback
        );
        #[cfg(all(feature = "wgpu-29", not(feature = "wgpu-30")))]
        let hal_texture = wgpu::hal::metal::Device::texture_from_raw(
            metal_texture,
            frame.format,
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
            frame.format,
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
                    width: frame.size.width,
                    height: frame.size.height,
                    depth_or_array_layers: 1,
                },
                format: frame.format,
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
