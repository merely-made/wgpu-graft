//! Linux DMABUF to Vulkan/wgpu import.
//!
//! This module owns the low-level Vulkan work shared by browser producers:
//! explicit DRM-modifier image creation, external-memory import, memory-type
//! selection, shared-fd multi-plane handling, and foreign-queue acquisition.
//! Producer-specific modifier and synchronization policy stays in the caller.

use ash::vk;
use std::ffi::CStr;

use crate::{HostWgpuContext, InteropError, UnsupportedReason, VulkanExternalImage};

pub(crate) fn required_device_extensions() -> [&'static CStr; 4] {
    [
        ash::ext::external_memory_dma_buf::NAME,
        ash::ext::image_drm_format_modifier::NAME,
        ash::ext::queue_family_foreign::NAME,
        ash::khr::external_memory_fd::NAME,
    ]
}

pub(crate) fn base_dmabuf_device_extensions() -> [&'static CStr; 3] {
    [
        ash::ext::external_memory_dma_buf::NAME,
        ash::ext::image_drm_format_modifier::NAME,
        ash::khr::external_memory_fd::NAME,
    ]
}

/// One plane in an owned DMABUF import.
///
/// `fd` is consumed by [`import_dmabuf`]. Every descriptor is closed on all
/// error paths. On a successful Vulkan memory import the driver owns the first
/// fd; redundant same-buffer plane fds are closed by Graft.
#[derive(Clone, Copy, Debug)]
pub struct VulkanDmaBufPlane {
    pub fd: i32,
    pub offset: u64,
    pub stride: u64,
}

/// Queue-family and layout state at the Vulkan import boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VulkanDmaBufQueueOwnership {
    /// The image was produced outside this Vulkan device. Graft acquires it
    /// from `VK_QUEUE_FAMILY_FOREIGN_EXT` and leaves it shader-readable.
    Foreign,
    /// The image is locally created and still in `VK_IMAGE_LAYOUT_UNDEFINED`.
    /// This preserves the historical [`VulkanExternalImage`] contract.
    LocalUninitialized,
}

/// Complete low-level description of a DMABUF-backed image.
///
/// `drm_modifier` must be an explicit modifier. A producer that reports
/// `DRM_FORMAT_MOD_INVALID` must either negotiate one or deliberately apply a
/// format-specific fallback policy before calling this function.
#[derive(Debug)]
pub struct VulkanDmaBufImport {
    pub size: dpi::PhysicalSize<u32>,
    pub format: wgpu::TextureFormat,
    /// Linux DRM fourcc. Known 32-bit RGBA/BGRA values override `format` when
    /// selecting the Vulkan format. Set to `0` to map from `format` alone.
    pub drm_format: u32,
    pub drm_modifier: u64,
    pub planes: Vec<VulkanDmaBufPlane>,
    pub queue_ownership: VulkanDmaBufQueueOwnership,
}

struct PlaneFdGuard(Vec<i32>);

impl PlaneFdGuard {
    fn new(planes: &[VulkanDmaBufPlane]) -> Self {
        let mut fds = Vec::with_capacity(planes.len());
        for plane in planes {
            if !fds.contains(&plane.fd) {
                fds.push(plane.fd);
            }
        }
        Self(fds)
    }

    fn first(&self) -> i32 {
        self.0[0]
    }

    fn transfer_first_to_vulkan(&mut self) {
        self.0[0] = -1;
    }
}

impl Drop for PlaneFdGuard {
    fn drop(&mut self) {
        for fd in &mut self.0 {
            if *fd >= 0 {
                // SAFETY: import_dmabuf owns every fd recorded in this guard.
                unsafe { libc::close(*fd) };
                *fd = -1;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FdIdentity {
    device: libc::dev_t,
    inode: libc::ino_t,
}

fn fd_identity(fd: i32) -> Option<FdIdentity> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: fstat only reads metadata for the live descriptor.
    if unsafe { libc::fstat(fd, stat.as_mut_ptr()) } != 0 {
        return None;
    }
    let stat = unsafe { stat.assume_init() };
    Some(FdIdentity {
        device: stat.st_dev,
        inode: stat.st_ino,
    })
}

fn planes_share_kernel_object(planes: &[VulkanDmaBufPlane]) -> bool {
    if planes.len() <= 1 {
        return true;
    }
    let Some(first) = fd_identity(planes[0].fd) else {
        return false;
    };
    planes[1..]
        .iter()
        .all(|plane| fd_identity(plane.fd) == Some(first))
}

fn map_format(format: wgpu::TextureFormat) -> Result<vk::Format, InteropError> {
    match format {
        wgpu::TextureFormat::Rgba8Unorm => Ok(vk::Format::R8G8B8A8_UNORM),
        wgpu::TextureFormat::Rgba8UnormSrgb => Ok(vk::Format::R8G8B8A8_SRGB),
        wgpu::TextureFormat::Bgra8Unorm => Ok(vk::Format::B8G8R8A8_UNORM),
        wgpu::TextureFormat::Bgra8UnormSrgb => Ok(vk::Format::B8G8R8A8_SRGB),
        other => Err(InteropError::Vulkan(format!(
            "DMABUF import does not support wgpu format {other:?}"
        ))),
    }
}

fn map_drm_format(
    drm_format: u32,
    fallback: wgpu::TextureFormat,
) -> Result<vk::Format, InteropError> {
    // linux/drm_fourcc.h, little-endian byte order.
    const DRM_FORMAT_ARGB8888: u32 = 0x3432_5241; // AR24, BGRA bytes
    const DRM_FORMAT_ABGR8888: u32 = 0x3432_4241; // AB24, RGBA bytes
    const DRM_FORMAT_XRGB8888: u32 = 0x3432_5258; // XR24
    const DRM_FORMAT_XBGR8888: u32 = 0x3432_4258; // XB24

    match drm_format {
        DRM_FORMAT_ARGB8888 | DRM_FORMAT_XRGB8888 => Ok(vk::Format::B8G8R8A8_UNORM),
        DRM_FORMAT_ABGR8888 | DRM_FORMAT_XBGR8888 => Ok(vk::Format::R8G8B8A8_UNORM),
        0 => map_format(fallback),
        other => Err(InteropError::Vulkan(format!(
            "DMABUF import does not support DRM fourcc {other:#010x}"
        ))),
    }
}

/// Import an owned DMABUF as a texture on `host`'s Vulkan device.
///
/// Shared-fd multi-plane layouts are supported. Every plane must refer to the
/// same kernel DMABUF; disjoint per-plane allocations require Vulkan disjoint
/// binding and are rejected. Callers must serialize this function with other
/// submissions to the host queue because the foreign-ownership path records a
/// direct Vulkan submit through wgpu-hal.
pub fn import_dmabuf(
    frame: VulkanDmaBufImport,
    host: &HostWgpuContext,
) -> Result<wgpu::Texture, InteropError> {
    let mut plane_fds = PlaneFdGuard::new(&frame.planes);

    if host.backend != crate::InteropBackend::Vulkan {
        return Err(InteropError::BackendMismatch {
            expected: "Vulkan",
            actual: "non-Vulkan",
        });
    }
    if !host.dmabuf_support {
        return Err(InteropError::Unsupported(
            UnsupportedReason::VulkanDmabufExtensionNotEnabled,
        ));
    }
    if frame.queue_ownership == VulkanDmaBufQueueOwnership::Foreign
        && !host_has_foreign_queue_support(host)
    {
        return Err(InteropError::Unsupported(
            UnsupportedReason::VulkanForeignQueueExtensionNotEnabled,
        ));
    }
    if frame.size.width == 0 || frame.size.height == 0 {
        return Err(InteropError::InvalidFrame("DMABUF dimensions are zero"));
    }
    if frame.planes.is_empty() {
        return Err(InteropError::InvalidFrame("DMABUF has no planes"));
    }
    if frame.planes.iter().any(|plane| plane.fd < 0) {
        return Err(InteropError::InvalidFrame("DMABUF plane fd is negative"));
    }
    if !planes_share_kernel_object(&frame.planes) {
        return Err(InteropError::Unsupported(
            UnsupportedReason::NativeImportNotYetImplemented,
        ));
    }

    #[cfg(not(feature = "wgpu-30"))]
    if frame.queue_ownership == VulkanDmaBufQueueOwnership::Foreign {
        return Err(InteropError::Vulkan(
            "foreign-queue DMABUF import requires wgpu 30 so the established \
             RESOURCE state can be registered at the HAL boundary"
                .into(),
        ));
    }

    let vk_format = map_drm_format(frame.drm_format, frame.format)?;
    let width = frame.size.width;
    let height = frame.size.height;

    unsafe {
        let hal_device = host.device.as_hal::<wgpu::wgc::api::Vulkan>().ok_or(
            InteropError::BackendMismatch {
                expected: "Vulkan",
                actual: "non-Vulkan",
            },
        )?;
        let vk_device = hal_device.raw_device().clone();
        let vk_instance = hal_device.shared_instance().raw_instance().clone();
        let physical_device = hal_device.raw_physical_device();

        let plane_layouts: Vec<vk::SubresourceLayout> = frame
            .planes
            .iter()
            .map(|plane| vk::SubresourceLayout {
                offset: plane.offset,
                size: 0,
                row_pitch: plane.stride,
                array_pitch: 0,
                depth_pitch: 0,
            })
            .collect();
        let mut drm_modifier_info = vk::ImageDrmFormatModifierExplicitCreateInfoEXT::default()
            .drm_format_modifier(frame.drm_modifier)
            .plane_layouts(&plane_layouts);
        let mut external_memory_info = vk::ExternalMemoryImageCreateInfo::default()
            .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
        let image_create_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk_format)
            .extent(vk::Extent3D {
                width,
                height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
            .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_SRC)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .push_next(&mut external_memory_info)
            .push_next(&mut drm_modifier_info);

        let vulkan_image = vk_device
            .create_image(&image_create_info, None)
            .map_err(|err| InteropError::Vulkan(format!("vkCreateImage (DMABUF): {err}")))?;

        let memory = match allocate_and_bind_dmabuf_memory(
            &vk_device,
            &vk_instance,
            physical_device,
            vulkan_image,
            plane_fds.first(),
            &mut plane_fds,
        ) {
            Ok(memory) => memory,
            Err(err) => {
                vk_device.destroy_image(vulkan_image, None);
                return Err(err);
            }
        };

        let initial_state = match frame.queue_ownership {
            VulkanDmaBufQueueOwnership::Foreign => {
                if let Err(err) = acquire_from_foreign_queue(
                    &vk_device,
                    hal_device.raw_queue(),
                    hal_device.queue_family_index(),
                    vulkan_image,
                ) {
                    vk_device.free_memory(memory, None);
                    vk_device.destroy_image(vulkan_image, None);
                    return Err(err);
                }
                wgpu::TextureUses::RESOURCE
            }
            VulkanDmaBufQueueOwnership::LocalUninitialized => wgpu::TextureUses::UNINITIALIZED,
        };

        let hal_descriptor = wgpu_hal::TextureDescriptor {
            label: Some("grafting-dmabuf-import"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: frame.format,
            usage: wgpu::TextureUses::RESOURCE | wgpu::TextureUses::COPY_SRC,
            memory_flags: wgpu_hal::MemoryFlags::empty(),
            view_formats: Vec::new(),
        };
        let vk_device_for_drop = vk_device.clone();
        let hal_texture = hal_device.texture_from_raw(
            vulkan_image,
            &hal_descriptor,
            Some(Box::new(move || {
                vk_device_for_drop.destroy_image(vulkan_image, None);
                vk_device_for_drop.free_memory(memory, None);
            })),
            wgpu_hal::vulkan::TextureMemory::External,
        );

        Ok(crate::wgpu_compat::create_texture_from_hal::<
            wgpu_hal::api::Vulkan,
        >(
            &host.device,
            hal_texture,
            &wgpu::TextureDescriptor {
                label: Some("grafting-dmabuf-import"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                format: frame.format,
                dimension: wgpu::TextureDimension::D2,
                mip_level_count: 1,
                sample_count: 1,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            },
            initial_state,
        ))
    }
}

fn host_has_foreign_queue_support(host: &HostWgpuContext) -> bool {
    unsafe {
        host.device
            .as_hal::<wgpu::wgc::api::Vulkan>()
            .is_some_and(|device| {
                device
                    .enabled_device_extensions()
                    .contains(&ash::ext::queue_family_foreign::NAME)
            })
    }
}

unsafe fn allocate_and_bind_dmabuf_memory(
    vk_device: &ash::Device,
    vk_instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    vulkan_image: vk::Image,
    dmabuf_fd: i32,
    plane_fds: &mut PlaneFdGuard,
) -> Result<vk::DeviceMemory, InteropError> {
    let memory_requirements = unsafe { vk_device.get_image_memory_requirements(vulkan_image) };
    let external_memory_fd_api = ash::khr::external_memory_fd::Device::new(vk_instance, vk_device);
    let mut fd_properties = vk::MemoryFdPropertiesKHR::default();
    unsafe {
        external_memory_fd_api
            .get_memory_fd_properties(
                vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT,
                dmabuf_fd,
                &mut fd_properties,
            )
            .map_err(|err| InteropError::Vulkan(format!("vkGetMemoryFdPropertiesKHR: {err}")))?;
    }

    let allowed_memory_type_bits =
        memory_requirements.memory_type_bits & fd_properties.memory_type_bits;
    let memory_properties =
        unsafe { vk_instance.get_physical_device_memory_properties(physical_device) };
    let memory_type_index = memory_properties.memory_types
        [..memory_properties.memory_type_count as usize]
        .iter()
        .enumerate()
        .position(|(index, _)| allowed_memory_type_bits & (1 << index) != 0)
        .ok_or_else(|| {
            InteropError::Vulkan("image and imported DMABUF have no compatible memory type".into())
        })? as u32;

    let mut import_memory_info = vk::ImportMemoryFdInfoKHR::default()
        .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT)
        .fd(dmabuf_fd);
    let mut dedicated_allocate_info =
        vk::MemoryDedicatedAllocateInfo::default().image(vulkan_image);
    let memory = unsafe {
        vk_device
            .allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(memory_requirements.size)
                    .memory_type_index(memory_type_index)
                    .push_next(&mut import_memory_info)
                    .push_next(&mut dedicated_allocate_info),
                None,
            )
            .map_err(|err| InteropError::Vulkan(format!("vkAllocateMemory (DMABUF): {err}")))?
    };

    // A successful VkImportMemoryFdKHR allocation transfers ownership of fd.
    plane_fds.transfer_first_to_vulkan();
    if let Err(err) = unsafe { vk_device.bind_image_memory(vulkan_image, memory, 0) } {
        unsafe { vk_device.free_memory(memory, None) };
        return Err(InteropError::Vulkan(format!(
            "vkBindImageMemory (DMABUF): {err}"
        )));
    }
    Ok(memory)
}

unsafe fn acquire_from_foreign_queue(
    vk_device: &ash::Device,
    queue: vk::Queue,
    queue_family_index: u32,
    image: vk::Image,
) -> Result<(), InteropError> {
    let pool = unsafe {
        vk_device
            .create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(queue_family_index)
                    .flags(vk::CommandPoolCreateFlags::TRANSIENT),
                None,
            )
            .map_err(|err| {
                InteropError::Vulkan(format!("vkCreateCommandPool (DMABUF acquire): {err}"))
            })?
    };

    let result = unsafe {
        let command_buffer = vk_device
            .allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
            .map_err(|err| {
                InteropError::Vulkan(format!("vkAllocateCommandBuffers (DMABUF acquire): {err}"))
            })?[0];
        vk_device
            .begin_command_buffer(
                command_buffer,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )
            .map_err(|err| {
                InteropError::Vulkan(format!("vkBeginCommandBuffer (DMABUF acquire): {err}"))
            })?;

        let barrier = vk::ImageMemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::TRANSFER_READ)
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .src_queue_family_index(vk::QUEUE_FAMILY_FOREIGN_EXT)
            .dst_queue_family_index(queue_family_index)
            .image(image)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        vk_device.cmd_pipeline_barrier(
            command_buffer,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::ALL_COMMANDS,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier],
        );
        vk_device
            .end_command_buffer(command_buffer)
            .map_err(|err| {
                InteropError::Vulkan(format!("vkEndCommandBuffer (DMABUF acquire): {err}"))
            })?;

        let command_buffers = [command_buffer];
        let submit = vk::SubmitInfo::default().command_buffers(&command_buffers);
        vk_device
            .queue_submit(queue, &[submit], vk::Fence::null())
            .map_err(|err| {
                InteropError::Vulkan(format!("vkQueueSubmit (DMABUF acquire): {err}"))
            })?;
        vk_device
            .queue_wait_idle(queue)
            .map_err(|err| InteropError::Vulkan(format!("vkQueueWaitIdle (DMABUF acquire): {err}")))
    };

    unsafe { vk_device.destroy_command_pool(pool, None) };
    result
}

pub(crate) fn import_vulkan_external_image(
    frame: &VulkanExternalImage,
    host: &HostWgpuContext,
) -> Result<wgpu::Texture, InteropError> {
    if frame.dmabuf_fd <= 0 {
        return Err(InteropError::InvalidFrame("dmabuf_fd <= 0"));
    }
    import_dmabuf(
        VulkanDmaBufImport {
            size: frame.size,
            format: frame.format,
            drm_format: 0,
            drm_modifier: frame.drm_modifier,
            planes: vec![VulkanDmaBufPlane {
                fd: frame.dmabuf_fd,
                offset: frame.dmabuf_offset,
                stride: frame.dmabuf_stride,
            }],
            queue_ownership: VulkanDmaBufQueueOwnership::LocalUninitialized,
        },
        host,
    )
}

/// Construct a [`HostWgpuContext`] with the Vulkan extensions required for
/// [`import_dmabuf`].
pub fn create_dmabuf_host_context(
    adapter: &wgpu::Adapter,
    desc: &wgpu::DeviceDescriptor<'_>,
) -> Result<HostWgpuContext, InteropError> {
    use wgpu_hal::vulkan::CreateDeviceCallbackArgs;

    let hal_adapter = unsafe {
        adapter
            .as_hal::<wgpu::wgc::api::Vulkan>()
            .ok_or(InteropError::BackendMismatch {
                expected: "Vulkan",
                actual: "non-Vulkan",
            })?
    };
    let capabilities = hal_adapter.physical_device_capabilities();
    let missing: Vec<_> = required_device_extensions()
        .into_iter()
        .filter(|extension| !capabilities.supports_extension(extension))
        .map(|extension| extension.to_string_lossy().into_owned())
        .collect();
    if !missing.is_empty() {
        return Err(InteropError::Vulkan(format!(
            "physical device does not advertise required DMABUF extensions: {missing:?}"
        )));
    }

    let callback = Box::new(|args: CreateDeviceCallbackArgs<'_, '_, '_>| {
        for extension in required_device_extensions() {
            if !args.extensions.contains(&extension) {
                args.extensions.push(extension);
            }
        }
    });

    #[cfg(any(feature = "wgpu-29", feature = "wgpu-30"))]
    let open = unsafe {
        hal_adapter
            .open_with_callback(
                desc.required_features,
                &desc.required_limits,
                &desc.memory_hints,
                Some(callback),
            )
            .map_err(|err| InteropError::Vulkan(format!("open_with_callback: {err}")))?
    };
    #[cfg(not(any(feature = "wgpu-29", feature = "wgpu-30")))]
    let open = unsafe {
        hal_adapter
            .open_with_callback(desc.required_features, &desc.memory_hints, Some(callback))
            .map_err(|err| InteropError::Vulkan(format!("open_with_callback: {err}")))?
    };
    drop(hal_adapter);

    let (device, queue) = unsafe {
        adapter
            .create_device_from_hal::<wgpu::wgc::api::Vulkan>(open, desc)
            .map_err(|err| InteropError::Vulkan(format!("create_device_from_hal: {err}")))?
    };
    Ok(HostWgpuContext::new(device, queue))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plane(fd: i32) -> VulkanDmaBufPlane {
        VulkanDmaBufPlane {
            fd,
            offset: 0,
            stride: 0,
        }
    }

    fn open_pipe_read_fd() -> i32 {
        let mut fds = [0; 2];
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        unsafe { libc::close(fds[1]) };
        fds[0]
    }

    #[test]
    fn duplicated_plane_fds_are_one_kernel_object() {
        let first = open_pipe_read_fd();
        let second = unsafe { libc::dup(first) };
        assert!(second >= 0);
        assert!(planes_share_kernel_object(&[plane(first), plane(second)]));
        unsafe { libc::close(first) };
        unsafe { libc::close(second) };
    }

    #[test]
    fn repeated_descriptor_is_closed_once() {
        let fd = open_pipe_read_fd();
        let guard = PlaneFdGuard::new(&[plane(fd), plane(fd)]);
        assert_eq!(guard.0, vec![fd]);
        drop(guard);
    }

    #[test]
    fn independent_plane_fds_are_disjoint() {
        let first = open_pipe_read_fd();
        let second = open_pipe_read_fd();
        assert!(!planes_share_kernel_object(&[plane(first), plane(second)]));
        unsafe { libc::close(first) };
        unsafe { libc::close(second) };
    }

    #[test]
    fn invalid_plane_fd_is_conservatively_disjoint() {
        let first = open_pipe_read_fd();
        assert!(!planes_share_kernel_object(&[plane(first), plane(-1)]));
        unsafe { libc::close(first) };
    }
}
