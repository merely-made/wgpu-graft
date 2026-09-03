// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The one seam where the carried wgpu majors disagree on a call signature.
//!
//! wgpu 30 made the texture tracker's initial state an explicit parameter of
//! `Device::create_texture_from_hal`; wgpu 28/29 hardcoded
//! `TextureUses::UNINITIALIZED` internally. wgpu 30 makes the imported
//! resource's state explicit, so each import path supplies the state it has
//! established at the HAL boundary. Routing the version split through here
//! keeps 28/29 behavior intact while making the 30 contract visible at every
//! import site.

/// Wrap an already-created HAL texture in a `wgpu::Texture`, identically
/// across the carried wgpu majors.
///
/// # Safety
///
/// Same contract as `wgpu::Device::create_texture_from_hal`: `hal_texture`
/// must come from this device, match `desc`, and be initialized. On wgpu 30,
/// `initial_state` must match the resource's actual backend state.
pub(crate) unsafe fn create_texture_from_hal<A: wgpu_hal::Api>(
    device: &wgpu::Device,
    hal_texture: A::Texture,
    desc: &wgpu::TextureDescriptor<'_>,
    initial_state: wgpu::TextureUses,
) -> wgpu::Texture {
    #[cfg(feature = "wgpu-30")]
    unsafe {
        device.create_texture_from_hal::<A>(hal_texture, desc, initial_state)
    }
    #[cfg(not(feature = "wgpu-30"))]
    let _ = initial_state;
    #[cfg(not(feature = "wgpu-30"))]
    unsafe {
        device.create_texture_from_hal::<A>(hal_texture, desc)
    }
}
