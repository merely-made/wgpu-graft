// SPDX-License-Identifier: MPL-2.0
//! Exportable Vulkan DMA-BUF storage and final-use tracking (Linux, wgpu 30).
//!
//! A host transports owned descriptors, waits for `poll_ready`, and returns a
//! release acknowledgement only after `VulkanRelease::poll_released`. Both
//! endpoints use GENERAL layout and FOREIGN queue ownership at the boundary.
//! CPU messages order completed GPU submissions; they do not replace barriers.
use super::vulkan_dmabuf::{VulkanDmaBufImport, VulkanDmaBufPlane, VulkanDmaBufQueueOwnership};
use crate::{HostWgpuContext, InteropError, wgpu};
use ash::vk;
use std::{
    os::fd::{FromRawFd, OwnedFd},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub(crate) struct Storage {
    _device_guard: wgpu::Device,
    device: ash::Device,
    pub(crate) image: vk::Image,
    memory: vk::DeviceMemory,
}
impl Storage {
    pub(crate) fn new(
        host: &HostWgpuContext,
        device: ash::Device,
        image: vk::Image,
        memory: vk::DeviceMemory,
    ) -> Arc<Self> {
        Arc::new(Self {
            _device_guard: host.device.clone(),
            device,
            image,
            memory,
        })
    }
}
impl Drop for Storage {
    fn drop(&mut self) {
        // Each in-flight command retains storage through its queue callback.
        unsafe {
            self.device.destroy_image(self.image, None);
            self.device.free_memory(self.memory, None);
        }
    }
}
pub(crate) struct ImportTracking {
    pub(crate) dropped: Arc<AtomicBool>,
    pub(crate) storage: Option<Arc<Storage>>,
}
fn error(context: &str, error: vk::Result) -> InteropError {
    InteropError::Vulkan(format!("{context}: {error}"))
}
fn range() -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .level_count(1)
        .layer_count(1)
}
fn usage() -> vk::ImageUsageFlags {
    vk::ImageUsageFlags::SAMPLED
        | vk::ImageUsageFlags::TRANSFER_SRC
        | vk::ImageUsageFlags::TRANSFER_DST
}
fn transition(host: &HostWgpuContext, texture: &wgpu::Texture, state: wgpu::TextureUses) {
    let mut encoder = host.device.create_command_encoder(&Default::default());
    encoder.transition_resources(
        std::iter::empty(),
        std::iter::once(wgpu::TextureTransition {
            texture,
            selector: None,
            state,
        }),
    );
    host.queue.submit([encoder.finish()]);
}
fn raw_submit(
    host: &HostWgpuContext,
    storage: Arc<Storage>,
    record: impl FnOnce(&ash::Device, vk::CommandBuffer, u32),
) -> Result<Arc<AtomicBool>, InteropError> {
    let mut encoder = host.device.create_command_encoder(&Default::default());
    unsafe {
        let hal = host
            .device
            .as_hal::<wgpu::wgc::api::Vulkan>()
            .ok_or(InteropError::InvalidFrame("Vulkan device required"))?;
        encoder.as_hal_mut::<wgpu::wgc::api::Vulkan, _, _>(|encoder| {
            let encoder = encoder.ok_or(InteropError::InvalidFrame("Vulkan encoder required"))?;
            record(
                hal.raw_device(),
                encoder.raw_handle(),
                hal.queue_family_index(),
            );
            Ok::<_, InteropError>(())
        })?;
    }
    let done = Arc::new(AtomicBool::new(false));
    let callback = done.clone();
    host.queue.submit([encoder.finish()]);
    host.queue.on_submitted_work_done(move || {
        drop(storage);
        callback.store(true, Ordering::Release);
    });
    Ok(done)
}
fn poll(host: &HostWgpuContext, flag: &Arc<AtomicBool>) -> Result<bool, InteropError> {
    host.device
        .poll(wgpu::PollType::Poll)
        .map_err(InteropError::NormalizationCompletion)?;
    Ok(flag.load(Ordering::Acquire))
}

/// Single-plane linear RGBA8 storage. Allocation fails explicitly if the
/// physical adapter cannot export AND import the negotiated DRM modifier.
/// The backing may be reused only after an actual consumer release witness.
pub struct SharedVulkanSurface {
    host: HostWgpuContext,
    storage: Arc<Storage>,
    size: crate::FrameSize,
    stride: u64,
    offset: u64,
    in_flight: bool,
    initialized: bool,
    ready: Option<Arc<AtomicBool>>,
}
impl SharedVulkanSurface {
    pub fn new(host: &HostWgpuContext, size: crate::FrameSize) -> Result<Self, InteropError> {
        if size.width == 0
            || size.height == 0
            || size.width > host.device.limits().max_texture_dimension_2d
            || size.height > host.device.limits().max_texture_dimension_2d
        {
            return Err(InteropError::InvalidFrame("invalid Vulkan surface extent"));
        }
        unsafe {
            let hal = host
                .device
                .as_hal::<wgpu::wgc::api::Vulkan>()
                .ok_or(InteropError::InvalidFrame("Vulkan device required"))?;
            let device = hal.raw_device();
            let instance = hal.shared_instance().raw_instance();
            let physical = hal.raw_physical_device();
            for extension in super::vulkan_dmabuf::required_device_extensions() {
                if !hal.enabled_device_extensions().contains(&extension) {
                    return Err(InteropError::InvalidFrame(
                        "DMA-BUF device extensions required",
                    ));
                }
            }
            // A fixed explicit LINEAR modifier keeps the wire contract narrow.
            // Query exact format/usage/handle support instead of guessing it.
            let mut modifier = vk::PhysicalDeviceImageDrmFormatModifierInfoEXT::default()
                .drm_format_modifier(0)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);
            let mut external = vk::PhysicalDeviceExternalImageFormatInfo::default()
                .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
            let info = vk::PhysicalDeviceImageFormatInfo2::default()
                .format(vk::Format::R8G8B8A8_UNORM)
                .ty(vk::ImageType::TYPE_2D)
                .tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
                .usage(usage())
                .push_next(&mut modifier)
                .push_next(&mut external);
            let mut external_properties = vk::ExternalImageFormatProperties::default();
            let mut properties =
                vk::ImageFormatProperties2::default().push_next(&mut external_properties);
            instance
                .get_physical_device_image_format_properties2(physical, &info, &mut properties)
                .map_err(|e| error("linear RGBA8 DMA-BUF capability", e))?;
            let features = external_properties
                .external_memory_properties
                .external_memory_features;
            if !features.contains(
                vk::ExternalMemoryFeatureFlags::EXPORTABLE
                    | vk::ExternalMemoryFeatureFlags::IMPORTABLE,
            ) {
                return Err(InteropError::InvalidFrame(
                    "linear RGBA8 DMA-BUF is not exportable and importable",
                ));
            }
            let modifiers = [0u64];
            let mut modifier_info = vk::ImageDrmFormatModifierListCreateInfoEXT::default()
                .drm_format_modifiers(&modifiers);
            let mut external_info = vk::ExternalMemoryImageCreateInfo::default()
                .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
            let image_info = vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(vk::Format::R8G8B8A8_UNORM)
                .extent(vk::Extent3D {
                    width: size.width,
                    height: size.height,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
                .usage(usage())
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .push_next(&mut modifier_info)
                .push_next(&mut external_info);
            let image = device
                .create_image(&image_info, None)
                .map_err(|e| error("export image", e))?;
            let requirements = device.get_image_memory_requirements(image);
            let memory_properties = instance.get_physical_device_memory_properties(physical);
            let memory_type = (0..memory_properties.memory_type_count)
                .find(|i| requirements.memory_type_bits & (1 << i) != 0);
            let Some(memory_type) = memory_type else {
                device.destroy_image(image, None);
                return Err(InteropError::InvalidFrame("no export memory type"));
            };
            let mut export = vk::ExportMemoryAllocateInfo::default()
                .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
            let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
            let allocation = vk::MemoryAllocateInfo::default()
                .allocation_size(requirements.size)
                .memory_type_index(memory_type)
                .push_next(&mut export)
                .push_next(&mut dedicated);
            let memory = match device.allocate_memory(&allocation, None) {
                Ok(memory) => memory,
                Err(e) => {
                    device.destroy_image(image, None);
                    return Err(error("export allocation", e));
                },
            };
            if let Err(e) = device.bind_image_memory(image, memory, 0) {
                device.destroy_image(image, None);
                device.free_memory(memory, None);
                return Err(error("export bind", e));
            }
            let storage = Storage::new(host, device.clone(), image, memory);
            let layout = device.get_image_subresource_layout(
                image,
                vk::ImageSubresource::default()
                    .aspect_mask(vk::ImageAspectFlags::MEMORY_PLANE_0_EXT),
            );
            if layout.row_pitch < u64::from(size.width) * 4 {
                return Err(InteropError::InvalidFrame("invalid exported row pitch"));
            }
            Ok(Self {
                host: host.clone(),
                storage,
                size,
                stride: layout.row_pitch,
                offset: layout.offset,
                in_flight: false,
                initialized: false,
                ready: None,
            })
        }
    }
    pub fn size(&self) -> crate::FrameSize {
        self.size
    }
    pub fn stride(&self) -> u64 {
        self.stride
    }
    pub fn offset(&self) -> u64 {
        self.offset
    }
    /// Copy an ordinary local RGBA8 texture into export storage, then release
    /// that storage to FOREIGN/GENERAL. No wait occurs here.
    ///
    /// # Safety
    /// `source` must belong to this surface's host device. All GPU use of it
    /// must be submitted through that host queue.
    pub unsafe fn copy_from(&mut self, source: &wgpu::Texture) -> Result<(), InteropError> {
        if self.in_flight {
            return Err(InteropError::InvalidFrame("export surface is still leased"));
        }
        if source.format() != wgpu::TextureFormat::Rgba8Unorm
            || source.width() != self.size.width
            || source.height() != self.size.height
            || source.depth_or_array_layers() != 1
            || source.sample_count() != 1
            || !source.usage().contains(wgpu::TextureUsages::COPY_SRC)
        {
            return Err(InteropError::InvalidFrame(
                "export source shape/format/usage mismatch",
            ));
        }
        transition(&self.host, source, wgpu::TextureUses::COPY_SRC);
        let source_image = unsafe {
            source
                .as_hal::<wgpu::wgc::api::Vulkan>()
                .ok_or(InteropError::InvalidFrame("Vulkan source required"))?
                .raw_handle()
        };
        let destination = self.storage.image;
        let initialized = self.initialized;
        let size = self.size;
        let done = raw_submit(
            &self.host,
            self.storage.clone(),
            |device, command, family| unsafe {
                // Keep ownership transfers layout-neutral. Both endpoints
                // match GENERAL->GENERAL; local transitions happen separately.
                if initialized {
                    let acquire = vk::ImageMemoryBarrier::default()
                        .old_layout(vk::ImageLayout::GENERAL)
                        .new_layout(vk::ImageLayout::GENERAL)
                        .src_queue_family_index(vk::QUEUE_FAMILY_FOREIGN_EXT)
                        .dst_queue_family_index(family)
                        .dst_access_mask(vk::AccessFlags::MEMORY_WRITE)
                        .image(destination)
                        .subresource_range(range());
                    device.cmd_pipeline_barrier(
                        command,
                        vk::PipelineStageFlags::TOP_OF_PIPE,
                        vk::PipelineStageFlags::ALL_COMMANDS,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[acquire],
                    );
                }
                let prepare = vk::ImageMemoryBarrier::default()
                    .old_layout(if initialized {
                        vk::ImageLayout::GENERAL
                    } else {
                        vk::ImageLayout::UNDEFINED
                    })
                    .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .image(destination)
                    .subresource_range(range());
                device.cmd_pipeline_barrier(
                    command,
                    vk::PipelineStageFlags::ALL_COMMANDS,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[prepare],
                );
                let layers = vk::ImageSubresourceLayers::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .layer_count(1);
                device.cmd_copy_image(
                    command,
                    source_image,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    destination,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[vk::ImageCopy::default()
                        .src_subresource(layers)
                        .dst_subresource(layers)
                        .extent(vk::Extent3D {
                            width: size.width,
                            height: size.height,
                            depth: 1,
                        })],
                );
                let prepare_release = vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .new_layout(vk::ImageLayout::GENERAL)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE)
                    .image(destination)
                    .subresource_range(range());
                device.cmd_pipeline_barrier(
                    command,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::ALL_COMMANDS,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[prepare_release],
                );
                let release = vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::GENERAL)
                    .new_layout(vk::ImageLayout::GENERAL)
                    .src_queue_family_index(family)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_FOREIGN_EXT)
                    .src_access_mask(vk::AccessFlags::MEMORY_WRITE)
                    .image(destination)
                    .subresource_range(range());
                device.cmd_pipeline_barrier(
                    command,
                    vk::PipelineStageFlags::ALL_COMMANDS,
                    vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[release],
                );
            },
        )?;
        let keep_alive = source.clone();
        self.host
            .queue
            .on_submitted_work_done(move || drop(keep_alive));
        self.ready = Some(done);
        self.in_flight = true;
        self.initialized = true;
        Ok(())
    }
    pub fn poll_ready(&self) -> Result<bool, InteropError> {
        match &self.ready {
            Some(done) => poll(&self.host, done),
            None => Ok(false),
        }
    }
    /// Export a fresh owned descriptor only after GPU release has completed.
    pub fn export_fd(&self) -> Result<OwnedFd, InteropError> {
        if !self.in_flight || !self.poll_ready()? {
            return Err(InteropError::InvalidFrame("export surface is not ready"));
        }
        unsafe {
            let hal = self
                .host
                .device
                .as_hal::<wgpu::wgc::api::Vulkan>()
                .ok_or(InteropError::InvalidFrame("Vulkan device required"))?;
            let api = ash::khr::external_memory_fd::Device::new(
                hal.shared_instance().raw_instance(),
                hal.raw_device(),
            );
            let fd = api
                .get_memory_fd(
                    &vk::MemoryGetFdInfoKHR::default()
                        .memory(self.storage.memory)
                        .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT),
                )
                .map_err(|e| error("export memory fd", e))?;
            Ok(OwnedFd::from_raw_fd(fd))
        }
    }
    /// Permit reuse after the host validates the current lease acknowledgement.
    ///
    /// # Safety
    /// Every importer must have completed its FOREIGN/GENERAL release and
    /// destroyed every texture/view reference before this is called. Peer death
    /// or an unvalidated/stale acknowledgement is not such a witness.
    pub unsafe fn acknowledge_release(&mut self) -> Result<(), InteropError> {
        if !self.in_flight || !self.poll_ready()? {
            return Err(InteropError::InvalidFrame("surface has no ready lease"));
        }
        self.in_flight = false;
        self.ready = None;
        Ok(())
    }
}

/// Consumer final-use tracker. Retained views block FOREIGN release even
/// after the queue is idle. Release discards the consumed pixels only after
/// every alias is destroyed; the producer always overwrites the full image.
pub struct VulkanRelease {
    host: HostWgpuContext,
    storage: Arc<Storage>,
    dropped: Arc<AtomicBool>,
    sealed: Option<Arc<AtomicBool>>,
    released: Option<Arc<AtomicBool>>,
}
impl VulkanRelease {
    pub fn seal(&mut self) -> Result<(), InteropError> {
        if self.sealed.is_some() {
            return Err(InteropError::InvalidFrame("Vulkan use already sealed"));
        }
        let flag = Arc::new(AtomicBool::new(false));
        let callback = flag.clone();
        self.host
            .queue
            .on_submitted_work_done(move || callback.store(true, Ordering::Release));
        self.sealed = Some(flag);
        Ok(())
    }
    /// Nonblocking. Once final references are gone, schedule a real ownership
    /// release and acknowledge only after that submission completes.
    pub fn poll_released(&mut self) -> Result<bool, InteropError> {
        self.host
            .device
            .poll(wgpu::PollType::Poll)
            .map_err(InteropError::NormalizationCompletion)?;
        if let Some(done) = &self.released {
            return Ok(done.load(Ordering::Acquire));
        }
        if !self.dropped.load(Ordering::Acquire)
            || !self
                .sealed
                .as_ref()
                .is_some_and(|done| done.load(Ordering::Acquire))
        {
            return Ok(false);
        }
        let image = self.storage.image;
        self.released = Some(raw_submit(
            &self.host,
            self.storage.clone(),
            |device, command, family| unsafe {
                // Every consumer alias is gone. Discard consumed pixels
                // instead of assuming wgpu's final tracked image layout.
                // The producer always overwrites the whole image on reuse.
                let discard = vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::GENERAL)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .src_access_mask(vk::AccessFlags::MEMORY_READ)
                    .dst_access_mask(vk::AccessFlags::MEMORY_WRITE)
                    .image(image)
                    .subresource_range(range());
                device.cmd_pipeline_barrier(
                    command,
                    vk::PipelineStageFlags::ALL_COMMANDS,
                    vk::PipelineStageFlags::ALL_COMMANDS,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[discard],
                );
                let release = vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::GENERAL)
                    .new_layout(vk::ImageLayout::GENERAL)
                    .src_queue_family_index(family)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_FOREIGN_EXT)
                    .src_access_mask(vk::AccessFlags::MEMORY_WRITE)
                    .image(image)
                    .subresource_range(range());
                device.cmd_pipeline_barrier(
                    command,
                    vk::PipelineStageFlags::ALL_COMMANDS,
                    vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[release],
                );
            },
        )?);
        Ok(false)
    }
}
/// Import an owned linear RGBA8 DMA-BUF from a completed Graft Vulkan export.
/// The host must transport actual descriptors and a validated completion witness.
pub fn import_surface(
    fd: OwnedFd,
    size: crate::FrameSize,
    offset: u64,
    stride: u64,
    host: &HostWgpuContext,
) -> Result<(wgpu::Texture, VulkanRelease), InteropError> {
    if size.width == 0 || size.height == 0 || stride < u64::from(size.width) * 4 {
        return Err(InteropError::InvalidFrame("invalid Vulkan surface layout"));
    }
    let mut tracking = ImportTracking {
        dropped: Arc::new(AtomicBool::new(false)),
        storage: None,
    };
    let frame = VulkanDmaBufImport::new(
        dpi::PhysicalSize::new(size.width, size.height),
        wgpu::TextureFormat::Rgba8Unorm,
        u32::from_le_bytes(*b"AB24"),
        0,
        vec![fd],
        vec![VulkanDmaBufPlane {
            buffer_index: 0,
            offset,
            stride,
        }],
        VulkanDmaBufQueueOwnership::Foreign,
    )?;
    let texture = super::vulkan_dmabuf::import_tracked(frame, host, &mut tracking)?;
    let storage = tracking
        .storage
        .ok_or(InteropError::InvalidFrame("missing import storage"))?;
    Ok((
        texture,
        VulkanRelease {
            host: host.clone(),
            storage,
            dropped: tracking.dropped,
            sealed: None,
            released: None,
        },
    ))
}
