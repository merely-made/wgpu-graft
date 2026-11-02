// SPDX-License-Identifier: MPL-2.0
//! IOSurface storage for host-owned Metal handoffs (wgpu 30, macOS).
//!
//! Transport must move an actual Mach send right, never its process-local name.
//! Readiness is a separate contract: import only after producer GPU completion
//! has been observed. Release requires both a sealed consumer queue fence and
//! destruction of every wgpu reference to the imported texture.
use crate::{FrameSize, HostWgpuContext, InteropError, OwnedIoSurface, PixelFormat, wgpu};
use objc2_core_foundation::{CFDictionary, CFNumber, CFType};
use objc2_io_surface::{
    IOSurfaceRef, kIOSurfaceBytesPerElement, kIOSurfaceHeight, kIOSurfacePixelFormat,
    kIOSurfaceWidth,
};
use objc2_metal::{
    MTLDevice, MTLPixelFormat, MTLTextureDescriptor, MTLTextureType, MTLTextureUsage,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

unsafe extern "C" {
    static mach_task_self_: u32;
    fn mach_port_deallocate(task: u32, name: u32) -> i32;
}

/// One owned Mach send-right reference in this task's namespace.
#[derive(Debug)]
pub struct MachSendRight(u32);
impl MachSendRight {
    /// Takes exactly one send-right reference, as received in a Mach message.
    ///
    /// # Safety
    /// `name` must denote one live, owned send-right reference in this task.
    /// Ownership must not also be adopted or deallocated by another object.
    pub unsafe fn from_raw(name: u32) -> Self {
        Self(name)
    }
    pub fn as_raw(&self) -> u32 {
        self.0
    }
}
impl Drop for MachSendRight {
    fn drop(&mut self) {
        // SAFETY: construction transfers exactly one live send-right reference.
        unsafe {
            mach_port_deallocate(mach_task_self_, self.0);
        }
    }
}

fn format(format: PixelFormat) -> Result<(wgpu::TextureFormat, MTLPixelFormat, u32), InteropError> {
    match format {
        PixelFormat::Rgba8Unorm => Ok((
            wgpu::TextureFormat::Rgba8Unorm,
            MTLPixelFormat::RGBA8Unorm,
            u32::from_be_bytes(*b"RGBA"),
        )),
        PixelFormat::Bgra8Unorm => Ok((
            wgpu::TextureFormat::Bgra8Unorm,
            MTLPixelFormat::BGRA8Unorm,
            u32::from_be_bytes(*b"BGRA"),
        )),
        _ => Err(InteropError::InvalidFrame(
            "IOSurface handoff supports linear RGBA8/BGRA8 storage",
        )),
    }
}
fn validate_size(host: &HostWgpuContext, size: FrameSize) -> Result<(), InteropError> {
    let limit = host.device.limits().max_texture_dimension_2d;
    if size.width == 0 || size.height == 0 || size.width > limit || size.height > limit {
        return Err(InteropError::InvalidFrame(
            "IOSurface extent exceeds device limits",
        ));
    }
    Ok(())
}

/// Producer-owned exportable allocation. Rendering into an ordinary texture
/// requires a GPU copy into `texture()` before publishing readiness.
pub struct SharedMetalSurface {
    surface: OwnedIoSurface,
    texture: wgpu::Texture,
}
impl SharedMetalSurface {
    pub fn new(
        host: &HostWgpuContext,
        size: FrameSize,
        pixel: PixelFormat,
    ) -> Result<Self, InteropError> {
        validate_size(host, size)?;
        let (_, _, fourcc) = format(pixel)?;
        let width = CFNumber::new_i64(size.width.into());
        let height = CFNumber::new_i64(size.height.into());
        let bpe = CFNumber::new_i32(4);
        let pixel_format = CFNumber::new_i64(fourcc.into());
        // SAFETY: framework constants and retained CFNumber values have the
        // documented IOSurface property types; the dictionary retains them.
        let properties = unsafe {
            CFDictionary::<CFType, CFType>::from_slices(
                &[
                    kIOSurfaceWidth.as_ref(),
                    kIOSurfaceHeight.as_ref(),
                    kIOSurfaceBytesPerElement.as_ref(),
                    kIOSurfacePixelFormat.as_ref(),
                ],
                &[
                    width.as_ref(),
                    height.as_ref(),
                    bpe.as_ref(),
                    pixel_format.as_ref(),
                ],
            )
        };
        let surface = unsafe { IOSurfaceRef::new(properties.as_opaque()) }
            .ok_or_else(|| InteropError::Metal("IOSurface allocation failed".into()))?;
        let surface = OwnedIoSurface::new(surface);
        let texture = wrap(&surface, size, pixel, host, None)?;
        Ok(Self { surface, texture })
    }
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
    pub fn export_right(&self) -> Result<MachSendRight, InteropError> {
        let port = self.surface.surface().create_mach_port();
        if port == 0 {
            return Err(InteropError::Metal("IOSurface Mach export failed".into()));
        }
        // SAFETY: IOSurfaceCreateMachPort returns one owned send right.
        Ok(unsafe { MachSendRight::from_raw(port) })
    }
}

/// Reconstitutes storage from a capability transferred by the host transport.
/// Lookup retains the IOSurface; dropping the right afterwards is safe.
pub fn lookup_surface(right: &MachSendRight) -> Result<OwnedIoSurface, InteropError> {
    IOSurfaceRef::lookup_from_mach_port(right.as_raw())
        .map(OwnedIoSurface::new)
        .ok_or(InteropError::InvalidFrame("invalid IOSurface Mach right"))
}

/// Tracks the last wgpu reference and the consumer's final submitted GPU use.
/// Dropping this token early abandons release notification, never grants reuse.
pub struct MetalRelease {
    device: wgpu::Device,
    queue: wgpu::Queue,
    texture_dropped: Arc<AtomicBool>,
    fence: Option<Arc<AtomicBool>>,
}
impl MetalRelease {
    /// Call after submitting the final consumer work on the importing queue.
    /// No more uses may be submitted afterwards. Texture clones must also drop.
    pub fn seal(&mut self) -> Result<(), InteropError> {
        if self.fence.is_some() {
            return Err(InteropError::InvalidFrame("consumer use already sealed"));
        }
        let done = Arc::new(AtomicBool::new(false));
        let callback = done.clone();
        self.queue
            .on_submitted_work_done(move || callback.store(true, Ordering::Release));
        self.fence = Some(done);
        Ok(())
    }
    /// Nonblocking. Device polling also retires wgpu's deferred texture drops.
    pub fn poll_released(&self) -> Result<bool, InteropError> {
        self.device
            .poll(wgpu::PollType::Poll)
            .map_err(InteropError::NormalizationCompletion)?;
        Ok(self
            .fence
            .as_ref()
            .is_some_and(|f| f.load(Ordering::Acquire))
            && self.texture_dropped.load(Ordering::Acquire))
    }
}

/// Wraps the same IOSurface storage on the consumer's device without CPU copy.
/// The returned Metal texture retains its IOSurface backing. The caller must
/// already have observed producer completion and must seal its final queue use.
pub fn import_surface(
    surface: OwnedIoSurface,
    size: FrameSize,
    pixel: PixelFormat,
    host: &HostWgpuContext,
) -> Result<(wgpu::Texture, MetalRelease), InteropError> {
    let dropped = Arc::new(AtomicBool::new(false));
    let callback = dropped.clone();
    let texture = wrap(
        &surface,
        size,
        pixel,
        host,
        Some(Box::new(move || callback.store(true, Ordering::Release))),
    )?;
    Ok((
        texture,
        MetalRelease {
            device: host.device.clone(),
            queue: host.queue.clone(),
            texture_dropped: dropped,
            fence: None,
        },
    ))
}

fn wrap(
    surface: &OwnedIoSurface,
    size: FrameSize,
    pixel: PixelFormat,
    host: &HostWgpuContext,
    drop_callback: Option<wgpu_hal::DropCallback>,
) -> Result<wgpu::Texture, InteropError> {
    validate_size(host, size)?;
    let (format, native_format, fourcc) = format(pixel)?;
    let io = surface.surface();
    if io.width() != size.width as usize
        || io.height() != size.height as usize
        || io.bytes_per_element() != 4
        || io.pixel_format() != fourcc
        || io.plane_count() != 0
    {
        return Err(InteropError::InvalidFrame(
            "IOSurface differs from declared extent, pixel layout or format",
        ));
    }
    // SAFETY: use the host's own Metal device and validate the backing above.
    // The descriptor and wgpu descriptor agree exactly. Metal retains IOSurface
    // storage for the lifetime of the native texture; HAL takes that retain.
    unsafe {
        let device =
            host.device
                .as_hal::<wgpu::wgc::api::Metal>()
                .ok_or(InteropError::BackendMismatch {
                    expected: "Metal",
                    actual: "non-Metal",
                })?;
        let descriptor = MTLTextureDescriptor::new();
        descriptor.setWidth(size.width as usize);
        descriptor.setHeight(size.height as usize);
        descriptor.setDepth(1);
        descriptor.setArrayLength(1);
        descriptor.setMipmapLevelCount(1);
        descriptor.setSampleCount(1);
        descriptor.setTextureType(MTLTextureType::Type2D);
        descriptor.setPixelFormat(native_format);
        descriptor.setUsage(MTLTextureUsage::ShaderRead);
        let native = device
            .raw_device()
            .newTextureWithDescriptor_iosurface_plane(&descriptor, io, 0)
            .ok_or_else(|| {
                InteropError::Metal("Metal IOSurface texture allocation failed".into())
            })?;
        let hal = wgpu::hal::metal::Device::texture_from_raw(
            native,
            format,
            MTLTextureType::Type2D,
            1,
            1,
            wgpu::hal::CopyExtent {
                width: size.width,
                height: size.height,
                depth: 1,
            },
            drop_callback,
        );
        Ok(crate::wgpu_compat::create_texture_from_hal::<
            wgpu_hal::api::Metal,
        >(
            &host.device,
            hal,
            &wgpu::TextureDescriptor {
                label: Some("graft-iosurface"),
                size: wgpu::Extent3d {
                    width: size.width,
                    height: size.height,
                    depth_or_array_layers: 1,
                },
                format,
                dimension: wgpu::TextureDimension::D2,
                mip_level_count: 1,
                sample_count: 1,
                usage: wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::TextureUses::RESOURCE,
        ))
    }
}
