// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Offscreen upstream Servo views on a host-owned `wgpu` device.
//!
//! This is the engine library's producer, the Servo counterpart of
//! `scrying`'s `WebSurfaceProducer`: it owns the process root, the per-view
//! `WebView` and its delegate, input/cursor/navigation/accessibility
//! translation, and frame import through the adapters in this crate. It is
//! host-agnostic and names no embedder types; hosts such as inker's
//! `graft-engine` adapt it.
//!
//! The process root lives on the UI thread, including between the last closed
//! view and a later reopen. Upstream options and JS initialization are process
//! scoped, so [`ServoFactory::new`] succeeds once per process. Each view has
//! its own rendering context and one ordered delegate queue. The host must
//! release surfaces and cached frames before [`shutdown`].

mod delegate;
mod keys;
mod process;
mod surface;
#[cfg(test)]
mod tests;
mod types;

pub use process::{ServoFactory, active_views, pump, shutdown};
pub use surface::ServoSurface;
pub use types::{
    A11yFocusPolicy, ServoCursor, ServoEvent, ServoFrame, ServoFramePayload, ServoImportedFrame,
    ServoKeyEvent, ServoModifiers, ServoMouseButton, ServoMouseEvent, ServoMouseKind,
    ServoProducerConfig, ServoProducerError, ServoViewRequest, parse_gpu_sync,
};
