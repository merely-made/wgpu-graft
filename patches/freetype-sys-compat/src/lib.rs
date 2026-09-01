//! Version-edge compatibility for `zed-font-kit`'s direct Unix dependency.
//!
//! This package intentionally has no `links` key. Its 0.23 dependency owns
//! the one native FreeType link shared with Servo 0.5.

pub use freetype_sys_upstream::*;
