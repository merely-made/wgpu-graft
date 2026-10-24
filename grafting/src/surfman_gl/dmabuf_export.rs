// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

//! Immutable top-left GL snapshots exported as move-owned DMA-BUFs.
//!
//! The producer polls a GL fence with a zero timeout. Only completed snapshots
//! cross the native-frame boundary, so FrameSync::None means producer-complete,
//! not implicit GL/Vulkan ordering. No CPU pixels or completion wait are used.
use super::SurfmanFrameContext;
use crate::{InteropError, NeutralNativeFrame};
use glow::HasContext;
use grafting_frame::{
    DmaBufImage, DmaBufPlane, FrameMetadata, FrameSize, FrameSync, OwnedNativeImage, PixelFormat,
};
use std::{
    ffi::{CStr, c_void},
    os::fd::{FromRawFd, OwnedFd},
    rc::Rc,
};

pub struct PendingDmaBufFrame {
    context: Rc<SurfmanFrameContext>,
    texture: Option<glow::NativeTexture>,
    framebuffer: Option<glow::NativeFramebuffer>,
    fence: Option<glow::NativeFence>,
    frame: Option<NeutralNativeFrame>,
    backing: Option<LinearBacking>,
}
impl PendingDmaBufFrame {
    /// Poll on the creating GL thread. Pending returns immediately and retains
    /// all custody. A completed frame can be taken only once.
    pub fn try_take_frame(&mut self) -> Result<Option<NeutralNativeFrame>, InteropError> {
        if self.frame.is_none() {
            return Ok(None);
        }
        self.context
            .make_current()
            .map_err(|e| InteropError::Surfman(format!("{e:?}")))?;
        let gl = &self.context.glow_gl;
        match unsafe { gl.client_wait_sync(self.fence.expect("live export fence"), 0, 0) } {
            glow::ALREADY_SIGNALED | glow::CONDITION_SATISFIED => Ok(self.frame.take()),
            glow::TIMEOUT_EXPIRED => Ok(None),
            _ => Err(InteropError::OpenGl(
                "DMA-BUF snapshot fence poll failed".into(),
            )),
        }
    }
}
impl Drop for PendingDmaBufFrame {
    fn drop(&mut self) {
        // The context itself is retained until its snapshot resources retire.
        // GL deletion defers outstanding uses; dropping a pending export never
        // waits and never makes its descriptors available to the consumer.
        if self.context.make_current().is_ok() {
            let gl = &self.context.glow_gl;
            unsafe {
                if let Some(f) = self.fence.take() {
                    gl.delete_sync(f);
                }
                if let Some(f) = self.framebuffer.take() {
                    gl.delete_framebuffer(f);
                }
                if let Some(t) = self.texture.take() {
                    gl.delete_texture(t);
                }
            }
        }
        // EGL images must retire while the owning display/context remains live.
        self.backing.take();
    }
}

struct Egl {
    library: *mut c_void,
    display: *mut c_void,
    create_image:
        unsafe extern "C" fn(*mut c_void, *mut c_void, u32, *mut c_void, *const i32) -> *mut c_void,
    destroy_image: unsafe extern "C" fn(*mut c_void, *mut c_void) -> u32,
    query_display: unsafe extern "C" fn(*mut c_void, i32, *mut isize) -> u32,
    query_device_string: unsafe extern "C" fn(*mut c_void, i32) -> *const std::ffi::c_char,
    image_target: unsafe extern "C" fn(u32, *mut c_void),
}
impl Drop for Egl {
    fn drop(&mut self) {
        unsafe {
            libc::dlclose(self.library);
        }
    }
}
impl Egl {
    fn load() -> Result<Self, InteropError> {
        unsafe {
            let library = libc::dlopen(c"libEGL.so.1".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
            if library.is_null() {
                return Err(InteropError::OpenGl("libEGL.so.1 unavailable".into()));
            }
            let result = (|| {
                let get_proc = libc::dlsym(library, c"eglGetProcAddress".as_ptr());
                if get_proc.is_null() {
                    return Err(InteropError::OpenGl("eglGetProcAddress unavailable".into()));
                }
                let get_proc: unsafe extern "C" fn(*const std::ffi::c_char) -> *const c_void =
                    std::mem::transmute(get_proc);
                let symbol = |name: &CStr| -> Result<*const c_void, InteropError> {
                    let direct = libc::dlsym(library, name.as_ptr());
                    let pointer = if direct.is_null() {
                        get_proc(name.as_ptr())
                    } else {
                        direct.cast_const()
                    };
                    if pointer.is_null() {
                        Err(InteropError::OpenGl(format!(
                            "missing EGL entry point {name:?}"
                        )))
                    } else {
                        Ok(pointer)
                    }
                };
                let display: unsafe extern "C" fn() -> *mut c_void =
                    std::mem::transmute(symbol(c"eglGetCurrentDisplay")?);
                let context: unsafe extern "C" fn() -> *mut c_void =
                    std::mem::transmute(symbol(c"eglGetCurrentContext")?);
                let query_string: unsafe extern "C" fn(
                    *mut c_void,
                    i32,
                ) -> *const std::ffi::c_char = std::mem::transmute(symbol(c"eglQueryString")?);
                let display = display();
                if display.is_null() || context().is_null() {
                    return Err(InteropError::OpenGl(
                        "DMA-BUF export requires a current EGL context".into(),
                    ));
                }
                let extensions = query_string(display, 0x3055);
                if extensions.is_null()
                    || !CStr::from_ptr(extensions)
                        .to_bytes()
                        .split(|b| *b == b' ')
                        .any(|e| e == b"EGL_EXT_image_dma_buf_import")
                {
                    return Err(InteropError::OpenGl(
                        "EGL_EXT_image_dma_buf_import unavailable".into(),
                    ));
                }
                Ok(Self {
                    library,
                    display,
                    create_image: std::mem::transmute(symbol(c"eglCreateImageKHR")?),
                    destroy_image: std::mem::transmute(symbol(c"eglDestroyImageKHR")?),
                    query_display: std::mem::transmute(symbol(c"eglQueryDisplayAttribEXT")?),
                    query_device_string: std::mem::transmute(symbol(c"eglQueryDeviceStringEXT")?),
                    image_target: std::mem::transmute(symbol(c"glEGLImageTargetTexture2DOES")?),
                })
            })();
            if result.is_err() {
                libc::dlclose(library);
            }
            result
        }
    }
    fn linear_image(
        self,
        size: dpi::PhysicalSize<u32>,
    ) -> Result<(LinearBacking, DmaBufImage), InteropError> {
        use std::os::{fd::AsRawFd, unix::ffi::OsStrExt};
        unsafe {
            let mut device = 0isize;
            if (self.query_display)(self.display, 0x322C, &mut device) == 0 || device == 0 {
                return Err(InteropError::OpenGl(
                    "EGL display does not expose its DRM device".into(),
                ));
            }
            let node = (self.query_device_string)(device as *mut c_void, 0x3377);
            if node.is_null() {
                return Err(InteropError::OpenGl(
                    "EGL device has no DRM render node".into(),
                ));
            }
            let node = std::ffi::OsStr::from_bytes(CStr::from_ptr(node).to_bytes());
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(node)
                .map_err(|e| InteropError::OpenGl(format!("open EGL DRM render node: {e}")))?;
            let gbm = Gbm::load()?;
            let mut backing = LinearBacking {
                egl: self,
                gbm,
                device: std::ptr::null_mut(),
                bo: std::ptr::null_mut(),
                image: std::ptr::null_mut(),
                _node: file,
            };
            backing.device = (backing.gbm.create_device)(backing._node.as_raw_fd());
            if backing.device.is_null() {
                return Err(InteropError::OpenGl("GBM device creation failed".into()));
            }
            let fourcc = u32::from_le_bytes(*b"AB24");
            let linear = 0u64;
            backing.bo = (backing.gbm.create_bo)(
                backing.device,
                size.width,
                size.height,
                fourcc,
                &linear,
                1,
                1 << 2,
            );
            if backing.bo.is_null() {
                return Err(InteropError::OpenGl(
                    "GBM cannot allocate a renderable linear RGBA snapshot".into(),
                ));
            }
            let modifier = (backing.gbm.modifier)(backing.bo);
            if modifier != 0 || (backing.gbm.planes)(backing.bo) != 1 {
                return Err(InteropError::OpenGl(
                    "GBM did not supply the requested single-plane linear layout".into(),
                ));
            }
            let stride = (backing.gbm.stride)(backing.bo);
            let offset = (backing.gbm.offset)(backing.bo, 0);
            let raw = (backing.gbm.fd)(backing.bo);
            if raw < 0 {
                return Err(InteropError::OpenGl(
                    "GBM DMA-BUF descriptor export failed".into(),
                ));
            }
            let fd = OwnedFd::from_raw_fd(raw);
            if stride == 0 || stride > i32::MAX as u32 || offset > i32::MAX as u32 {
                return Err(InteropError::InvalidFrame(
                    "GBM snapshot plane exceeds EGL layout limits",
                ));
            }
            let attrs = [
                0x3057,
                size.width as i32,
                0x3056,
                size.height as i32,
                0x3271,
                fourcc as i32,
                0x3272,
                fd.as_raw_fd(),
                0x3273,
                offset as i32,
                0x3274,
                stride as i32,
                0x3038,
            ];
            backing.image = (backing.egl.create_image)(
                backing.egl.display,
                std::ptr::null_mut(),
                0x3270,
                std::ptr::null_mut(),
                attrs.as_ptr(),
            );
            if backing.image.is_null() {
                return Err(InteropError::OpenGl(
                    "EGL cannot import the linear GBM snapshot".into(),
                ));
            }
            let image = DmaBufImage::new(
                vec![fd],
                vec![DmaBufPlane {
                    buffer_index: 0,
                    offset: offset.into(),
                    stride: stride.into(),
                }],
                fourcc,
                modifier,
            )
            .map_err(|e| InteropError::OpenGl(e.to_string()))?;
            Ok((backing, image))
        }
    }
}
struct Gbm {
    library: *mut c_void,
    create_device: unsafe extern "C" fn(i32) -> *mut c_void,
    destroy_device: unsafe extern "C" fn(*mut c_void),
    create_bo:
        unsafe extern "C" fn(*mut c_void, u32, u32, u32, *const u64, u32, u32) -> *mut c_void,
    destroy_bo: unsafe extern "C" fn(*mut c_void),
    modifier: unsafe extern "C" fn(*mut c_void) -> u64,
    stride: unsafe extern "C" fn(*mut c_void) -> u32,
    offset: unsafe extern "C" fn(*mut c_void, i32) -> u32,
    planes: unsafe extern "C" fn(*mut c_void) -> i32,
    fd: unsafe extern "C" fn(*mut c_void) -> i32,
}
impl Drop for Gbm {
    fn drop(&mut self) {
        unsafe {
            libc::dlclose(self.library);
        }
    }
}
impl Gbm {
    fn load() -> Result<Self, InteropError> {
        unsafe {
            let library = libc::dlopen(c"libgbm.so.1".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
            if library.is_null() {
                return Err(InteropError::OpenGl("libgbm.so.1 unavailable".into()));
            }
            let result = (|| {
                let symbol = |name: &CStr| -> Result<*mut c_void, InteropError> {
                    let p = libc::dlsym(library, name.as_ptr());
                    if p.is_null() {
                        Err(InteropError::OpenGl(format!(
                            "missing GBM entry point {name:?}"
                        )))
                    } else {
                        Ok(p)
                    }
                };
                Ok(Self {
                    library,
                    create_device: std::mem::transmute(symbol(c"gbm_create_device")?),
                    destroy_device: std::mem::transmute(symbol(c"gbm_device_destroy")?),
                    create_bo: std::mem::transmute(symbol(c"gbm_bo_create_with_modifiers2")?),
                    destroy_bo: std::mem::transmute(symbol(c"gbm_bo_destroy")?),
                    modifier: std::mem::transmute(symbol(c"gbm_bo_get_modifier")?),
                    stride: std::mem::transmute(symbol(c"gbm_bo_get_stride")?),
                    offset: std::mem::transmute(symbol(c"gbm_bo_get_offset")?),
                    planes: std::mem::transmute(symbol(c"gbm_bo_get_plane_count")?),
                    fd: std::mem::transmute(symbol(c"gbm_bo_get_fd")?),
                })
            })();
            if result.is_err() {
                libc::dlclose(library);
            }
            result
        }
    }
}
struct LinearBacking {
    egl: Egl,
    gbm: Gbm,
    device: *mut c_void,
    bo: *mut c_void,
    image: *mut c_void,
    _node: std::fs::File,
}
impl Drop for LinearBacking {
    fn drop(&mut self) {
        unsafe {
            if !self.image.is_null() {
                (self.egl.destroy_image)(self.egl.display, self.image);
            }
            if !self.bo.is_null() {
                (self.gbm.destroy_bo)(self.bo);
            }
            if !self.device.is_null() {
                (self.gbm.destroy_device)(self.device);
            }
        }
    }
}

pub(super) fn export(
    context: Rc<SurfmanFrameContext>,
    size: dpi::PhysicalSize<u32>,
    generation: u64,
) -> Result<PendingDmaBufFrame, InteropError> {
    if size.width == 0
        || size.height == 0
        || size.width > i32::MAX as u32
        || size.height > i32::MAX as u32
    {
        return Err(InteropError::InvalidFrame("invalid GL snapshot dimensions"));
    }
    context
        .make_current()
        .map_err(|e| InteropError::Surfman(format!("{e:?}")))?;
    let egl = Egl::load()?;
    let gl = context.glow_gl.clone();
    let mut pending = PendingDmaBufFrame {
        context,
        texture: None,
        framebuffer: None,
        fence: None,
        frame: None,
        backing: None,
    };
    unsafe {
        for _ in 0..16 {
            if gl.get_error() == glow::NO_ERROR {
                break;
            }
        }
        let read = gl.get_parameter_i32(glow::READ_FRAMEBUFFER_BINDING);
        let draw = gl.get_parameter_i32(glow::DRAW_FRAMEBUFFER_BINDING);
        let texture = gl.get_parameter_i32(glow::TEXTURE_BINDING_2D);
        let scissor = gl.is_enabled(glow::SCISSOR_TEST);
        let result = (|| {
            let t = gl.create_texture().map_err(InteropError::OpenGl)?;
            pending.texture = Some(t);
            gl.bind_texture(glow::TEXTURE_2D, Some(t));
            let (backing, image) = egl.linear_image(size)?;
            (backing.egl.image_target)(glow::TEXTURE_2D, backing.image);
            pending.backing = Some(backing);
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::NEAREST as i32,
            );
            let fbo = gl.create_framebuffer().map_err(InteropError::OpenGl)?;
            pending.framebuffer = Some(fbo);
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(fbo));
            gl.framebuffer_texture_2d(
                glow::DRAW_FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(t),
                0,
            );
            if gl.check_framebuffer_status(glow::DRAW_FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE {
                return Err(InteropError::OpenGl(
                    "DMA-BUF snapshot framebuffer incomplete".into(),
                ));
            }
            gl.disable(glow::SCISSOR_TEST);
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, pending.context.framebuffer());
            // Bind the attached producer surface, rather than assuming Servo
            // left its read-framebuffer binding at the final rendered target.
            // Flip exactly once here; the native envelope promises top-left pixels.
            gl.blit_framebuffer(
                0,
                0,
                size.width as i32,
                size.height as i32,
                0,
                size.height as i32,
                size.width as i32,
                0,
                glow::COLOR_BUFFER_BIT,
                glow::NEAREST,
            );
            if gl.get_error() != glow::NO_ERROR {
                return Err(InteropError::OpenGl(
                    "DMA-BUF snapshot GL copy failed".into(),
                ));
            }
            pending.fence = Some(
                gl.fence_sync(glow::SYNC_GPU_COMMANDS_COMPLETE, 0)
                    .map_err(InteropError::OpenGl)?,
            );
            gl.flush();
            pending.frame = Some(NeutralNativeFrame::new(
                FrameMetadata {
                    size: FrameSize::new(size.width, size.height),
                    format: PixelFormat::Rgba8Unorm,
                    generation,
                },
                OwnedNativeImage::DmaBufImage(image),
                FrameSync::None,
            ));
            Ok(())
        })();
        gl.bind_framebuffer(
            glow::READ_FRAMEBUFFER,
            std::num::NonZeroU32::new(read as u32).map(glow::NativeFramebuffer),
        );
        gl.bind_framebuffer(
            glow::DRAW_FRAMEBUFFER,
            std::num::NonZeroU32::new(draw as u32).map(glow::NativeFramebuffer),
        );
        gl.bind_texture(
            glow::TEXTURE_2D,
            std::num::NonZeroU32::new(texture as u32).map(glow::NativeTexture),
        );
        if scissor {
            gl.enable(glow::SCISSOR_TEST);
        }
        result?;
    }
    Ok(pending)
}
