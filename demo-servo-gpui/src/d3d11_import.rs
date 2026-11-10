// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Windows zero-copy path: Servo's frame, exported by grafting as a shared
//! D3D11 texture, opened on GPUI's own Direct3D 11 device and handed to
//! bite-gpui's renderer as an imported texture.
//!
//! Servo renders through ANGLE on its own D3D11 device, on the same GPU as
//! GPUI's (matched by adapter LUID). grafting blits each frame into a shared
//! `R8G8B8A8_UNORM` texture with a bottom-left origin and finishes the GL
//! work. bite-gpui takes only `B8G8R8A8` textures made on its device, so one
//! draw on GPUI's device copies the shared texture into a reused BGRA texture,
//! flipping it upright. The draw runs in a device-context state of its own so
//! GPUI's bound state is untouched, and an event query waits for it before
//! ANGLE writes the next frame into the shared texture. No pixels reach the
//! CPU.

use std::ffi::c_void;
use std::os::windows::io::AsRawHandle as _;
use std::rc::Rc;

use gpui::{GpuCanvasContext, ImportedTextureHandle};
use gpui_windows::DirectXTextureExt as _;
use servo::RenderingContext;
use servo_wgpu_interop_adapter::ServoWgpuRenderingContext;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::{D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, ID3DBlob};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_QUERY_DESC, D3D11_QUERY_EVENT,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_VIEWPORT, ID3D11Device, ID3D11Device1,
    ID3D11DeviceContext, ID3D11DeviceContext1, ID3D11PixelShader, ID3D11Query,
    ID3D11RenderTargetView, ID3D11ShaderResourceView, ID3D11Texture2D, ID3D11VertexShader,
    ID3DDeviceContextState,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::core::{BOOL, Interface as _, PCSTR, s};
use winit::dpi::PhysicalSize;

/// GPUI's Direct3D 11 device, borrowed by the boot frame.
pub type Host = ID3D11Device;

/// `None` when GPUI's renderer is not the Direct3D one.
pub fn host(canvas: &mut GpuCanvasContext) -> Option<Host> {
    canvas.try_device::<gpui_windows::DirectXRenderer>()
}

/// Draws the shared RGBA texture into the BGRA target, flipped upright.
const SHADER: &str = r"
Texture2D<float4> source : register(t0);

float4 vs_main(uint id : SV_VertexID) : SV_Position {
    float2 uv = float2((id << 1) & 2, id & 2);
    return float4(uv * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
}

float4 ps_main(float4 position : SV_Position) : SV_Target {
    uint width, height;
    source.GetDimensions(width, height);
    uint2 pixel = uint2(position.xy);
    return source.Load(int3(pixel.x, height - 1 - pixel.y, 0));
}
";

/// The shared texture as opened on GPUI's device, keyed by grafting's
/// allocation so it is reopened only when the producer reallocates it.
struct Shared {
    key: u64,
    view: ID3D11ShaderResourceView,
}

/// The BGRA texture GPUI samples, reallocated when the frame size changes.
struct Target {
    size: PhysicalSize<u32>,
    view: ID3D11RenderTargetView,
    handle: ImportedTextureHandle,
}

/// Servo's frames, exported by grafting and handed to GPUI each frame.
pub struct GpuImport {
    render_ctx: Rc<ServoWgpuRenderingContext>,
    device: ID3D11Device,
    device1: ID3D11Device1,
    context1: ID3D11DeviceContext1,
    state: ID3DDeviceContextState,
    vertex_shader: ID3D11VertexShader,
    pixel_shader: ID3D11PixelShader,
    done: ID3D11Query,
    shared: Option<Shared>,
    target: Option<Target>,
}

fn err(context: &str) -> impl FnOnce(windows::core::Error) -> String + '_ {
    move |error| format!("{context}: {error}")
}

fn compile(entry: PCSTR, target: PCSTR) -> Result<Vec<u8>, String> {
    let mut code: Option<ID3DBlob> = None;
    let mut errors: Option<ID3DBlob> = None;
    let result = unsafe {
        D3DCompile(
            SHADER.as_ptr().cast::<c_void>(),
            SHADER.len(),
            PCSTR::null(),
            None,
            None,
            entry,
            target,
            0,
            0,
            &mut code,
            Some(&mut errors),
        )
    };
    if let Err(error) = result {
        let message = errors
            .map(|blob| unsafe {
                let bytes = std::slice::from_raw_parts(
                    blob.GetBufferPointer().cast::<u8>(),
                    blob.GetBufferSize(),
                );
                String::from_utf8_lossy(bytes).into_owned()
            })
            .unwrap_or_default();
        return Err(format!("shader compile failed: {error} {message}"));
    }
    let code = code.ok_or("shader compile returned no code")?;
    Ok(unsafe {
        std::slice::from_raw_parts(code.GetBufferPointer().cast::<u8>(), code.GetBufferSize())
    }
    .to_vec())
}

impl GpuImport {
    pub fn new(size: PhysicalSize<u32>, device: Host) -> Result<Self, String> {
        // Render Servo on GPUI's GPU: a shared handle does not cross GPUs.
        let luid = unsafe {
            device
                .cast::<IDXGIDevice>()
                .map_err(err("IDXGIDevice"))?
                .GetAdapter()
                .map_err(err("GetAdapter"))?
                .GetDesc()
                .map_err(err("adapter GetDesc"))?
                .AdapterLuid
        };
        let render_ctx = Rc::new(
            ServoWgpuRenderingContext::new_for_adapter_luid(size, luid.LowPart, luid.HighPart)
                .map_err(|error| format!("{error:?}"))?,
        );

        let device1: ID3D11Device1 = device.cast().map_err(err("ID3D11Device1"))?;
        let context: ID3D11DeviceContext =
            unsafe { device.GetImmediateContext() }.map_err(err("GetImmediateContext"))?;
        let context1: ID3D11DeviceContext1 = context.cast().map_err(err("ID3D11DeviceContext1"))?;

        let mut state = None;
        unsafe {
            device1
                .CreateDeviceContextState(
                    0,
                    &[device.GetFeatureLevel()],
                    windows::Win32::Graphics::Direct3D11::D3D11_SDK_VERSION,
                    &ID3D11Device::IID,
                    None,
                    Some(&mut state),
                )
                .map_err(err("CreateDeviceContextState"))?;
        }
        let state = state.ok_or("CreateDeviceContextState returned no state")?;

        let vs_code = compile(s!("vs_main"), s!("vs_5_0"))?;
        let ps_code = compile(s!("ps_main"), s!("ps_5_0"))?;
        let mut vertex_shader = None;
        let mut pixel_shader = None;
        let mut done = None;
        unsafe {
            device
                .CreateVertexShader(&vs_code, None, Some(&mut vertex_shader))
                .map_err(err("CreateVertexShader"))?;
            device
                .CreatePixelShader(&ps_code, None, Some(&mut pixel_shader))
                .map_err(err("CreatePixelShader"))?;
            device
                .CreateQuery(
                    &D3D11_QUERY_DESC {
                        Query: D3D11_QUERY_EVENT,
                        MiscFlags: 0,
                    },
                    Some(&mut done),
                )
                .map_err(err("CreateQuery"))?;
        }

        Ok(Self {
            render_ctx,
            device,
            device1,
            context1,
            state,
            vertex_shader: vertex_shader.ok_or("no vertex shader")?,
            pixel_shader: pixel_shader.ok_or("no pixel shader")?,
            done: done.ok_or("no query")?,
            shared: None,
            target: None,
        })
    }

    pub fn rendering_context(&self) -> Rc<dyn RenderingContext> {
        self.render_ctx.clone()
    }

    /// Export Servo's painted frame and copy it into GPUI's texture. Returns
    /// the frame size when a new frame arrived.
    pub fn present(&mut self) -> Result<Option<(u32, u32)>, String> {
        let frame = self
            .render_ctx
            .current_dx12_shared_texture()
            .map_err(|error| format!("{error:?}"))?;
        let size = frame.metadata().size;
        if size.width == 0 || size.height == 0 {
            return Ok(None);
        }

        let resource = frame.resource();
        let key = resource.allocation_key();
        if self.shared.as_ref().is_none_or(|shared| shared.key != key) {
            let handle = HANDLE(resource.owned_resource().as_handle().as_raw_handle());
            let texture: ID3D11Texture2D = unsafe { self.device1.OpenSharedResource1(handle) }
                .map_err(err("OpenSharedResource1"))?;
            let mut view = None;
            unsafe {
                self.device
                    .CreateShaderResourceView(&texture, None, Some(&mut view))
                    .map_err(err("shared CreateShaderResourceView"))?;
            }
            self.shared = Some(Shared {
                key,
                view: view.ok_or("no shared view")?,
            });
        }
        if self
            .target
            .as_ref()
            .is_none_or(|target| target.size != size)
        {
            self.target = Some(self.create_target(size)?);
        }

        let (Some(shared), Some(target)) = (&self.shared, &self.target) else {
            return Ok(None);
        };
        unsafe {
            // Swap in this pass's own state so GPUI's bindings are restored
            // exactly afterwards.
            let mut previous = None;
            self.context1
                .SwapDeviceContextState(&self.state, Some(&mut previous));
            self.context1
                .OMSetRenderTargets(Some(&[Some(target.view.clone())]), None);
            self.context1.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: size.width as f32,
                Height: size.height as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            }]));
            self.context1
                .IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            self.context1.VSSetShader(&self.vertex_shader, None);
            self.context1.PSSetShader(&self.pixel_shader, None);
            self.context1
                .PSSetShaderResources(0, Some(&[Some(shared.view.clone())]));
            self.context1.Draw(3, 0);
            self.context1.PSSetShaderResources(0, Some(&[None]));
            self.context1.OMSetRenderTargets(None, None);
            self.context1.End(&self.done);
            if let Some(previous) = previous {
                self.context1.SwapDeviceContextState(&previous, None);
            }

            // ANGLE writes the next frame into the same shared texture, so
            // wait until this draw has read it.
            self.context1.Flush();
            let mut finished = BOOL(0);
            loop {
                self.context1
                    .GetData(
                        &self.done,
                        Some((&raw mut finished).cast::<c_void>()),
                        size_of::<BOOL>() as u32,
                        0,
                    )
                    .map_err(err("GetData"))?;
                if finished.as_bool() {
                    break;
                }
                std::thread::yield_now();
            }
        }
        Ok(Some((size.width, size.height)))
    }

    fn create_target(&self, size: PhysicalSize<u32>) -> Result<Target, String> {
        let mut texture = None;
        let mut view = None;
        unsafe {
            self.device
                .CreateTexture2D(
                    &D3D11_TEXTURE2D_DESC {
                        Width: size.width,
                        Height: size.height,
                        MipLevels: 1,
                        ArraySize: 1,
                        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: 1,
                            Quality: 0,
                        },
                        Usage: D3D11_USAGE_DEFAULT,
                        BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0)
                            as u32,
                        CPUAccessFlags: 0,
                        MiscFlags: 0,
                    },
                    None,
                    Some(&mut texture),
                )
                .map_err(err("target CreateTexture2D"))?;
        }
        let texture: ID3D11Texture2D = texture.ok_or("no target texture")?;
        unsafe {
            self.device
                .CreateRenderTargetView(&texture, None, Some(&mut view))
                .map_err(err("target CreateRenderTargetView"))?;
        }
        let handle = texture
            .to_imported_handle()
            .map_err(|error| format!("{error:#}"))?;
        Ok(Target {
            size,
            view: view.ok_or("no target view")?,
            handle,
        })
    }

    pub fn latest(&self) -> Option<ImportedTextureHandle> {
        self.target.as_ref().map(|target| target.handle.clone())
    }
}
