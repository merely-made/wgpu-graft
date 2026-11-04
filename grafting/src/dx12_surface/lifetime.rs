// SPDX-License-Identifier: MPL-2.0
//! A resource-private IUnknown witnesses actual imported COM resource death.
//! No resource reference is retained in the witness (which would form a cycle).
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, Ordering},
};
use windows::core::{GUID, HRESULT, IUnknown, IUnknown_Vtbl, Interface};
const KEY: GUID = GUID::from_u128(0xb8d29c3c_a998_4e1b_8518_a2cc58d3c6d7);
#[repr(C)]
struct Witness {
    vtable: *const IUnknown_Vtbl,
    refs: AtomicU32,
    dropped: Arc<AtomicBool>,
}
unsafe extern "system" fn query(
    this: *mut std::ffi::c_void,
    iid: *const GUID,
    out: *mut *mut std::ffi::c_void,
) -> HRESULT {
    if out.is_null() {
        return HRESULT(0x80004003u32 as i32);
    }
    // SAFETY: the COM caller supplies writable output storage when non-null.
    unsafe {
        *out = std::ptr::null_mut();
    }
    // SAFETY: the COM caller supplies a readable IID when non-null.
    if !iid.is_null() && unsafe { *iid == IUnknown::IID } {
        // SAFETY: `this` is our live interface; return one owned reference.
        unsafe {
            add(this);
            *out = this;
        }
        HRESULT(0)
    } else {
        HRESULT(0x80004002u32 as i32)
    }
}
unsafe extern "system" fn add(this: *mut std::ffi::c_void) -> u32 {
    // SAFETY: COM dispatch provides our live repr(C) interface pointer.
    unsafe {
        (*this.cast::<Witness>())
            .refs
            .fetch_add(1, Ordering::Relaxed)
            + 1
    }
}
unsafe extern "system" fn release(this: *mut std::ffi::c_void) -> u32 {
    // SAFETY: each COM owner releases exactly one live reference; atomic count
    // allows calls from native threads without retaining a resource cycle.
    let remaining = unsafe {
        (*this.cast::<Witness>())
            .refs
            .fetch_sub(1, Ordering::AcqRel)
            - 1
    };
    if remaining == 0 {
        // SAFETY: the final reference uniquely owns the original Box allocation.
        let witness = unsafe { Box::from_raw(this.cast::<Witness>()) };
        witness.dropped.store(true, Ordering::Release);
    }
    remaining
}
static VTABLE: IUnknown_Vtbl = IUnknown_Vtbl {
    QueryInterface: query,
    AddRef: add,
    Release: release,
};
pub(super) fn attach(resource: &ID3D12Resource) -> Result<Arc<AtomicBool>> {
    // Never replace a live witness on a previously opened native object.
    let mut existing_size = 0;
    // SAFETY: live resource; this size-only query installs no interface reference.
    if unsafe { resource.GetPrivateData(&KEY, &mut existing_size, None) }.is_ok()
        || existing_size != 0
    {
        return Err(invalid(
            "DX12 resource already has an active lifetime witness",
        ));
    }
    let dropped = Arc::new(AtomicBool::new(false));
    let witness = Box::new(Witness {
        vtable: &VTABLE,
        refs: AtomicU32::new(1),
        dropped: dropped.clone(),
    });
    // SAFETY: repr(C) IUnknown with a static ABI-correct vtable and one owned
    // reference. SetPrivateDataInterface retains one; this local reference drops.
    let unknown = unsafe { IUnknown::from_raw(Box::into_raw(witness).cast()) };
    // SAFETY: the resource retains one reference to the ABI-correct IUnknown.
    unsafe { resource.SetPrivateDataInterface(&KEY, &unknown) }.map_err(dx)?;
    Ok(dropped)
}
/// Consumer final-use token. Dropping it abandons notification, never frees a
/// producer lease. Seal after the final use, then drop every texture/view alias.
pub struct Dx12Release {
    device: wgpu::Device,
    queue: wgpu::Queue,
    native_queue: ID3D12CommandQueue,
    fence: ID3D12Fence,
    completion: ID3D12Fence,
    value: u64,
    dropped: Arc<AtomicBool>,
    sealed: bool,
}
impl Dx12Release {
    pub(super) fn new(
        host: &HostWgpuContext,
        native_queue: ID3D12CommandQueue,
        fence: ID3D12Fence,
        completion: ID3D12Fence,
        value: u64,
        dropped: Arc<AtomicBool>,
    ) -> Self {
        Self {
            device: host.device.clone(),
            queue: host.queue.clone(),
            native_queue,
            fence,
            completion,
            value,
            dropped,
            sealed: false,
        }
    }
    /// The texture must be the exact imported texture paired with this token.
    /// All final uses must already be submitted on its queue; submit no further
    /// use of any alias afterwards. Serialized host queue ownership is required.
    ///
    /// # Errors
    /// Rejects duplicate seals, a different/unmarked texture or native failures.
    pub fn seal(&mut self, texture: &wgpu::Texture) -> Result<()> {
        if self.sealed {
            return Err(invalid("DX12 consumer use already sealed"));
        }
        // SAFETY: the HAL guard retains the native texture. Our private GUID
        // stores only a Witness; GetPrivateData returns an AddRef-owned pointer.
        // Match that witness before releasing the temporary interface reference.
        unsafe {
            let native = texture
                .as_hal::<wgpu::hal::api::Dx12>()
                .ok_or(invalid("DX12 texture required"))?;
            let mut bytes = std::mem::size_of::<*mut std::ffi::c_void>() as u32;
            let mut pointer = std::ptr::null_mut::<std::ffi::c_void>();
            native
                .raw_resource()
                .GetPrivateData(
                    &KEY,
                    &mut bytes,
                    Some((&mut pointer as *mut *mut std::ffi::c_void).cast()),
                )
                .map_err(dx)?;
            if pointer.is_null() {
                return Err(invalid("DX12 release texture has no witness"));
            }
            // GetPrivateData AddRefs stored interfaces; adopt and drop that ref.
            let witness = &*pointer.cast::<Witness>();
            let paired = Arc::ptr_eq(&witness.dropped, &self.dropped);
            drop(IUnknown::from_raw(pointer));
            if !paired {
                return Err(invalid("DX12 release texture mismatch"));
            }
        }
        let mut encoder = self.device.create_command_encoder(&Default::default());
        common(&mut encoder, texture);
        self.queue.submit([encoder.finish()]);
        self.sealed = true;
        // SAFETY: the host serializes this queue; this signal follows the final
        // submitted COMMON barrier and retains the matching fence.
        unsafe { self.native_queue.Signal(&self.fence, self.value) }.map_err(dx)?;
        // SAFETY: independent fence belongs to this consumer device. A removed
        // producer can poison its shared fence with UINT64_MAX; that value is
        // never evidence that this consumer's last work has completed.
        unsafe { self.native_queue.Signal(&self.completion, 1) }.map_err(dx)?;
        Ok(())
    }
    /// Observe both GPU completion and native-object destruction.
    ///
    /// # Errors
    /// Reports polling failures or native device removal.
    pub fn poll_released(&self) -> Result<bool> {
        self.device
            .poll(wgpu::PollType::Poll)
            .map_err(InteropError::NormalizationCompletion)?;
        // SAFETY: this token retains the native fence through polling.
        let value = unsafe { self.completion.GetCompletedValue() };
        if value == u64::MAX {
            return Err(invalid("DX12 device removed"));
        }
        Ok(self.sealed && value >= 1 && self.dropped.load(Ordering::Acquire))
    }
}
