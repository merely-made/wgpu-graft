// SPDX-License-Identifier: MPL-2.0
//! Same-adapter shared DX12 allocations with paired GPU fence values.
//!
//! An odd value publishes producer work; the following even value completes
//! consumer work. Reuse additionally requires the host's exact lease release,
//! issued only after the last imported texture/view has been destroyed.
use crate::{FrameSize, HostWgpuContext, InteropError, wgpu};
use std::os::windows::io::{AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle};
use windows::Win32::{
    Foundation::{GENERIC_ALL, HANDLE},
    Graphics::{Direct3D12::*, Dxgi::Common::*},
};
mod lifetime;
pub use lifetime::Dx12Release;

type Result<T> = std::result::Result<T, InteropError>;
fn dx(error: windows::core::Error) -> InteropError {
    InteropError::Dx12(error.to_string())
}
fn invalid(message: &'static str) -> InteropError {
    InteropError::InvalidFrame(message)
}
fn raw(host: &HostWgpuContext) -> Result<(ID3D12Device, ID3D12CommandQueue)> {
    // SAFETY: retain the native references while their owning wgpu device lives.
    unsafe {
        let device = host
            .device
            .as_hal::<wgpu::hal::api::Dx12>()
            .ok_or(invalid("DX12 device required"))?;
        let queue = host
            .queue
            .as_hal::<wgpu::hal::api::Dx12>()
            .ok_or(invalid("DX12 queue required"))?;
        Ok((device.raw_device().clone(), queue.as_raw().clone()))
    }
}
/// Stable adapter identity within this Windows boot; not a cross-boot ID.
///
/// # Errors
/// Returns an error if the context does not expose a DX12 device/queue.
pub fn adapter_luid(host: &HostWgpuContext) -> Result<u64> {
    let (device, _) = raw(host)?;
    // SAFETY: `raw` retained a live native device for this call.
    let luid = unsafe { device.GetAdapterLuid() };
    Ok((u64::from(luid.HighPart as u32) << 32) | u64::from(luid.LowPart))
}
fn share<P: windows::core::Param<ID3D12DeviceChild>>(
    device: &ID3D12Device,
    object: P,
) -> Result<OwnedHandle> {
    // SAFETY: the live device and child interface stay borrowed for the call.
    let handle =
        unsafe { device.CreateSharedHandle(object, None, GENERIC_ALL.0, None) }.map_err(dx)?;
    // SAFETY: CreateSharedHandle returned exactly one owned NT handle.
    Ok(unsafe { OwnedHandle::from_raw_handle(handle.0) })
}
fn extent(host: &HostWgpuContext, size: FrameSize) -> Result<()> {
    let limit = host.device.limits().max_texture_dimension_2d;
    if size.width == 0 || size.height == 0 || size.width > limit || size.height > limit {
        return Err(invalid("DX12 surface extent exceeds device limits"));
    }
    Ok(())
}
fn common(encoder: &mut wgpu::CommandEncoder, texture: &wgpu::Texture) {
    // wgpu's DX12 PRESENT state maps to D3D12_RESOURCE_STATE_COMMON.
    encoder.transition_resources(
        std::iter::empty(),
        std::iter::once(wgpu::TextureTransition {
            texture,
            selector: None,
            state: wgpu::TextureUses::PRESENT,
        }),
    );
}
fn wrap(resource: ID3D12Resource, size: FrameSize, host: &HostWgpuContext) -> wgpu::Texture {
    let desc = wgpu::TextureDescriptor {
        label: Some("graft shared DX12 surface"),
        size: wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    };
    // SAFETY: the caller establishes exclusive COMMON-state ownership and
    // verifies native shape. This resource was created/opened on this device.
    unsafe {
        let texture = wgpu_hal::dx12::Device::texture_from_raw(
            resource,
            desc.format,
            desc.dimension,
            desc.size,
            1,
            1,
        );
        host.device.create_texture_from_hal::<wgpu::hal::api::Dx12>(
            texture,
            &desc,
            wgpu::TextureUses::PRESENT,
        )
    }
}
/// Producer-owned allocation. Its wgpu wrapper never escapes; copies finish in
/// COMMON and no new writes are accepted while an export is leased.
pub struct SharedDx12Surface {
    host: HostWgpuContext,
    texture: wgpu::Texture,
    resource_handle: OwnedHandle,
    fence_handle: OwnedHandle,
    fence: ID3D12Fence,
    queue: ID3D12CommandQueue,
    value: u64,
    leased: bool,
}
impl SharedDx12Surface {
    /// Allocate one same-adapter RGBA8 texture and shared fence.
    ///
    /// # Errors
    /// Rejects invalid extents, non-DX12 contexts and native allocation failures.
    pub fn new(host: &HostWgpuContext, size: FrameSize) -> Result<Self> {
        extent(host, size)?;
        let (device, queue) = raw(host)?;
        let desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
            Alignment: 0,
            Width: u64::from(size.width),
            Height: size.height,
            DepthOrArraySize: 1,
            MipLevels: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
            Flags: D3D12_RESOURCE_FLAG_NONE,
        };
        let heap = D3D12_HEAP_PROPERTIES {
            Type: D3D12_HEAP_TYPE_DEFAULT,
            CreationNodeMask: 1,
            VisibleNodeMask: 1,
            ..Default::default()
        };
        let mut resource: Option<ID3D12Resource> = None;
        // SAFETY: validated descriptors and extent, live device, owned output.
        unsafe {
            device.CreateCommittedResource(
                &heap,
                D3D12_HEAP_FLAG_SHARED,
                &desc,
                D3D12_RESOURCE_STATE_COMMON,
                None,
                &mut resource,
            )
        }
        .map_err(dx)?;
        let resource = resource.ok_or(invalid("DX12 allocation returned null"))?;
        // SAFETY: live device; the typed output owns the created fence.
        let fence: ID3D12Fence =
            unsafe { device.CreateFence(0, D3D12_FENCE_FLAG_SHARED) }.map_err(dx)?;
        let resource_handle = share(&device, &resource)?;
        let fence_handle = share(&device, &fence)?;
        Ok(Self {
            host: HostWgpuContext::new(host.device.clone(), host.queue.clone()),
            texture: wrap(resource, size, host),
            resource_handle,
            fence_handle,
            fence,
            queue,
            value: 0,
            leased: false,
        })
    }
    pub fn size(&self) -> FrameSize {
        FrameSize::new(self.texture.width(), self.texture.height())
    }
    pub fn producer_value(&self) -> u64 {
        self.value
    }
    /// Copy on the producer's exact device. The caller must serialize access
    /// to this queue with other native submissions and fence operations.
    ///
    /// # Safety
    /// `source` must belong to this exact host device and have no concurrent
    /// native writers; all prior source writes must have been submitted.
    ///
    /// # Errors
    /// Rejects a leased allocation, incompatible source, exhausted fence counter
    /// or native queue signal failure.
    pub unsafe fn copy_from(&mut self, source: &wgpu::Texture) -> Result<()> {
        if self.leased {
            return Err(invalid("DX12 surface still leased"));
        }
        if source.width() != self.texture.width()
            || source.height() != self.texture.height()
            || source.format() != wgpu::TextureFormat::Rgba8Unorm
            || source.dimension() != wgpu::TextureDimension::D2
            || source.depth_or_array_layers() != 1
            || source.sample_count() != 1
            || !source.usage().contains(wgpu::TextureUsages::COPY_SRC)
        {
            return Err(invalid("DX12 copy source shape/usage mismatch"));
        }
        let value = self
            .value
            .checked_add(if self.value == 0 { 1 } else { 2 })
            .filter(|v| *v < u64::MAX)
            .ok_or(invalid("DX12 fence value exhausted"))?;
        let mut encoder = self.host.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_texture(
            source.as_image_copy(),
            self.texture.as_image_copy(),
            self.texture.size(),
        );
        common(&mut encoder, &self.texture);
        self.host.queue.submit([encoder.finish()]);
        self.value = value;
        self.leased = true;
        // SAFETY: exclusive serialized producer queue ownership; the value is
        // fresh and follows the submitted COMMON-state transition.
        unsafe { self.queue.Signal(&self.fence, value) }.map_err(dx)?;
        Ok(())
    }
    /// Duplicate local handle ownership for transport. Transport must duplicate
    /// each into the receiving process; integer handle values are not portable.
    ///
    /// # Errors
    /// Rejects an unpublished allocation or failed handle duplication.
    pub fn export(&self) -> Result<(OwnedHandle, OwnedHandle, u64)> {
        if !self.leased {
            return Err(invalid("DX12 surface has no publication"));
        }
        Ok((
            self.resource_handle
                .try_clone()
                .map_err(|e| InteropError::Dx12(e.to_string()))?,
            self.fence_handle
                .try_clone()
                .map_err(|e| InteropError::Dx12(e.to_string()))?,
            self.value,
        ))
    }
    /// Observe producer GPU completion without waiting.
    ///
    /// # Errors
    /// Reports native device removal.
    pub fn poll_ready(&self) -> Result<bool> {
        // SAFETY: this object retains the live native fence.
        let value = unsafe { self.fence.GetCompletedValue() };
        if value == u64::MAX {
            return Err(invalid("DX12 device removed"));
        }
        Ok(self.leased && value >= self.value)
    }
    /// # Safety
    /// The host must have validated the exact current lease's release after
    /// final imported texture/view destruction. A fence or peer exit alone is
    /// insufficient. The even GPU value is independently checked here.
    ///
    /// # Errors
    /// Rejects absent/duplicate leases, incomplete consumer fences or device loss.
    pub unsafe fn acknowledge_release(&mut self) -> Result<()> {
        // SAFETY: this object retains the live native fence.
        let value = unsafe { self.fence.GetCompletedValue() };
        if !self.leased || value == u64::MAX || value < self.value + 1 {
            return Err(invalid("DX12 consumer has not completed release"));
        }
        self.leased = false;
        Ok(())
    }
}
/// Import exactly one RGBA8 allocation and its matching producer fence.
///
/// Host transport must bind both handles/value to one trusted producer publication
/// on the same adapter, with exclusive custody until exact lease release. The
/// producer must have submitted COMMON-state transition before signalling the
/// positive odd value; imported aliases may not be used after release sealing.
///
/// # Errors
/// Rejects invalid metadata/fence values, mismatched native shape, duplicate
/// native-object imports and native resource/fence open or queue-wait failures.
pub fn import_surface(
    resource: BorrowedHandle<'_>,
    fence: BorrowedHandle<'_>,
    value: u64,
    size: FrameSize,
    host: &HostWgpuContext,
) -> Result<(wgpu::Texture, Dx12Release)> {
    extent(host, size)?;
    if value == 0 || value % 2 == 0 || value == u64::MAX {
        return Err(invalid("invalid DX12 producer fence value"));
    }
    let (device, queue) = raw(host)?;
    let mut native: Option<ID3D12Resource> = None;
    let mut ready: Option<ID3D12Fence> = None;
    // SAFETY: borrowed capabilities stay live through both typed native opens.
    unsafe {
        device
            .OpenSharedHandle(HANDLE(resource.as_raw_handle()), &mut native)
            .map_err(dx)?;
        device
            .OpenSharedHandle(HANDLE(fence.as_raw_handle()), &mut ready)
            .map_err(dx)?;
    }
    let native = native.ok_or(invalid("null shared DX12 resource"))?;
    let ready = ready.ok_or(invalid("null shared DX12 fence"))?;
    // SAFETY: the typed open produced a live resource reference.
    let desc = unsafe { native.GetDesc() };
    if desc.Dimension != D3D12_RESOURCE_DIMENSION_TEXTURE2D
        || desc.Width != u64::from(size.width)
        || desc.Height != size.height
        || desc.DepthOrArraySize != 1
        || desc.MipLevels != 1
        || desc.Format != DXGI_FORMAT_R8G8B8A8_UNORM
        || desc.SampleDesc.Count != 1
        || desc.SampleDesc.Quality != 0
        || desc.Flags != D3D12_RESOURCE_FLAG_NONE
    {
        return Err(invalid("DX12 resource differs from declared shape"));
    }
    // SAFETY: this fence belongs only to the live consumer device, so producer
    // exit cannot turn it into a spurious completion witness.
    let completion: ID3D12Fence =
        unsafe { device.CreateFence(0, D3D12_FENCE_FLAG_NONE) }.map_err(dx)?;
    let dropped = lifetime::attach(&native)?;
    // SAFETY: host transport binds this live fence and odd publication value;
    // the producer submits the COMMON transition before signalling it.
    unsafe { queue.Wait(&ready, value) }.map_err(dx)?;
    let texture = wrap(native, size, host);
    let release = Dx12Release::new(host, queue, ready, completion, value + 1, dropped);
    Ok((texture, release))
}
