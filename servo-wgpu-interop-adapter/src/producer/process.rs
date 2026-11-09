// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::{Arc, OnceLock},
    thread::ThreadId,
};

use servo::{EventLoopWaker, Opts, Preferences, Servo, ServoBuilder, WebViewBuilder};
use url::Url;
use winit::dpi::PhysicalSize;

use super::{
    delegate::Delegate,
    surface::{Interop, ServoSurface},
    types::{A11yFocusPolicy, ServoProducerConfig, ServoProducerError, ServoViewRequest},
};

static PROCESS_OWNER: OnceLock<ThreadId> = OnceLock::new();
thread_local! {
    static ROOT: RefCell<Option<Rc<ProcessRoot>>> = const { RefCell::new(None) };
    static INITIALIZED: Cell<bool> = const { Cell::new(false) };
}

pub(super) struct ProcessRoot {
    pub servo: Servo,
    pub config: ServoProducerConfig,
    pub views: Cell<usize>,
    pub wake: Arc<dyn Fn() + Send + Sync>,
}

#[derive(Clone)]
struct Waker(Arc<dyn Fn() + Send + Sync>);

impl EventLoopWaker for Waker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(self.clone())
    }
    fn wake(&self) {
        (self.0)();
    }
}

pub(super) fn spawn_error(detail: impl Into<String>) -> ServoProducerError {
    ServoProducerError::Spawn(detail.into())
}

pub(super) fn require_owner(owner: ThreadId) -> Result<(), ServoProducerError> {
    if std::thread::current().id() == owner {
        Ok(())
    } else {
        Err(spawn_error("Servo operation requires its UI thread"))
    }
}

/// Builds views on the process root it initialized.
pub struct ServoFactory {
    owner: ThreadId,
    config: ServoProducerConfig,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl ServoFactory {
    #[must_use]
    pub fn config(&self) -> &ServoProducerConfig {
        &self.config
    }

    #[must_use]
    pub fn profile_name(&self) -> &str {
        &self.config.profile_name
    }

    #[must_use]
    pub fn profile_dir(&self) -> &std::path::Path {
        &self.config.profile_dir
    }

    #[must_use]
    pub fn a11y_focus_policy(&self) -> A11yFocusPolicy {
        self.config.a11y_focus
    }

    /// The host device and queue this factory imports onto. Frames are paired
    /// to this exact pair.
    #[must_use]
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    #[must_use]
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Construct exactly once on the UI thread. The named profile is shared
    /// by this process's Servo views, independently of any per-node profile.
    ///
    /// # Errors
    /// Refuses an unprovisioned host device (Linux), an empty profile,
    /// construction off the original UI thread, and a second construction.
    pub fn new(
        config: ServoProducerConfig,
        device: wgpu::Device,
        queue: wgpu::Queue,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self, ServoProducerError> {
        #[cfg(target_os = "linux")]
        {
            let host = grafting::HostWgpuContext::new(device.clone(), queue.clone());
            // Inspect the enabled native extensions before process-global
            // Servo initialization. Recovery cannot silently lose native import.
            let foreign =
                // SAFETY: the guard is only inspected and dropped.
                unsafe { device.as_hal::<wgpu::wgc::api::Vulkan>() }.is_some_and(|device| {
                    device
                        .enabled_device_extensions()
                        .contains(&c"VK_EXT_queue_family_foreign")
                });
            if host.backend != grafting::InteropBackend::Vulkan || !host.dmabuf_support || !foreign
            {
                return Err(spawn_error(
                    "Servo DMA-BUF requires a Vulkan host provisioned for foreign native import",
                ));
            }
        }
        if config.profile_name.trim().is_empty() || config.profile_dir.as_os_str().is_empty() {
            return Err(spawn_error(
                "Servo needs an explicitly named profile directory",
            ));
        }
        let owner = std::thread::current().id();
        if PROCESS_OWNER.get().is_some_and(|actual| *actual != owner) {
            return Err(spawn_error(
                "Servo must be constructed on its original UI thread",
            ));
        }
        ROOT.with(|slot| {
            if slot.borrow().is_some() || INITIALIZED.with(Cell::get) {
                return Err(spawn_error(
                    "Servo process root is already initialized; reuse its factory",
                ));
            }
            std::fs::create_dir_all(&config.profile_dir)
                .map_err(|error| spawn_error(error.to_string()))?;
            let opts = Opts {
                config_dir: Some(config.profile_dir.clone()),
                ..Opts::default()
            };
            // Record before upstream initialization: its global options cannot
            // safely be retried, even if construction unwinds.
            if PROCESS_OWNER.set(owner).is_err() {
                require_owner(*PROCESS_OWNER.get().expect("Servo owner was assigned"))?;
            }
            INITIALIZED.with(|initialized| initialized.set(true));
            if rustls::crypto::CryptoProvider::get_default().is_none() {
                // An existing process provider wins, including an install
                // racing this one. Servo's net initialization needs a default.
                let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
            }
            let servo = ServoBuilder::default()
                .opts(opts)
                .preferences(Preferences {
                    accessibility_enabled: true,
                    ..Preferences::default()
                })
                .event_loop_waker(Box::new(Waker(wake.clone())))
                .build();
            *slot.borrow_mut() = Some(Rc::new(ProcessRoot {
                servo,
                config: config.clone(),
                views: Cell::new(0),
                wake,
            }));
            Ok(Self {
                owner,
                config,
                device,
                queue,
            })
        })
    }

    fn root(&self) -> Result<Rc<ProcessRoot>, ServoProducerError> {
        require_owner(self.owner)?;
        ROOT.with(|slot| slot.borrow().clone())
            .ok_or_else(|| spawn_error("Servo process root is shut down"))
    }

    /// Open a view.
    ///
    /// # Errors
    /// Fails if the root was shut down, the call is off the UI thread, the
    /// URL is invalid, or the platform import adapter cannot be built.
    pub fn build(&self, request: &ServoViewRequest) -> Result<ServoSurface, ServoProducerError> {
        let root = self.root()?;
        if root.config != self.config {
            return Err(spawn_error(
                "Servo process configuration differs from the initialized root",
            ));
        }
        let url = Url::parse(&request.url).map_err(|error| spawn_error(error.to_string()))?;
        let size = PhysicalSize::new(request.width.max(1), request.height.max(1));
        let interop = Interop::new(&self.device, &self.queue, size, &self.config)?;
        let delegate = Rc::new(Delegate::new(request.url.clone(), root.wake.clone()));
        let view = WebViewBuilder::new(&root.servo, interop.rendering_context())
            .url(url)
            .delegate(delegate.clone())
            .build();
        root.views.set(root.views.get() + 1);
        Ok(ServoSurface {
            view: Some(view),
            interop: Some(interop),
            delegate,
            root,
            frame_epoch: 0,
            device: self.device.clone(),
            queue: self.queue.clone(),
            pressed_keys: HashMap::new(),
        })
    }
}

/// Spin Servo's event loop on its UI thread. A no-op before construction.
///
/// # Errors
/// Fails off the process's UI thread.
pub fn pump() -> Result<(), ServoProducerError> {
    if let Some(owner) = PROCESS_OWNER.get() {
        require_owner(*owner)?;
    }
    let root = ROOT.with(|slot| slot.borrow().clone());
    if let Some(root) = root {
        root.servo.spin_event_loop();
    }
    Ok(())
}

/// Live views on the process root.
#[must_use]
pub fn active_views() -> usize {
    ROOT.with(|slot| slot.borrow().as_ref().map_or(0, |root| root.views.get()))
}

/// Drop the process root. Surfaces and cached frames must be released first.
///
/// # Errors
/// [`ServoProducerError::Busy`] while views are live; fails off the UI thread.
pub fn shutdown() -> Result<(), ServoProducerError> {
    if let Some(owner) = PROCESS_OWNER.get() {
        require_owner(*owner)?;
    }
    ROOT.with(|slot| {
        if slot
            .borrow()
            .as_ref()
            .is_some_and(|root| root.views.get() != 0)
        {
            return Err(ServoProducerError::Busy(
                "Servo shutdown with live views".into(),
            ));
        }
        let root = slot.borrow_mut().take();
        drop(root);
        Ok(())
    })
}
