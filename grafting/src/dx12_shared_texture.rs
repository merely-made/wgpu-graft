// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Direct D3D12 shared-handle → `wgpu::Texture` import path.
//!
//! For producers that have an `ID3D12Resource` (or D3D11 resource with shared
//! NT handle) and want it imported into wgpu's DX12 backend zero-copy. The
//! producer creates the resource with `D3D12_HEAP_FLAG_SHARED` and exports an
//! NT handle via `IDXGIResource1::CreateSharedHandle`; the importer opens its
//! own reference via `ID3D12Device::OpenSharedHandle`.

use std::os::windows::io::{AsRawHandle, BorrowedHandle};

use crate::{Dx12SharedTexture, FrameMetadata, HostWgpuContext, InteropBackend, InteropError};

pub fn import_dx12_shared_texture(
    frame: Dx12SharedTexture,
    host: &HostWgpuContext,
) -> Result<wgpu::Texture, InteropError> {
    import_dx12_shared_handle(frame.raw_handle(), frame.metadata(), host)
}

/// Import a borrowed D3D12/D3D11 NT shared handle without taking ownership of
/// the handle itself.
///
/// Prefer [`import_dx12_shared_texture`], whose safe frame owns the handle
/// custody. This escape hatch exists for integrations that cannot yet hand
/// Graft an owned handle.
///
/// # Safety
///
/// `handle` must designate a live DXGI NT shared resource and remain valid for
/// the duration of this call. The producer must keep it in
/// `D3D12_RESOURCE_STATE_COMMON` before synchronization. This function does
/// not close the borrowed handle.
pub unsafe fn import_dx12_shared_handle_borrowed(
    handle: BorrowedHandle<'_>,
    metadata: FrameMetadata,
    host: &HostWgpuContext,
) -> Result<wgpu::Texture, InteropError> {
    import_dx12_shared_handle(handle.as_raw_handle(), metadata, host)
}

fn import_dx12_shared_handle(
    handle: *mut std::ffi::c_void,
    metadata: FrameMetadata,
    host: &HostWgpuContext,
) -> Result<wgpu::Texture, InteropError> {
    if host.backend != InteropBackend::Dx12 {
        return Err(InteropError::BackendMismatch {
            expected: "Dx12",
            actual: "non-Dx12",
        });
    }

    let texture = unsafe {
        let hal_device =
            host.device
                .as_hal::<wgpu::wgc::api::Dx12>()
                .ok_or(InteropError::BackendMismatch {
                    expected: "Dx12",
                    actual: "non-Dx12",
                })?;

        let d3d_device = hal_device.raw_device().clone();
        let mut resource: Option<windows::Win32::Graphics::Direct3D12::ID3D12Resource> = None;
        d3d_device
            .OpenSharedHandle(windows::Win32::Foundation::HANDLE(handle), &mut resource)
            .map_err(|e| InteropError::Dx12(e.to_string()))?;
        let resource =
            resource.ok_or_else(|| InteropError::Dx12("OpenSharedHandle returned null".into()))?;

        let hal_texture = wgpu_hal::dx12::Device::texture_from_raw(
            resource,
            metadata.format,
            wgpu::TextureDimension::D2,
            wgpu::Extent3d {
                width: metadata.size.width,
                height: metadata.size.height,
                depth_or_array_layers: 1,
            },
            1, // mip_level_count
            1, // sample_count
        );

        let desc = wgpu::TextureDescriptor {
            label: Some("dx12-shared-texture-import"),
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
        };

        crate::wgpu_compat::create_texture_from_hal::<wgpu_hal::api::Dx12>(
            &host.device,
            hal_texture,
            &desc,
            // D3D12 shared resources are handed over in COMMON state.
            wgpu::TextureUses::UNINITIALIZED,
        )
    };

    Ok(texture)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::io::FromRawHandle;

    use windows::Win32::{
        Foundation::{GetHandleInformation, HANDLE},
        System::Threading::CreateEventW,
    };

    fn test_metadata() -> FrameMetadata {
        FrameMetadata {
            size: dpi::PhysicalSize::new(1, 1),
            format: wgpu::TextureFormat::Rgba8Unorm,
            generation: 7,
            producer_sync: crate::SyncMechanism::None,
        }
    }

    #[test]
    fn resource_token_keeps_the_handle_alive_after_frame_consumption() {
        let handle = unsafe { CreateEventW(None, false, false, None) }.expect("CreateEventW");
        let raw = handle.0;
        let owned = unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(raw) };
        let resource = crate::Dx12SharedResource::from_owned_handle(owned);
        let allocation_key = resource.allocation_key();

        let frame = Dx12SharedTexture::new(test_metadata(), resource.clone(), 0);
        assert_eq!(frame.allocation_key(), allocation_key);
        drop(frame);

        let mut flags = 0;
        unsafe { GetHandleInformation(HANDLE(raw), &mut flags) }
            .expect("resource token retains the handle");
        drop(resource);
        assert!(unsafe { GetHandleInformation(HANDLE(raw), &mut flags) }.is_err());
    }

    #[test]
    fn frame_metadata_stays_copyable_while_resource_custody_is_explicit() {
        let handle = unsafe { CreateEventW(None, false, false, None) }.expect("CreateEventW");
        let raw = handle.0;
        let owned = unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(raw) };
        let resource = crate::Dx12SharedResource::from_owned_handle(owned);
        let metadata = test_metadata();
        let frame = Dx12SharedTexture::new(metadata, resource, 5);

        assert_eq!(frame.metadata(), metadata);
        assert_eq!(frame.fence_value(), 5);
        drop(frame);

        let mut flags = 0;
        assert!(unsafe { GetHandleInformation(HANDLE(raw), &mut flags) }.is_err());
    }
}
