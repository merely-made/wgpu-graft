# servo-wgpu-interop-adapter

Servo-specific offscreen rendering adapter built on [`grafting`](../grafting/).

This crate bridges Servo's rendering context to the host application. It provides two things:

1. **`ServoWgpuRenderingContext`** — an offscreen `RenderingContext` that Servo renders into. Supports CPU readback via `read_full_frame()` (returns an `image::RgbaImage` of the current page).
2. **`ServoWgpuInteropAdapter`** — native GPU import path that imports Servo's GL framebuffer into a host `wgpu::Texture` via the core interop crate.

## Which path to use

- **CPU readback** (`ServoWgpuRenderingContext::read_full_frame()`): Works on all platforms. Simpler to integrate — just display the returned image in your framework's image widget. Adds a GPU→CPU→GPU round-trip per frame.
- **GPU import** (`ServoWgpuInteropAdapter`): Avoids CPU frame readback, but requires compatible native sharing support between Servo's GL producer and the host wgpu backend. Default GL normalization copies the imported surface into a fresh GPU texture with top-left origin. Linux uses Vulkan external memory and Apple uses IOSurface/Metal. On Windows this adapter requires DX12 to match the host's physical GPU and imports ANGLE D3D11 output through a shared NT handle. Graft's lower-level Windows Vulkan importer is a separate route; this adapter constructor refuses a Vulkan host device.

The CPU readback demos ([xilem](../demo-servo-xilem/), [iced](../demo-servo-iced/), [gpui](../demo-servo-gpui/)) use `read_full_frame()`. The [winit demo](../demo-servo-winit/) tries GPU import first and falls back to CPU readback.

## Feature flags

- **`servo`** (optional) — enables the exact Git-pinned upstream Servo dependency and Servo trait implementations. All Servo-embedding demos enable this feature.
- **`wgpu-28`**, **`wgpu-29`**, **`wgpu-30`** — select the host wgpu type family. Enable exactly one; the default is `wgpu-29`.

Without `servo`, only the surfman-level types are available (useful for testing the adapter layer without pulling in all of Servo).

## Usage

The adapter is unpublished. Use its exact Git revision; the published Graft
core package has a separate distribution status.

```toml
[dependencies]
servo-wgpu-interop-adapter = { git = "https://github.com/merely-made/wgpu-graft", rev = "dec11bbd2c9c8676e66987fb6ca32cd4ae6310eb", default-features = false, features = ["wgpu-30", "servo"] }
servo = { git = "https://github.com/servo/servo", rev = "1d44e5dd6a8b64c02f9dbf7fcbdf4ebdd0740019" }
```

```rust
use servo_wgpu_interop_adapter::ServoWgpuRenderingContext;
use winit::dpi::PhysicalSize;

// Create the rendering context (implements Servo's RenderingContext trait)
let render_ctx = ServoWgpuRenderingContext::new(PhysicalSize::new(1024, 600))?;

// After Servo paints, read the frame as an RGBA image
if let Some(rgba_image) = render_ctx.read_full_frame() {
    // Display in your framework's image widget
}
```

See the demo crates for complete integration examples.

For GPU import, hand `adapter.rendering_context()` to `WebViewBuilder`, then
paint, present through that same importing context, and take its frame:

```rust
webview.paint();
adapter.rendering_context().present();
let imported = adapter.take_imported_texture();
```

The pinned upstream `WebView::paint()` does not present. The explicit present
call invokes the import hook before swapping GL buffers. A failed import leaves
no frame; a host that requires GPU import must reject that absence. On Windows,
enable upstream Servo's `no-wgl` feature and stage the matching build's ANGLE
DLLs beside the executable. The bounded winit smoke gate exercises this sequence
without accepting its regular CPU fallback.

The current source discards swap-chain preservation after importing the frame.
Pinned Servo clears the entire context and renders with buffer age zero on each
explicit paint. This avoids Surfman's ANGLE preservation blit between default
framebuffer IDs while retaining GL completion. The Windows wgpu 30 donor has
passed GPU initial-pixel, click, and resize checks; host integration,
accessibility, and full teardown remain open in the
[qualification receipt](../docs/receipts/servo_present_hook_20261006/README.md).

## License

[MPL-2.0](../LICENSE)
