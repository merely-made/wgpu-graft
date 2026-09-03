// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Version-edge compatibility for `zed-font-kit`'s direct Unix dependency.
//!
//! This package intentionally has no `links` key. Its 0.23 dependency owns
//! the one native FreeType link shared with Servo 0.5.

pub use freetype_sys_upstream::*;
