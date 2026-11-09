// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

//! Host-neutral vocabulary of the Servo producer.

use std::{path::PathBuf, time::Duration};

use grafting::{DiagnosticGpuSync, ImportedTexture};

/// Failure classes the producer reports; hosts map them onto their own errors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServoProducerError {
    /// Process or view construction was refused or failed.
    Spawn(String),
    /// A lifecycle operation is blocked by live views.
    Busy(String),
    /// The producer does not support the requested operation.
    Unsupported(String),
    /// A frame could not be acquired or failed validation.
    Frame(String),
    /// A navigation request was invalid.
    Navigation(String),
    /// An input event could not be projected.
    Input(String),
    /// A configuration value was invalid.
    Config(String),
}

impl std::fmt::Display for ServoProducerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (kind, detail) = match self {
            Self::Spawn(d) => ("spawn failed", d),
            Self::Busy(d) => ("busy", d),
            Self::Unsupported(d) => ("unsupported", d),
            Self::Frame(d) => ("frame acquisition failed", d),
            Self::Navigation(d) => ("navigation failed", d),
            Self::Input(d) => ("input failed", d),
            Self::Config(d) => ("invalid configuration", d),
        };
        write!(f, "{kind}: {detail}")
    }
}

impl std::error::Error for ServoProducerError {}

/// How accessibility focus is projected at the producer boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum A11yFocusPolicy {
    /// Forward the supplier's focus unchanged.
    Upstream,
    /// Report the tree's document root as focused. Servo exports a fixed
    /// placeholder, not DOM keyboard focus; this partial projection never
    /// supplies missing tree metadata or nodes.
    DocumentRoot,
}

impl A11yFocusPolicy {
    /// Stable name, as accepted by [`A11yFocusPolicy::parse`].
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Upstream => "upstream",
            Self::DocumentRoot => "document-root",
        }
    }

    /// Parse a host setting; `None` selects the default, `document-root`.
    ///
    /// # Errors
    /// Any value other than `upstream` or `document-root` is refused.
    pub fn parse(value: Option<&str>) -> Result<Self, ServoProducerError> {
        match value {
            None | Some("document-root") => Ok(Self::DocumentRoot),
            Some("upstream") => Ok(Self::Upstream),
            Some(other) => Err(ServoProducerError::Config(format!(
                "accessibility focus policy must be upstream or document-root, got {other:?}"
            ))),
        }
    }

    pub(super) fn project(self, update: &mut servo::accesskit::TreeUpdate) {
        if self == Self::DocumentRoot {
            if let Some(tree) = &update.tree {
                update.focus = tree.root;
            }
        }
    }
}

/// Parse a GPU synchronization diagnostic (`existing`, `producer`,
/// `normalization` or `both`). The timeout applies to the waiting modes.
///
/// These explicit diagnostics preserve the existing production policy. Native
/// runners guard the process because GL producer completion has no timeout.
///
/// # Errors
/// Unknown modes are refused.
pub fn parse_gpu_sync(
    mode: &str,
    timeout: Duration,
) -> Result<DiagnosticGpuSync, ServoProducerError> {
    match mode {
        "existing" => Ok(DiagnosticGpuSync::Existing),
        "producer" => Ok(DiagnosticGpuSync::ProducerCompletion),
        "normalization" => Ok(DiagnosticGpuSync::NormalizationCompletion { timeout }),
        "both" => Ok(DiagnosticGpuSync::Both { timeout }),
        _ => Err(ServoProducerError::Config(
            "GPU sync must be existing, producer, normalization or both".into(),
        )),
    }
}

/// Process-scoped options chosen by the host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServoProducerConfig {
    /// Name of the process profile; must be non-empty.
    pub profile_name: String,
    /// Servo's config directory for this process; must be non-empty.
    pub profile_dir: PathBuf,
    /// Accessibility focus projection.
    pub a11y_focus: A11yFocusPolicy,
    /// Windows ANGLE/DX12 completion diagnostics. Ignored on Linux, where
    /// DMA-BUF snapshots carry their own completion.
    pub gpu_sync: DiagnosticGpuSync,
}

impl ServoProducerConfig {
    /// Production defaults: document-root focus, existing GPU sync.
    #[must_use]
    pub fn new(profile_name: impl Into<String>, profile_dir: impl Into<PathBuf>) -> Self {
        Self {
            profile_name: profile_name.into(),
            profile_dir: profile_dir.into(),
            a11y_focus: A11yFocusPolicy::DocumentRoot,
            gpu_sync: DiagnosticGpuSync::Existing,
        }
    }
}

/// A new view. Dimensions are clamped to at least one pixel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServoViewRequest {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

/// A texture already imported and normalized on the host device.
#[derive(Debug)]
pub struct ServoImportedFrame {
    pub imported: ImportedTexture,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub resource_epoch: u64,
}

/// Frame custody by platform path.
#[derive(Debug)]
pub enum ServoFramePayload {
    /// macOS/Windows: pre-present import onto the host device.
    Imported(ServoImportedFrame),
    /// Linux: a completed DMA-BUF snapshot, not yet imported.
    #[cfg(target_os = "linux")]
    DmaBuf(grafting::NeutralNativeFrame),
}

/// One composited frame.
#[derive(Debug)]
pub struct ServoFrame {
    pub payload: ServoFramePayload,
    pub width: u32,
    pub height: u32,
    /// Identity of the underlying allocation; advances per paint on the
    /// imported path and follows the snapshot generation on Linux.
    pub resource_epoch: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServoMouseButton {
    Left,
    Middle,
    Right,
    Back,
    Forward,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ServoMouseKind {
    Moved,
    Button {
        button: ServoMouseButton,
        pressed: bool,
    },
    ScrollPixels {
        dx: f32,
        dy: f32,
    },
    ScrollLines {
        dx: f32,
        dy: f32,
    },
}

/// Mouse input in device pixels of the view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ServoMouseEvent {
    pub x: f32,
    pub y: f32,
    pub kind: ServoMouseKind,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ServoModifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

/// Keyboard input as a host framework reports it: a virtual key code (ASCII
/// for letters/digits/space, DOM-style for named keys) plus composed text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServoKeyEvent {
    pub key_code: u32,
    pub modifiers: ServoModifiers,
    pub pressed: bool,
    pub text: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServoCursor {
    Default,
    Pointer,
    Text,
    Grab,
    Grabbing,
    Crosshair,
    Move,
    ResizeNs,
    ResizeEw,
    ResizeNesw,
    ResizeNwse,
    NotAllowed,
    Hidden,
}

/// Ordered delegate events: navigation, address and title changes in the
/// order upstream reported them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServoEvent {
    AddressChanged { url: String },
    TitleChanged { title: String },
    NavigationStarted { url: String },
    NavigationCommitted { url: String },
    NavigationFinished { url: String, title: Option<String> },
    NavigationFailed { url: String, reason: String },
    /// The load was stopped by the embedder or by script (`window.stop()`).
    NavigationStopped { url: String },
}
