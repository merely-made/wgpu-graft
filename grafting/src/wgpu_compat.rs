//! The one seam where the carried wgpu majors disagree on a call signature.
//!
//! wgpu 30 made the texture tracker's initial state an explicit parameter of
//! `Device::create_texture_from_hal`; wgpu 28/29 hardcoded
//! `TextureUses::UNINITIALIZED` internally. Every import path in this crate
//! wraps a texture some external producer already wrote, so the correct value
//! is the same everywhere: UNINITIALIZED, meaning the first consumer submit
//! transitions from an undefined layout, with producer-side writes ordered by
//! the frame's sync mechanism (keyed mutex / fence / semaphore), not by
//! layout. Routing all ten call sites through here keeps the version split in
//! one place.

/// Wrap an already-created HAL texture in a `wgpu::Texture`, identically
/// across the carried wgpu majors.
///
/// # Safety
///
/// Same contract as `wgpu::Device::create_texture_from_hal`: `hal_texture`
/// must come from this device, match `desc`, and be initialized.
pub(crate) unsafe fn create_texture_from_hal<A: wgpu_hal::Api>(
    device: &wgpu::Device,
    hal_texture: A::Texture,
    desc: &wgpu::TextureDescriptor<'_>,
) -> wgpu::Texture {
    #[cfg(feature = "wgpu-30")]
    unsafe {
        device.create_texture_from_hal::<A>(hal_texture, desc, wgpu::TextureUses::UNINITIALIZED)
    }
    #[cfg(not(feature = "wgpu-30"))]
    unsafe {
        device.create_texture_from_hal::<A>(hal_texture, desc)
    }
}
