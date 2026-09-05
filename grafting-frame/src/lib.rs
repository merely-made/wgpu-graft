// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Move-owned native image and synchronization custody.
//!
//! This crate is the producer/host boundary beneath `grafting`. It deliberately
//! has no dependency on wgpu, wgpu-hal, Vulkan bindings, or an engine contract.
//! Import policy and GPU API transitions belong in an importer above this crate.

use std::fmt;

/// Pixel dimensions of one producer frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameSize {
    pub width: u32,
    pub height: u32,
}

impl FrameSize {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// Four-channel, eight-bit pixel layouts supported by the native-frame seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PixelFormat {
    Rgba8Unorm,
    Rgba8UnormSrgb,
    Bgra8Unorm,
    Bgra8UnormSrgb,
}

/// Resource-independent facts that remain valid after an image is imported.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameMetadata {
    pub size: FrameSize,
    pub format: PixelFormat,
    pub generation: u64,
}

/// Stable discriminator for host routing before a platform downcast.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum OwnedNativeFrameKind {
    Dx12SharedTexture,
    DmaBufImage,
    MetalTexture,
    IoSurface,
}

/// One move-owned producer frame.
///
/// Dropping a frame releases every image and synchronization resource still in
/// Rust custody. Importers take this value by value when an operating-system or
/// driver import consumes any contained descriptor.
pub struct OwnedNativeFrame {
    metadata: FrameMetadata,
    image: OwnedNativeImage,
    sync: FrameSync,
}

impl OwnedNativeFrame {
    pub fn new(metadata: FrameMetadata, image: OwnedNativeImage, sync: FrameSync) -> Self {
        Self {
            metadata,
            image,
            sync,
        }
    }

    pub fn metadata(&self) -> FrameMetadata {
        self.metadata
    }
    pub fn kind(&self) -> OwnedNativeFrameKind {
        self.image.kind()
    }
    pub fn image(&self) -> &OwnedNativeImage {
        &self.image
    }
    pub fn sync(&self) -> &FrameSync {
        &self.sync
    }
    pub fn into_parts(self) -> (FrameMetadata, OwnedNativeImage, FrameSync) {
        (self.metadata, self.image, self.sync)
    }
}

impl fmt::Debug for OwnedNativeFrame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OwnedNativeFrame")
            .field("metadata", &self.metadata)
            .field("kind", &self.kind())
            .field("sync", &self.sync)
            .finish_non_exhaustive()
    }
}

/// Platform image custody. Variants exist only where their native ownership
/// type exists.
#[non_exhaustive]
pub enum OwnedNativeImage {
    #[cfg(target_os = "windows")]
    Dx12SharedTexture(Dx12SharedTexture),
    #[cfg(target_os = "linux")]
    DmaBufImage(DmaBufImage),
    #[cfg(target_vendor = "apple")]
    MetalTexture(MetalTexture),
    #[cfg(target_vendor = "apple")]
    IoSurface(IoSurface),
}

impl OwnedNativeImage {
    pub fn kind(&self) -> OwnedNativeFrameKind {
        match self {
            #[cfg(target_os = "windows")]
            Self::Dx12SharedTexture(_) => OwnedNativeFrameKind::Dx12SharedTexture,
            #[cfg(target_os = "linux")]
            Self::DmaBufImage(_) => OwnedNativeFrameKind::DmaBufImage,
            #[cfg(target_vendor = "apple")]
            Self::MetalTexture(_) => OwnedNativeFrameKind::MetalTexture,
            #[cfg(target_vendor = "apple")]
            Self::IoSurface(_) => OwnedNativeFrameKind::IoSurface,
        }
    }
}

/// Synchronization custody paired with an image.
///
/// `None` means the producer and consumer have another documented ordering
/// relationship, such as a shared queue. GL flush policy remains in `grafting`
/// because it is not an owned native synchronization resource.
#[derive(Debug)]
#[non_exhaustive]
pub enum FrameSync {
    None,
    #[cfg(target_os = "windows")]
    Dx12Fence {
        fence: Dx12SharedFence,
        value: u64,
    },
    #[cfg(target_os = "linux")]
    VulkanSemaphore(std::os::fd::OwnedFd),
    #[cfg(target_vendor = "apple")]
    MetalSharedEvent {
        event: objc2::rc::Retained<objc2::runtime::ProtocolObject<dyn objc2_metal::MTLSharedEvent>>,
        value: u64,
    },
}

#[cfg(target_os = "linux")]
mod linux {
    use std::os::fd::{FromRawFd, OwnedFd, RawFd};

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct DmaBufPlane {
        pub buffer_index: u32,
        pub offset: u64,
        pub stride: u64,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[non_exhaustive]
    pub enum DmaBufError {
        NoBuffers,
        NoPlanes,
        InvalidBufferIndex { plane: usize, buffer_index: u32 },
        ZeroStride { plane: usize },
    }

    impl std::fmt::Display for DmaBufError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::NoBuffers => formatter.write_str("DMABUF image has no owned buffers"),
                Self::NoPlanes => formatter.write_str("DMABUF image has no planes"),
                Self::InvalidBufferIndex {
                    plane,
                    buffer_index,
                } => write!(
                    formatter,
                    "DMABUF plane {plane} references absent buffer {buffer_index}"
                ),
                Self::ZeroStride { plane } => {
                    write!(formatter, "DMABUF plane {plane} has a zero stride")
                }
            }
        }
    }

    impl std::error::Error for DmaBufError {}

    /// Move-owned DMABUF descriptor table and DRM plane layout. Several planes
    /// may reference one buffer entry.
    pub struct DmaBufImage {
        buffers: Vec<OwnedFd>,
        planes: Vec<DmaBufPlane>,
        drm_format: u32,
        drm_modifier: u64,
    }

    impl DmaBufImage {
        pub fn new(
            buffers: Vec<OwnedFd>,
            planes: Vec<DmaBufPlane>,
            drm_format: u32,
            drm_modifier: u64,
        ) -> Result<Self, DmaBufError> {
            if buffers.is_empty() {
                return Err(DmaBufError::NoBuffers);
            }
            if planes.is_empty() {
                return Err(DmaBufError::NoPlanes);
            }
            for (plane_index, plane) in planes.iter().enumerate() {
                if plane.buffer_index as usize >= buffers.len() {
                    return Err(DmaBufError::InvalidBufferIndex {
                        plane: plane_index,
                        buffer_index: plane.buffer_index,
                    });
                }
                if plane.stride == 0 {
                    return Err(DmaBufError::ZeroStride { plane: plane_index });
                }
            }
            Ok(Self {
                buffers,
                planes,
                drm_format,
                drm_modifier,
            })
        }

        /// # Safety
        /// Every entry in `buffers` must be a valid, open descriptor uniquely
        /// owned by the caller. This takes responsibility for closing all of them.
        pub unsafe fn from_raw_owned_parts(
            buffers: Vec<RawFd>,
            planes: Vec<DmaBufPlane>,
            drm_format: u32,
            drm_modifier: u64,
        ) -> Result<Self, DmaBufError> {
            let buffers = buffers
                .into_iter()
                .map(|fd| unsafe { OwnedFd::from_raw_fd(fd) })
                .collect();
            Self::new(buffers, planes, drm_format, drm_modifier)
        }

        pub fn buffers(&self) -> &[OwnedFd] {
            &self.buffers
        }
        pub fn planes(&self) -> &[DmaBufPlane] {
            &self.planes
        }
        pub fn drm_format(&self) -> u32 {
            self.drm_format
        }
        pub fn drm_modifier(&self) -> u64 {
            self.drm_modifier
        }
        pub fn into_parts(self) -> (Vec<OwnedFd>, Vec<DmaBufPlane>, u32, u64) {
            (
                self.buffers,
                self.planes,
                self.drm_format,
                self.drm_modifier,
            )
        }
        pub fn into_buffers(self) -> Vec<OwnedFd> {
            self.buffers
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux::{DmaBufError, DmaBufImage, DmaBufPlane};

#[cfg(target_os = "windows")]
mod windows_frame {
    use std::{
        os::windows::io::{AsHandle, BorrowedHandle, OwnedHandle},
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
    };

    static NEXT_ALLOCATION_KEY: AtomicU64 = AtomicU64::new(1);
    static NEXT_FENCE_KEY: AtomicU64 = AtomicU64::new(1);

    #[derive(Clone, Debug)]
    pub struct Dx12SharedResource {
        handle: Arc<OwnedHandle>,
        allocation_key: u64,
    }

    impl Dx12SharedResource {
        pub fn from_owned_handle(handle: OwnedHandle) -> Self {
            Self {
                handle: Arc::new(handle),
                allocation_key: NEXT_ALLOCATION_KEY.fetch_add(1, Ordering::Relaxed),
            }
        }
        pub fn allocation_key(&self) -> u64 {
            self.allocation_key
        }
        pub fn as_handle(&self) -> BorrowedHandle<'_> {
            self.handle.as_handle()
        }
    }

    #[derive(Debug)]
    pub struct Dx12SharedTexture {
        resource: Dx12SharedResource,
    }

    impl Dx12SharedTexture {
        pub fn new(resource: Dx12SharedResource) -> Self {
            Self { resource }
        }
        pub fn allocation_key(&self) -> u64 {
            self.resource.allocation_key()
        }
        pub fn resource(&self) -> &Dx12SharedResource {
            &self.resource
        }
        pub fn into_resource(self) -> Dx12SharedResource {
            self.resource
        }
    }

    #[derive(Clone, Debug)]
    pub struct Dx12SharedFence {
        handle: Arc<OwnedHandle>,
        fence_key: u64,
    }

    impl Dx12SharedFence {
        pub fn from_owned_handle(handle: OwnedHandle) -> Self {
            Self {
                handle: Arc::new(handle),
                fence_key: NEXT_FENCE_KEY.fetch_add(1, Ordering::Relaxed),
            }
        }
        pub fn fence_key(&self) -> u64 {
            self.fence_key
        }
        pub fn as_handle(&self) -> BorrowedHandle<'_> {
            self.handle.as_handle()
        }
    }
}

#[cfg(target_os = "windows")]
pub use windows_frame::{Dx12SharedFence, Dx12SharedResource, Dx12SharedTexture};

#[cfg(target_vendor = "apple")]
mod apple {
    use objc2::{rc::Retained, runtime::ProtocolObject};
    use objc2_core_foundation::CFRetained;
    use objc2_io_surface::IOSurfaceRef;
    use objc2_metal::MTLTexture;

    pub struct MetalTexture {
        texture: Retained<ProtocolObject<dyn MTLTexture>>,
    }
    impl MetalTexture {
        pub fn new(texture: Retained<ProtocolObject<dyn MTLTexture>>) -> Self {
            Self { texture }
        }
        pub fn texture(&self) -> &ProtocolObject<dyn MTLTexture> {
            &self.texture
        }
        pub fn into_retained(self) -> Retained<ProtocolObject<dyn MTLTexture>> {
            self.texture
        }
    }

    pub struct IoSurface {
        surface: CFRetained<IOSurfaceRef>,
    }
    impl IoSurface {
        pub fn new(surface: CFRetained<IOSurfaceRef>) -> Self {
            Self { surface }
        }
        pub fn surface(&self) -> &IOSurfaceRef {
            &self.surface
        }
        pub fn into_retained(self) -> CFRetained<IOSurfaceRef> {
            self.surface
        }
    }
}

#[cfg(target_vendor = "apple")]
pub use apple::{IoSurface, MetalTexture};

#[cfg(test)]
mod tests {
    use super::*;
    use static_assertions::assert_not_impl_any;

    assert_not_impl_any!(OwnedNativeFrame: Copy, Clone);
    assert_not_impl_any!(OwnedNativeImage: Copy, Clone);
    assert_not_impl_any!(FrameSync: Copy, Clone);

    #[test]
    fn metadata_is_resource_free_and_copyable() {
        let metadata = FrameMetadata {
            size: FrameSize::new(960, 640),
            format: PixelFormat::Bgra8Unorm,
            generation: 42,
        };
        assert_eq!(metadata, metadata);
        assert_eq!(metadata.size.width, 960);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn dx12_custody_clones_one_allocation_identity() {
        use std::os::windows::io::{FromRawHandle, OwnedHandle};

        let raw = unsafe {
            windows::Win32::System::Threading::CreateEventW(None, true, false, None)
                .expect("create test handle")
        };
        let owned = unsafe { OwnedHandle::from_raw_handle(raw.0) };
        let resource = Dx12SharedResource::from_owned_handle(owned);
        let clone = resource.clone();
        assert_eq!(resource.allocation_key(), clone.allocation_key());

        let texture = Dx12SharedTexture::new(clone);
        assert_eq!(texture.allocation_key(), resource.allocation_key());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn invalid_dmabuf_layout_closes_taken_descriptor() {
        use std::{io::Write, os::fd::IntoRawFd, os::unix::net::UnixStream};

        let (taken, mut peer) = UnixStream::pair().expect("create descriptor pair");
        let raw = taken.into_raw_fd();
        let result = unsafe {
            DmaBufImage::from_raw_owned_parts(
                vec![raw],
                vec![DmaBufPlane {
                    buffer_index: 1,
                    offset: 0,
                    stride: 4,
                }],
                0,
                0,
            )
        };
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("invalid table must fail"),
        };
        assert!(matches!(error, DmaBufError::InvalidBufferIndex { .. }));
        assert!(peer.write_all(b"closed").is_err());
    }
}
