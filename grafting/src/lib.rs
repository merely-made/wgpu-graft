// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

#![doc = include_str!("../README.md")]

// Alias the feature-selected wgpu / wgpu-hal pair back to the plain crate names
// so the rest of the crate keeps writing `wgpu::` and `wgpu_hal::` unchanged.
// The newest enabled version wins when several are on (e.g. `--all-features`).
//
// Public, so a consumer can write `grafting::wgpu::Texture` and be certain it
// names the same wgpu grafting was built against. Depending on `wgpu` directly
// alongside grafting risks resolving a different major, and an imported texture
// only works on a device from the matching one. Our own integration tests go
// through the re-export for the same reason.
#[cfg(feature = "wgpu-30")]
pub extern crate wgpu_30 as wgpu;
#[cfg(feature = "wgpu-30")]
pub extern crate wgpu_hal_30 as wgpu_hal;

#[cfg(all(feature = "wgpu-29", not(feature = "wgpu-30")))]
pub extern crate wgpu_29 as wgpu;
#[cfg(all(feature = "wgpu-29", not(feature = "wgpu-30")))]
pub extern crate wgpu_hal_29 as wgpu_hal;

#[cfg(all(feature = "wgpu-28", not(feature = "wgpu-29"), not(feature = "wgpu-30")))]
pub extern crate wgpu_28 as wgpu;
#[cfg(all(feature = "wgpu-28", not(feature = "wgpu-29"), not(feature = "wgpu-30")))]
pub extern crate wgpu_hal_28 as wgpu_hal;

#[cfg(not(any(feature = "wgpu-28", feature = "wgpu-29", feature = "wgpu-30")))]
compile_error!(
    "grafting needs one wgpu version feature: enable `wgpu-29` (default), `wgpu-30`, or `wgpu-28` to match your host's wgpu"
);

mod error;
mod wgpu_compat;
mod sync;

#[cfg(target_os = "windows")]
mod sync_dx12;
#[cfg(target_vendor = "apple")]
mod sync_metal;
#[cfg(target_os = "linux")]
mod sync_vulkan;

#[cfg(target_os = "linux")]
pub mod vulkan_dmabuf;

#[cfg(target_vendor = "apple")]
mod metal_texture_ref;

#[cfg(target_vendor = "apple")]
pub use metal_texture_ref::{import_metal_texture_borrowed, import_metal_texture_ref};

#[cfg(target_os = "windows")]
mod dx12_shared_texture;

/// Import a D3D12/D3D11 shared NT handle (described by [`Dx12SharedTexture`])
/// into a `wgpu::Texture` on the given [`HostWgpuContext`]'s DX12 device.
///
/// The returned texture aliases the shared resource (no copy). Use this with
/// [`crate::surfman_gl`]'s shared-handle export path when the consumer owns its
/// own wgpu device (e.g. a UI framework that exposes the device only on its
/// render thread). The safe frame carries shared RAII custody of the exported
/// handle; callers remain responsible for any required Y-flip.
#[cfg(target_os = "windows")]
pub use dx12_shared_texture::{import_dx12_shared_handle_borrowed, import_dx12_shared_texture};

#[cfg(all(
    feature = "gl",
    any(target_os = "linux", target_os = "android", target_os = "windows")
))]
mod gl_bindings {
    #![allow(unsafe_op_in_unsafe_fn)]

    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}

#[cfg(feature = "gl")]
pub mod raw_gl;

#[cfg(feature = "surfman")]
pub mod surfman_gl;

#[cfg(feature = "gl")]
use std::rc::Rc;
#[cfg(target_os = "windows")]
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[cfg(target_os = "windows")]
use std::os::windows::io::{AsRawHandle, OwnedHandle};

use dpi::PhysicalSize;
pub use error::{InteropError, UnsupportedReason};
pub use grafting_frame;
pub use grafting_frame::{
    FrameMetadata as NeutralFrameMetadata, FrameSize, FrameSync,
    OwnedNativeFrame as NeutralNativeFrame, OwnedNativeFrameKind, OwnedNativeImage, PixelFormat,
};
#[cfg(target_os = "linux")]
pub use grafting_frame::{DmaBufError, DmaBufImage, DmaBufPlane};
#[cfg(target_os = "windows")]
pub use grafting_frame::{
    Dx12SharedFence as OwnedDx12SharedFence,
    Dx12SharedResource as OwnedDx12SharedResource,
    Dx12SharedTexture as OwnedDx12SharedTexture,
};
#[cfg(target_vendor = "apple")]
pub use grafting_frame::{IoSurface as OwnedIoSurface, MetalTexture as OwnedMetalTexture};
pub use sync::{ImplicitOnlySynchronizer, InteropSynchronizer, NoopSynchronizer, SyncMechanism};

#[cfg(target_os = "windows")]
pub use sync_dx12::Dx12FenceSynchronizer;
#[cfg(target_vendor = "apple")]
pub use sync_metal::MetalSharedEventSynchronizer;
#[cfg(target_os = "linux")]
pub use sync_vulkan::VulkanSemaphoreSynchronizer;

/// The wgpu graphics backend in use on the host device.
///
/// Detected automatically by [`HostWgpuContext::new`] via `as_hal`. Used to
/// drive [`CapabilityMatrix::for_backend`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum InteropBackend {
    /// Vulkan backend (Linux, Android, Windows with `wgpu::Backends::VULKAN`).
    Vulkan,
    /// Metal backend (macOS, iOS).
    Metal,
    /// Direct3D 12 backend (Windows). GL→DX12 import is supported on
    /// ANGLE-backed surfman via the shared D3D11 NT-handle path
    /// (`surfman_gl::windows_dx12_shared`).
    Dx12,
    /// Backend could not be detected. All import paths will report
    /// [`CapabilityStatus::Unsupported`].
    Unknown,
}

/// Which corner of the texture holds row 0 of the image.
///
/// GL renders with the origin at the bottom-left; most compositors expect
/// top-left. The import paths in this crate Y-flip during blit so that all
/// returned textures have [`TextureOrigin::TopLeft`] when
/// [`ImportOptions::normalize_origin`] is `true` (the default).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextureOrigin {
    /// Row 0 is the top row. The standard convention for wgpu/Vulkan/Metal.
    TopLeft,
    /// Row 0 is the bottom row. Raw GL output before Y-flip normalization.
    BottomLeft,
}

/// Discriminant for [`NativeFrame`] variants, without carrying the frame data.
///
/// Returned by [`NativeFrame::kind`] and used in [`ProducerCapabilities`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum NativeFrameKind {
    /// A GL framebuffer that will be imported via the platform-specific path.
    #[cfg(feature = "gl")]
    GlFramebufferSource,
    /// A Vulkan external image (Linux DMABUF import).
    VulkanExternalImage,
    /// A Metal texture reference (macOS/iOS).
    MetalTextureRef,
    /// A D3D12 shared texture (Windows).
    Dx12SharedTexture,
}

/// Whether a particular interop capability is available on this device.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CapabilityStatus {
    /// The capability is available and `import_frame` should succeed.
    Supported,
    /// The capability is not available for the given reason.
    Unsupported(UnsupportedReason),
}

/// Reports which frame types can be imported on the current device and backend.
///
/// Obtain via [`HostWgpuContext::capabilities`] or
/// [`CapabilityMatrix::for_backend`]. Use this before attempting an import to
/// give the user an early, descriptive error rather than a runtime failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityMatrix {
    /// The backend detected on the host wgpu device.
    pub host_backend: InteropBackend,
    /// GL framebuffer import (the primary path — Linux Vulkan, Apple Metal).
    pub gl_framebuffer_source: CapabilityStatus,
    /// Direct DMABUF→Vulkan import (Linux only).
    pub vulkan_external_image: CapabilityStatus,
    /// Direct Metal texture reference import (Apple platforms only).
    pub metal_texture_ref: CapabilityStatus,
    /// D3D12 shared texture import (Windows only).
    pub dx12_shared_texture: CapabilityStatus,
}

/// The set of [`NativeFrameKind`]s a [`FrameProducer`] is able to emit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProducerCapabilities {
    /// Frame kinds this producer can supply.
    pub supported_frames: Vec<NativeFrameKind>,
}

/// Wraps a `wgpu::Device` and `wgpu::Queue` together with the detected backend.
///
/// Pass one of these to [`WgpuTextureImporter::new`] or directly to the
/// platform-specific import functions.
#[derive(Clone, Debug)]
pub struct HostWgpuContext {
    /// The wgpu device that will own imported textures.
    pub device: wgpu::Device,
    /// The queue associated with `device`.
    pub queue: wgpu::Queue,
    /// The graphics backend detected on `device` at construction time.
    pub backend: InteropBackend,
    /// Whether `device` was constructed with the Vulkan extensions required
    /// for the base DMABUF import path (`VK_EXT_image_drm_format_modifier`,
    /// `VK_EXT_external_memory_dma_buf`, and `VK_KHR_external_memory_fd`).
    /// Linux-only; always `false` on other platforms or non-Vulkan backends.
    ///
    /// Detected automatically by [`HostWgpuContext::new`] by inspecting the
    /// hal device's enabled extension list. On Linux, use
    /// `vulkan_dmabuf::create_dmabuf_host_context` to obtain a context where
    /// this is `true`.
    pub dmabuf_support: bool,
}

impl HostWgpuContext {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let backend = detect_backend(&device);
        let dmabuf_support = detect_dmabuf_support(&device, backend);
        Self {
            backend,
            dmabuf_support,
            device,
            queue,
        }
    }

    pub fn capabilities(&self) -> CapabilityMatrix {
        CapabilityMatrix::for_host(self.backend, self.dmabuf_support)
    }
}

/// Options that control how [`WgpuTextureImporter`] processes each frame.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct ImportOptions {
    /// If `true` (default), the importer runs a GPU blit/shader pass to
    /// flip the texture to [`TextureOrigin::TopLeft`]. Set to `false` only
    /// if you want the raw GL bottom-left orientation.
    pub normalize_origin: bool,
    /// If `true` (default), the importer converts BGRA output (Apple) to
    /// RGBA so that all returned textures have a consistent
    /// `Rgba8Unorm` format.
    pub normalize_format: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            normalize_origin: true,
            normalize_format: true,
        }
    }
}

/// A successfully imported wgpu texture, ready for use in a render pipeline.
///
/// Returned by [`TextureImporter::import_frame`].
#[derive(Debug)]
pub struct ImportedTexture {
    /// The imported wgpu texture. Bind this as a texture resource in your
    /// render pipeline.
    pub texture: wgpu::Texture,
    /// The pixel format of `texture`. `Rgba8Unorm` when
    /// [`ImportOptions::normalize_format`] is `true` (the default).
    pub format: wgpu::TextureFormat,
    /// Dimensions of `texture` in physical pixels.
    pub size: PhysicalSize<u32>,
    /// Whether row 0 of `texture` is the top or bottom of the image.
    /// [`TextureOrigin::TopLeft`] when [`ImportOptions::normalize_origin`]
    /// is `true` (the default).
    pub origin: TextureOrigin,
    /// Monotonically increasing counter that the producer increments each
    /// time new content is rendered. Use this to skip redundant re-imports.
    pub generation: u64,
    /// The synchronization mechanism the consumer should use after reading
    /// `texture`. Passed to [`InteropSynchronizer::consumer_ready`].
    pub consumer_sync: SyncMechanism,
}

#[cfg(feature = "gl")]
pub struct GlFramebufferSource {
    size: PhysicalSize<u32>,
    generation: u64,
    producer_sync: SyncMechanism,
    importer: Rc<dyn GlFramebufferSourceImpl>,
}

#[cfg(feature = "gl")]
impl GlFramebufferSource {
    pub fn size(&self) -> PhysicalSize<u32> {
        self.size
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn producer_sync(&self) -> SyncMechanism {
        self.producer_sync
    }

    pub fn new(
        size: PhysicalSize<u32>,
        generation: u64,
        producer_sync: SyncMechanism,
        importer: Rc<dyn GlFramebufferSourceImpl>,
    ) -> Self {
        Self {
            size,
            generation,
            producer_sync,
            importer,
        }
    }
}

/// Copyable metadata describing a producer frame.
///
/// This deliberately contains no OS or driver resource. Resource-bearing
/// frames own their handles privately so a safe caller cannot duplicate a
/// descriptor whose import transfers ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameMetadata {
    pub size: PhysicalSize<u32>,
    pub format: wgpu::TextureFormat,
    pub generation: u64,
    pub producer_sync: SyncMechanism,
}

/// A frame backed by a Linux DMABUF imported via Vulkan
/// `VK_KHR_external_memory_fd` + `VK_EXT_image_drm_format_modifier`.
///
/// The producer (e.g. WPE) hands the consumer a DMABUF fd, DRM format
/// modifier, and per-plane offset/stride; the importer wraps it as a
/// `wgpu::Texture` on the host's wgpu Vulkan device. Single-plane RGBA is
/// the common case; multi-plane formats are not yet supported.
///
/// The safe constructor takes ownership of both descriptors. Vulkan consumes
/// each descriptor only after its matching driver import succeeds; otherwise
/// Rust closes it as this frame is dropped. The raw constructor is `unsafe`
/// because it must be given uniquely owned file descriptors.
/// The Linux Vulkan semaphore synchronizer imports a duplicate of the
/// frame-owned semaphore descriptor, then drops the original with the
/// consumed frame.
pub struct VulkanExternalImage {
    metadata: FrameMetadata,
    #[cfg(target_os = "linux")]
    dmabuf_fd: std::os::fd::OwnedFd,
    #[cfg(target_os = "linux")]
    dmabuf_offset: u64,
    #[cfg(target_os = "linux")]
    dmabuf_stride: u64,
    #[cfg(target_os = "linux")]
    drm_modifier: u64,
    #[cfg(target_os = "linux")]
    wait_semaphore_fd: Option<std::os::fd::OwnedFd>,
}

impl VulkanExternalImage {
    /// Metadata that remains valid after the frame is consumed.
    pub fn metadata(&self) -> FrameMetadata {
        self.metadata
    }

    #[cfg(target_os = "linux")]
    pub fn new(
        metadata: FrameMetadata,
        dmabuf_fd: std::os::fd::OwnedFd,
        dmabuf_offset: u64,
        dmabuf_stride: u64,
        drm_modifier: u64,
        wait_semaphore_fd: Option<std::os::fd::OwnedFd>,
    ) -> Self {
        Self {
            metadata,
            dmabuf_fd,
            dmabuf_offset,
            dmabuf_stride,
            drm_modifier,
            wait_semaphore_fd,
        }
    }

    /// Construct an owned frame from raw Linux descriptors.
    ///
    /// # Safety
    ///
    /// Each descriptor must be valid and uniquely owned by the caller. This
    /// function assumes responsibility for closing it if the Vulkan driver
    /// does not successfully consume it.
    #[cfg(target_os = "linux")]
    pub unsafe fn from_raw_owned_parts(
        metadata: FrameMetadata,
        dmabuf_fd: std::os::fd::RawFd,
        dmabuf_offset: u64,
        dmabuf_stride: u64,
        drm_modifier: u64,
        wait_semaphore_fd: Option<std::os::fd::RawFd>,
    ) -> Self {
        use std::os::fd::FromRawFd;

        Self::new(
            metadata,
            unsafe { std::os::fd::OwnedFd::from_raw_fd(dmabuf_fd) },
            dmabuf_offset,
            dmabuf_stride,
            drm_modifier,
            wait_semaphore_fd.map(|fd| unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) }),
        )
    }
}

/// A frame backed by a retained `MTLTexture` from a Metal producer.
///
/// The safe constructor takes an Objective-C retain. Import moves that retain
/// into wgpu, so the producer must clone its retain when it intends to keep
/// rendering into the same texture. Raw borrowed pointers are available only
/// through the explicitly unsafe Metal import API.
pub struct MetalTextureRef {
    metadata: FrameMetadata,
    /// A retained `MTLTexture` represented as an opaque Objective-C pointer.
    /// It is private so the safe API cannot manufacture a borrowed pointer.
    #[cfg(target_vendor = "apple")]
    raw_metal_texture:
        objc2::rc::Retained<objc2::runtime::ProtocolObject<dyn objc2_metal::MTLTexture>>,
}

impl MetalTextureRef {
    pub fn metadata(&self) -> FrameMetadata {
        self.metadata
    }

    #[cfg(target_vendor = "apple")]
    pub fn new(
        metadata: FrameMetadata,
        raw_metal_texture: objc2::rc::Retained<
            objc2::runtime::ProtocolObject<dyn objc2_metal::MTLTexture>,
        >,
    ) -> Self {
        Self {
            metadata,
            raw_metal_texture,
        }
    }

    /// Construct a frame from a raw +1 Objective-C retain.
    ///
    /// # Safety
    ///
    /// `raw_metal_texture` must be a non-null `MTLTexture *` with a retain
    /// count owned by the caller. This frame assumes that retain.
    #[cfg(target_vendor = "apple")]
    pub unsafe fn from_raw_retained(
        metadata: FrameMetadata,
        raw_metal_texture: *mut std::ffi::c_void,
    ) -> Option<Self> {
        let raw_metal_texture = unsafe {
            objc2::rc::Retained::from_raw(
                raw_metal_texture
                    .cast::<objc2::runtime::ProtocolObject<dyn objc2_metal::MTLTexture>>(),
            )
        }?;
        Some(Self::new(metadata, raw_metal_texture))
    }
}

/// A frame backed by a D3D12 resource shared via a DXGI NT handle.
///
/// Obtain the handle by calling `IDXGIResource1::CreateSharedHandle` on your
/// `ID3D12Resource`. The importer opens its own D3D12 reference via
/// `ID3D12Device::OpenSharedHandle`. On Windows, construct a
/// `Dx12SharedResource` from an `OwnedHandle` so Rust closes the exported
/// handle after the producer and all in-flight frames release their shared
/// RAII custody.
///
/// A D3D12 producer must leave the resource in `D3D12_RESOURCE_STATE_COMMON`
/// before signalling `producer_sync`. That is the state this interop contract
/// imports into wgpu.
#[derive(Debug)]
pub struct Dx12SharedTexture {
    metadata: FrameMetadata,
    /// Fence value the producer signalled at on its `ID3D11Fence` /
    /// `ID3D12Fence` (opened from `Dx12FenceSynchronizer::shared_handle`;
    /// not linked, because that type only exists on Windows builds).
    /// The synchronizer waits for this value on the wgpu D3D12 queue
    /// before the next consumer submit.
    ///
    /// Only meaningful when `producer_sync == SyncMechanism::ExplicitFence`.
    /// Set to `0` for the keyed-mutex / no-fence path; the synchronizer
    /// treats `0` as "no wait recorded for this frame".
    fence_value: u64,
    /// Shared RAII custody of the reusable NT handle. The producer's cache and
    /// every emitted frame hold a clone. `OpenSharedHandle` creates the wgpu
    /// resource reference without consuming this handle.
    #[cfg(target_os = "windows")]
    resource: Dx12SharedResource,
}

#[cfg(target_os = "windows")]
#[derive(Clone, Debug)]
pub struct Dx12SharedResource {
    handle: Arc<OwnedHandle>,
    allocation_key: u64,
}

#[cfg(target_os = "windows")]
static NEXT_DX12_ALLOCATION_KEY: AtomicU64 = AtomicU64::new(1);

impl Dx12SharedTexture {
    pub fn metadata(&self) -> FrameMetadata {
        self.metadata
    }

    pub fn fence_value(&self) -> u64 {
        self.fence_value
    }

    #[cfg(target_os = "windows")]
    pub fn new(metadata: FrameMetadata, resource: Dx12SharedResource, fence_value: u64) -> Self {
        Self {
            metadata,
            fence_value,
            resource,
        }
    }

    /// Stable identity for one shared-resource allocation, suitable for a host
    /// cache. It does not change as the producer updates frame content.
    #[cfg(target_os = "windows")]
    pub fn allocation_key(&self) -> u64 {
        self.resource.allocation_key()
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn raw_handle(&self) -> *mut std::ffi::c_void {
        self.resource.raw_handle()
    }

    /// Reusable custody of this frame's shared resource. Cloning the resource
    /// does not clone a frame: consumers must build a fresh frame with explicit
    /// metadata before importing it.
    #[cfg(target_os = "windows")]
    pub fn resource(&self) -> Dx12SharedResource {
        self.resource.clone()
    }
}

#[cfg(target_os = "windows")]
impl Dx12SharedResource {
    /// Take RAII custody of a shared DXGI NT handle.
    pub fn from_owned_handle(handle: OwnedHandle) -> Self {
        Self {
            handle: Arc::new(handle),
            allocation_key: NEXT_DX12_ALLOCATION_KEY.fetch_add(1, Ordering::Relaxed),
        }
    }

    /// Stable identity for this shared-resource allocation, suitable for a
    /// host import cache. It does not change as the producer updates pixels.
    pub fn allocation_key(&self) -> u64 {
        self.allocation_key
    }

    /// Build a fresh one-shot frame from this reusable resource-custody
    /// token. The returned frame is deliberately move-only.
    pub fn frame(&self, metadata: FrameMetadata, fence_value: u64) -> Dx12SharedTexture {
        Dx12SharedTexture::new(metadata, self.clone(), fence_value)
    }

    fn raw_handle(&self) -> *mut std::ffi::c_void {
        self.handle.as_raw_handle()
    }
}

/// A frame produced by a [`FrameProducer`], ready to be imported by a
/// [`TextureImporter`].
///
/// All four variants have working import implementations.
/// `VulkanExternalImage` is Linux-only; the others are gated on their
/// respective platforms.
#[non_exhaustive]
pub enum NativeFrame {
    /// A GL framebuffer — the primary, fully-implemented path.
    #[cfg(feature = "gl")]
    GlFramebufferSource(GlFramebufferSource),
    /// A Linux DMABUF imported via Vulkan
    /// `VK_KHR_external_memory_fd` + `VK_EXT_image_drm_format_modifier`.
    VulkanExternalImage(VulkanExternalImage),
    /// A Metal texture reference. Implemented via IOSurface interop.
    MetalTextureRef(MetalTextureRef),
    /// A D3D12 shared texture. Implemented via shared NT handle interop.
    Dx12SharedTexture(Dx12SharedTexture),
}

impl NativeFrame {
    pub fn kind(&self) -> NativeFrameKind {
        match self {
            #[cfg(feature = "gl")]
            NativeFrame::GlFramebufferSource(_) => NativeFrameKind::GlFramebufferSource,
            NativeFrame::VulkanExternalImage(_) => NativeFrameKind::VulkanExternalImage,
            NativeFrame::MetalTextureRef(_) => NativeFrameKind::MetalTextureRef,
            NativeFrame::Dx12SharedTexture(_) => NativeFrameKind::Dx12SharedTexture,
        }
    }

    pub fn producer_sync(&self) -> SyncMechanism {
        match self {
            #[cfg(feature = "gl")]
            NativeFrame::GlFramebufferSource(frame) => frame.producer_sync(),
            NativeFrame::VulkanExternalImage(frame) => frame.metadata().producer_sync,
            NativeFrame::MetalTextureRef(frame) => frame.metadata().producer_sync,
            NativeFrame::Dx12SharedTexture(frame) => frame.metadata().producer_sync,
        }
    }
}

/// Produces [`NativeFrame`]s for a [`TextureImporter`] to consume.
///
/// Implement this for your GL/Vulkan/Metal renderer to feed frames into the
/// interop pipeline. See [`raw_gl::producer::RawGlFrameProducer`] for a
/// ready-made implementation that wraps any GL context.
pub trait FrameProducer {
    /// Returns what frame kinds this producer can emit.
    fn capabilities(&self) -> ProducerCapabilities;
    /// Acquire the next frame from the producer. The returned [`NativeFrame`]
    /// should be passed immediately to [`TextureImporter::import_frame`].
    fn acquire_frame(&mut self) -> Result<NativeFrame, InteropError>;
}

/// Imports a [`NativeFrame`] into a `wgpu::Texture`.
pub trait TextureImporter {
    /// Import `frame` into a [`wgpu::Texture`] owned by the host device.
    ///
    /// Returns [`InteropError::Unsupported`] if the frame kind is not
    /// supported on the current platform/backend. Check
    /// [`HostWgpuContext::capabilities`] first to get a descriptive error
    /// before calling this.
    fn import_frame(
        &self,
        frame: NativeFrame,
        options: &ImportOptions,
    ) -> Result<ImportedTexture, InteropError>;
}

/// The main entry point for importing frames into wgpu textures.
///
/// Create one per wgpu device and reuse it across frames.
///
/// ```ignore
/// let host = HostWgpuContext::new(device, queue);
/// let importer = WgpuTextureImporter::new(host);
/// // each frame:
/// let frame = producer.acquire_frame()?;
/// let imported = importer.import_frame(frame, &ImportOptions::default())?;
/// // use imported.texture in your render pipeline
/// ```
pub struct WgpuTextureImporter {
    host: HostWgpuContext,
    synchronizer: Box<dyn InteropSynchronizer>,
    /// Copies a bottom-left aliased import into a fresh, host-owned, top-left
    /// `Rgba8Unorm` texture (see [`import_frame`](Self::import_frame)). Only the
    /// GL framebuffer path produces bottom-left frames, so this is `gl`-only.
    #[cfg(feature = "gl")]
    normalizer: crate::raw_gl::texture_normalizer::ImportedTextureNormalizer,
}

impl WgpuTextureImporter {
    /// Create a new importer with the default [`ImplicitOnlySynchronizer`].
    pub fn new(host: HostWgpuContext) -> Self {
        Self::with_synchronizer(host, Box::new(ImplicitOnlySynchronizer))
    }

    /// Create a new importer with a custom [`InteropSynchronizer`].
    pub fn with_synchronizer(
        host: HostWgpuContext,
        synchronizer: Box<dyn InteropSynchronizer>,
    ) -> Self {
        #[cfg(feature = "gl")]
        let normalizer =
            crate::raw_gl::texture_normalizer::ImportedTextureNormalizer::new(&host.device);
        Self {
            host,
            synchronizer,
            #[cfg(feature = "gl")]
            normalizer,
        }
    }

    /// Returns the underlying [`HostWgpuContext`].
    pub fn host(&self) -> &HostWgpuContext {
        &self.host
    }
}

/// A frame classified by whether its shared resource changed.
///
/// Use [`NewResource`](Self::NewResource) when the producer reports a new
/// resource epoch and can supply the native frame handle. Use
/// [`ReusedResource`](Self::ReusedResource) for later frames that overwrite the
/// same shared allocation in place.
pub enum EpochFrame {
    NewResource {
        resource_epoch: u64,
        frame: NativeFrame,
    },
    ReusedResource {
        resource_epoch: u64,
    },
}

struct CachedEpochTexture {
    resource_epoch: u64,
    imported: ImportedTexture,
}

/// Import cache keyed by producer resource epoch.
///
/// This wrapper imports a native frame only when the producer changes the
/// shared allocation. For reused resources it keeps sampling the cached
/// `wgpu::Texture` and submits a tiny texture-to-buffer copy each frame so the
/// host queue observes in-place producer writes.
pub struct EpochCachedImporter {
    importer: WgpuTextureImporter,
    cached: Option<CachedEpochTexture>,
    cache_flush_buffer: Option<wgpu::Buffer>,
}

impl EpochCachedImporter {
    pub fn new(host: HostWgpuContext) -> Self {
        Self::from_importer(WgpuTextureImporter::new(host))
    }

    pub fn with_synchronizer(
        host: HostWgpuContext,
        synchronizer: Box<dyn InteropSynchronizer>,
    ) -> Self {
        Self::from_importer(WgpuTextureImporter::with_synchronizer(host, synchronizer))
    }

    pub fn from_importer(importer: WgpuTextureImporter) -> Self {
        Self {
            importer,
            cached: None,
            cache_flush_buffer: None,
        }
    }

    pub fn host(&self) -> &HostWgpuContext {
        self.importer.host()
    }

    pub fn imported(&self) -> Option<&ImportedTexture> {
        self.cached.as_ref().map(|cached| &cached.imported)
    }

    pub fn resource_epoch(&self) -> Option<u64> {
        self.cached.as_ref().map(|cached| cached.resource_epoch)
    }

    pub fn update(
        &mut self,
        frame: EpochFrame,
        options: &ImportOptions,
    ) -> Result<&ImportedTexture, InteropError> {
        match frame {
            EpochFrame::NewResource {
                resource_epoch,
                frame,
            } => {
                if self.resource_epoch() != Some(resource_epoch) {
                    let imported = self.importer.import_frame(frame, options)?;
                    self.cached = Some(CachedEpochTexture {
                        resource_epoch,
                        imported,
                    });
                }
            }
            EpochFrame::ReusedResource { resource_epoch } => {
                if self.resource_epoch() != Some(resource_epoch) {
                    return Err(InteropError::InvalidFrame(
                        "reused resource epoch has no matching cached import",
                    ));
                }
            }
        }

        self.flush_cached_texture()?;
        self.imported().ok_or(InteropError::InvalidFrame(
            "epoch cache update produced no texture",
        ))
    }

    fn flush_cached_texture(&mut self) -> Result<(), InteropError> {
        let Self {
            importer,
            cached,
            cache_flush_buffer,
        } = self;
        let Some(cached) = cached.as_ref() else {
            return Ok(());
        };

        if cached.imported.size.width == 0 || cached.imported.size.height == 0 {
            return Ok(());
        }

        let device = &importer.host.device;
        let queue = &importer.host.queue;
        let flush_buffer = cache_flush_buffer.get_or_insert_with(|| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("grafting epoch-cache flush"),
                size: wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as u64,
                usage: wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("grafting epoch-cache flush"),
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &cached.imported.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: flush_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }
}

#[cfg(target_os = "windows")]
/// Close a raw Windows NT shared handle.
///
/// This is a raw-compatibility helper for integrations that still manage a
/// borrowed `HANDLE` themselves. New code should pass an `OwnedHandle` to
/// [`Dx12SharedResource::from_owned_handle`] and let the safe frame path keep
/// custody.
///
/// # Safety
///
/// `handle` must be a Windows `HANDLE` value uniquely owned by the caller. It
/// must not be used after this function returns `Ok(())`.
#[deprecated(note = "use Dx12SharedResource::from_owned_handle and the move-only safe frame path")]
pub unsafe fn close_shared_handle(handle: *mut std::ffi::c_void) -> Result<(), InteropError> {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};

    let handle = HANDLE(handle);
    if handle.is_invalid() {
        return Ok(());
    }
    unsafe { CloseHandle(handle) }
        .map(|_| ())
        .map_err(|err| InteropError::Dx12(format!("CloseHandle failed: {err}")))
}

#[cfg(not(target_os = "windows"))]
/// No-op shared-handle closer for non-Windows builds.
///
/// # Safety
///
/// This function exists so cross-platform hosts can call one helper at the API
/// boundary. It does not dereference or close `handle`.
#[deprecated(note = "use Dx12SharedResource::from_owned_handle and the move-only safe frame path")]
pub unsafe fn close_shared_handle(_handle: *mut std::ffi::c_void) -> Result<(), InteropError> {
    Ok(())
}

impl TextureImporter for WgpuTextureImporter {
    fn import_frame(
        &self,
        frame: NativeFrame,
        options: &ImportOptions,
    ) -> Result<ImportedTexture, InteropError> {
        self.synchronizer
            .producer_complete(&frame, frame.producer_sync())?;

        // `options` drives the GL bottom-left normalize pass only; the
        // shared-texture paths return top-left already.
        #[cfg(not(feature = "gl"))]
        let _ = options;

        let imported = match frame {
            #[cfg(feature = "gl")]
            NativeFrame::GlFramebufferSource(frame_source) => {
                frame_source
                    .importer
                    .import_into(&frame_source, &self.host, options)
            }
            NativeFrame::VulkanExternalImage(frame) => {
                import_vulkan_external_image(frame, &self.host)
            }
            NativeFrame::MetalTextureRef(frame) => import_metal_frame(frame, &self.host),
            NativeFrame::Dx12SharedTexture(frame) => import_dx12_shared_frame(frame, &self.host),
        }?;

        // The GL-producer paths return a texture that ALIASES the producer's
        // live surface (bottom-left origin, no copy). When normalization is
        // requested (the default), blit it into a fresh, host-owned top-left
        // `Rgba8Unorm` texture. This both corrects orientation and decouples the
        // consumer from the producer's in-place rendering — sampling the live
        // alias across frames otherwise races with the producer and flickers.
        // Paths that already return a normalized top-left texture (Metal) are
        // left untouched.
        #[cfg(feature = "gl")]
        let imported = if options.normalize_origin && imported.origin == TextureOrigin::BottomLeft {
            let texture = self.normalizer.normalize(
                &self.host.device,
                &self.host.queue,
                &imported.texture,
                imported.size,
            );
            ImportedTexture {
                texture,
                format: wgpu::TextureFormat::Rgba8Unorm,
                origin: TextureOrigin::TopLeft,
                ..imported
            }
        } else {
            imported
        };

        self.synchronizer
            .consumer_ready(&imported, imported.consumer_sync)?;
        Ok(imported)
    }
}

impl CapabilityMatrix {
    /// Reports the capability shape assuming a default-constructed wgpu device.
    ///
    /// On Linux + Vulkan, `vulkan_external_image` is reported as
    /// [`UnsupportedReason::VulkanDmabufExtensionNotEnabled`] because the
    /// default `wgpu::Device` does not enable `VK_EXT_image_drm_format_modifier`.
    /// Use [`Self::for_host`] (or [`HostWgpuContext::capabilities`]) for an
    /// accurate matrix that reflects the actual device.
    pub fn for_backend(host_backend: InteropBackend) -> Self {
        Self::for_host(host_backend, false)
    }

    /// Reports the capability shape for a specific host configuration.
    ///
    /// `dmabuf_support` should be `true` when the wgpu device has the Vulkan
    /// extensions needed for the DMABUF import path enabled — set
    /// automatically by [`HostWgpuContext::new`] via runtime detection.
    pub fn for_host(host_backend: InteropBackend, dmabuf_support: bool) -> Self {
        let gl_framebuffer_source = match host_backend {
            InteropBackend::Vulkan | InteropBackend::Metal | InteropBackend::Dx12 => {
                CapabilityStatus::Supported
            }
            InteropBackend::Unknown => {
                CapabilityStatus::Unsupported(UnsupportedReason::HostBackendUnavailable)
            }
        };

        let vulkan_external_image = match host_backend {
            InteropBackend::Vulkan => {
                #[cfg(target_os = "linux")]
                {
                    if dmabuf_support {
                        CapabilityStatus::Supported
                    } else {
                        CapabilityStatus::Unsupported(
                            UnsupportedReason::VulkanDmabufExtensionNotEnabled,
                        )
                    }
                }
                #[cfg(not(target_os = "linux"))]
                {
                    let _ = dmabuf_support;
                    // The wgpu Vulkan backend works on Linux/Android/Windows,
                    // but DMABUF imports are Linux-specific.
                    CapabilityStatus::Unsupported(UnsupportedReason::PlatformNotImplemented)
                }
            }
            InteropBackend::Metal | InteropBackend::Dx12 => {
                CapabilityStatus::Unsupported(UnsupportedReason::HostBackendMismatch)
            }
            InteropBackend::Unknown => {
                CapabilityStatus::Unsupported(UnsupportedReason::HostBackendUnavailable)
            }
        };

        let metal_texture_ref = match host_backend {
            InteropBackend::Metal => CapabilityStatus::Supported,
            InteropBackend::Vulkan | InteropBackend::Dx12 => {
                CapabilityStatus::Unsupported(UnsupportedReason::HostBackendMismatch)
            }
            InteropBackend::Unknown => {
                CapabilityStatus::Unsupported(UnsupportedReason::HostBackendUnavailable)
            }
        };

        let dx12_shared_texture = match host_backend {
            InteropBackend::Dx12 => CapabilityStatus::Supported,
            InteropBackend::Vulkan | InteropBackend::Metal => {
                CapabilityStatus::Unsupported(UnsupportedReason::HostBackendMismatch)
            }
            InteropBackend::Unknown => {
                CapabilityStatus::Unsupported(UnsupportedReason::HostBackendUnavailable)
            }
        };

        Self {
            host_backend,
            gl_framebuffer_source,
            vulkan_external_image,
            metal_texture_ref,
            dx12_shared_texture,
        }
    }
}

#[cfg(feature = "gl")]
pub trait GlFramebufferSourceImpl {
    fn import_into(
        &self,
        frame: &GlFramebufferSource,
        host: &HostWgpuContext,
        options: &ImportOptions,
    ) -> Result<ImportedTexture, InteropError>;
}

fn import_vulkan_external_image(
    #[cfg_attr(not(target_os = "linux"), allow(unused_variables))] frame: VulkanExternalImage,
    #[cfg_attr(not(target_os = "linux"), allow(unused_variables))] host: &HostWgpuContext,
) -> Result<ImportedTexture, InteropError> {
    #[cfg(target_os = "linux")]
    {
        let metadata = frame.metadata();
        let texture = vulkan_dmabuf::import_vulkan_external_image(frame, host)?;
        return Ok(ImportedTexture {
            texture,
            format: metadata.format,
            size: metadata.size,
            origin: TextureOrigin::TopLeft,
            generation: metadata.generation,
            consumer_sync: metadata.producer_sync,
        });
    }

    #[cfg(not(target_os = "linux"))]
    Err(InteropError::Unsupported(
        UnsupportedReason::NativeImportNotYetImplemented,
    ))
}

fn import_metal_frame(
    #[cfg_attr(not(target_vendor = "apple"), allow(unused_variables))] frame: MetalTextureRef,
    #[cfg_attr(not(target_vendor = "apple"), allow(unused_variables))] host: &HostWgpuContext,
) -> Result<ImportedTexture, InteropError> {
    #[cfg(target_vendor = "apple")]
    {
        let metadata = frame.metadata();
        let texture = metal_texture_ref::import_metal_texture_ref(frame, host)?;
        return Ok(ImportedTexture {
            texture,
            format: metadata.format,
            size: metadata.size,
            origin: TextureOrigin::TopLeft,
            generation: metadata.generation,
            consumer_sync: metadata.producer_sync,
        });
    }

    #[cfg(not(target_vendor = "apple"))]
    Err(InteropError::Unsupported(
        UnsupportedReason::HostBackendMismatch,
    ))
}

fn import_dx12_shared_frame(
    #[cfg_attr(not(target_os = "windows"), allow(unused_variables))] frame: Dx12SharedTexture,
    #[cfg_attr(not(target_os = "windows"), allow(unused_variables))] host: &HostWgpuContext,
) -> Result<ImportedTexture, InteropError> {
    #[cfg(target_os = "windows")]
    {
        let metadata = frame.metadata();
        let texture = dx12_shared_texture::import_dx12_shared_texture(frame, host)?;
        return Ok(ImportedTexture {
            texture,
            format: metadata.format,
            size: metadata.size,
            origin: TextureOrigin::TopLeft,
            generation: metadata.generation,
            consumer_sync: metadata.producer_sync,
        });
    }

    #[cfg(not(target_os = "windows"))]
    Err(InteropError::Unsupported(
        UnsupportedReason::HostBackendMismatch,
    ))
}

/// Returns a human-readable name of the wgpu backend powering `device`.
///
/// Useful for `eprintln!`/log lines on startup so the active graphics API is
/// visible without rebuilding with `RUST_LOG=wgpu=debug`. Returns `"Unknown"`
/// when the backend can't be detected (e.g. no `as_hal` impl matches).
pub fn backend_name(device: &wgpu::Device) -> &'static str {
    match detect_backend(device) {
        InteropBackend::Vulkan => "Vulkan",
        InteropBackend::Metal => "Metal",
        InteropBackend::Dx12 => "DirectX 12",
        InteropBackend::Unknown => "Unknown",
    }
}

/// Logs the active wgpu backend to stderr.
///
/// Equivalent to `eprintln!("[wgpu] backend: {}", backend_name(device))`.
/// Prefer [`backend_name`] when you want to route the value to a logger.
pub fn print_wgpu_backend(device: &wgpu::Device) {
    eprintln!("[wgpu] backend: {}", backend_name(device));
}

fn detect_backend(device: &wgpu::Device) -> InteropBackend {
    unsafe {
        // wgpu::wgc::api::Vulkan is only compiled in when the hal `vulkan` cfg
        // is set — i.e. Linux, Android, and Windows (not macOS).
        #[cfg(any(target_os = "linux", target_os = "android", target_os = "windows"))]
        if device.as_hal::<wgpu::wgc::api::Vulkan>().is_some() {
            return InteropBackend::Vulkan;
        }

        #[cfg(target_vendor = "apple")]
        if device.as_hal::<wgpu::wgc::api::Metal>().is_some() {
            return InteropBackend::Metal;
        }

        #[cfg(target_os = "windows")]
        if device.as_hal::<wgpu::wgc::api::Dx12>().is_some() {
            return InteropBackend::Dx12;
        }
    }

    InteropBackend::Unknown
}

/// Whether `device` has the base Vulkan device extensions needed to create and
/// bind a DMABUF-backed image. Foreign ownership is checked separately when
/// an imported frame requests it.
///
/// Returns `false` on non-Linux or non-Vulkan backends.
fn detect_dmabuf_support(
    #[cfg_attr(not(target_os = "linux"), allow(unused_variables))] device: &wgpu::Device,
    backend: InteropBackend,
) -> bool {
    #[cfg(target_os = "linux")]
    {
        if backend != InteropBackend::Vulkan {
            return false;
        }
        unsafe {
            let Some(hal_device) = device.as_hal::<wgpu::wgc::api::Vulkan>() else {
                return false;
            };
            let enabled = hal_device.enabled_device_extensions();
            crate::vulkan_dmabuf::base_dmabuf_device_extensions()
                .iter()
                .all(|extension| enabled.contains(extension))
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = backend;
        false
    }
}

#[cfg(test)]
mod ownership_shape_tests {
    use super::*;

    static_assertions::assert_not_impl_any!(NativeFrame: Clone, Copy);
    static_assertions::assert_not_impl_any!(VulkanExternalImage: Clone, Copy);
    static_assertions::assert_not_impl_any!(MetalTextureRef: Clone, Copy);
    static_assertions::assert_not_impl_any!(Dx12SharedTexture: Clone, Copy);

    #[cfg(target_os = "windows")]
    static_assertions::assert_impl_all!(Dx12SharedResource: Clone);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_options_default_prefers_normalized_textures() {
        let options = ImportOptions::default();

        assert!(options.normalize_origin);
        assert!(options.normalize_format);
    }

    #[test]
    fn capability_matrix_tracks_backend_shape() {
        let vulkan = CapabilityMatrix::for_backend(InteropBackend::Vulkan);
        let metal = CapabilityMatrix::for_backend(InteropBackend::Metal);
        let dx12 = CapabilityMatrix::for_backend(InteropBackend::Dx12);
        let unknown = CapabilityMatrix::for_backend(InteropBackend::Unknown);

        assert_eq!(vulkan.gl_framebuffer_source, CapabilityStatus::Supported);
        assert_eq!(metal.gl_framebuffer_source, CapabilityStatus::Supported);
        assert_eq!(dx12.gl_framebuffer_source, CapabilityStatus::Supported);
        assert_eq!(
            unknown.gl_framebuffer_source,
            CapabilityStatus::Unsupported(UnsupportedReason::HostBackendUnavailable)
        );

        // `for_backend` reports the default-device shape: on Linux + Vulkan
        // the DMABUF extensions are not enabled, so the import path is
        // reported as unsupported. `for_host(_, true)` flips it to Supported.
        #[cfg(target_os = "linux")]
        {
            assert_eq!(
                vulkan.vulkan_external_image,
                CapabilityStatus::Unsupported(UnsupportedReason::VulkanDmabufExtensionNotEnabled)
            );
            let vulkan_with_dmabuf = CapabilityMatrix::for_host(InteropBackend::Vulkan, true);
            assert_eq!(
                vulkan_with_dmabuf.vulkan_external_image,
                CapabilityStatus::Supported
            );
        }
        #[cfg(not(target_os = "linux"))]
        assert_eq!(
            vulkan.vulkan_external_image,
            CapabilityStatus::Unsupported(UnsupportedReason::PlatformNotImplemented)
        );
        assert_eq!(
            metal.vulkan_external_image,
            CapabilityStatus::Unsupported(UnsupportedReason::HostBackendMismatch)
        );

        assert_eq!(metal.metal_texture_ref, CapabilityStatus::Supported);
        assert_eq!(
            vulkan.metal_texture_ref,
            CapabilityStatus::Unsupported(UnsupportedReason::HostBackendMismatch)
        );

        assert_eq!(dx12.dx12_shared_texture, CapabilityStatus::Supported);
        assert_eq!(
            vulkan.dx12_shared_texture,
            CapabilityStatus::Unsupported(UnsupportedReason::HostBackendMismatch)
        );
    }

    #[test]
    fn implicit_synchronizer_accepts_implicit_flush() {
        assert!(ImplicitOnlySynchronizer::validate(SyncMechanism::ImplicitGlFlush).is_ok());
    }

    #[test]
    fn implicit_synchronizer_rejects_explicit_sync() {
        assert!(matches!(
            ImplicitOnlySynchronizer::validate(SyncMechanism::ExplicitExternalSemaphore),
            Err(InteropError::UnsupportedSynchronization(
                SyncMechanism::ExplicitExternalSemaphore
            ))
        ));
    }
}
