# Runtime Validation

This workspace ships multiple demos embedding Servo in different GUI frameworks, plus a standalone GL→wgpu demo. This document covers what to validate and how.

## Quick commands

```bash
# Core crate tests
cargo test -p grafting

# Build checks
cargo check -p servo-wgpu-interop-adapter --features servo
cargo check -p demo-servo-winit
cargo check -p demo-servo-xilem
cargo check -p demo-servo-gpui
cargo check -p demo-servo-bevy
cargo check -p demo-servo-blitz
cargo check -p demo-servo-egui
cargo check -p demo-servo-slint
cargo check --manifest-path demo-servo-iced/Cargo.toml
cargo check -p demo-raw-gl

# Run demos
cargo run -p demo-servo-winit
cargo run -p demo-servo-xilem
cargo run -p demo-servo-gpui
cargo run -p demo-servo-bevy
cargo run -p demo-servo-blitz
cargo run -p demo-servo-egui
cargo run -p demo-servo-slint
cargo run --manifest-path demo-servo-iced/Cargo.toml
cargo run -p demo-raw-gl

# All root-workspace Servo demos from an ordinary Windows PowerShell session
pwsh -File scripts/check-servo-demos.ps1

# Deterministic reference-demo gate on Windows / PowerShell
pwsh -File scripts/smoke-demo.ps1

# Deterministic reference-demo gate from a logged-in macOS session
bash scripts/smoke-demo-mac.sh
```

The check script uses two Cargo jobs. Servo 0.5's WebRender 0.70 shader build
needs one jobserver worker in addition to the build script itself, so `-j 1`
does not make progress. It checks demos in separate Cargo invocations because
Bevy and Xilem otherwise unify incompatible diagnostics features across their
independent Naga versions.

The deterministic smoke succeeds only after the imported texture contains the
fixture's expected pixel, a forwarded mouse click changes that pixel, and the
imported frame reaches the requested resized dimensions. `-SurvivalOnly`
retains the old process-liveness probe for a demo without a self-test mode,
but it is not a rendering receipt.

The macOS wrapper puts the built binary in a minimal temporary app bundle and
launches it through LaunchServices. This is required on self-hosted runners:
direct children of the runner worker can connect to AppKit but may never enter
the logged-in GUI event context.

On Windows without `nasm`, prefix with `AWS_LC_SYS_NO_ASM=1`. In PowerShell:

```powershell
$env:AWS_LC_SYS_NO_ASM=1; cargo run -p demo-servo-winit
```

If ESP-IDF has set `LIBCLANG_PATH` to its Xtensa libclang,
the PowerShell check and smoke scripts select the desktop LLVM installation
before building Servo/mozangle.

## Windows: ANGLE DLLs

Servo requires ANGLE on Windows. The `mozangle` crate builds `libEGL.dll` and `libGLESv2.dll` during compilation, but they may not end up next to the executable — especially when using a custom `CARGO_TARGET_DIR`.

Find them in `target/debug/build/mozangle-*/out/` and copy to your target's `debug/` directory.

## What to validate

1. **Startup**: the demo window opens without panics.
2. **First paint**: a web page appears (not a blank or solid-color window).
3. **Animation**: the default `animated.html` fixture updates continuously, not just one frame.
4. **Resize**: the content tracks the window size without stretching or freezing.
5. **Navigation**: where input is wired, clicking links navigates to new pages.
6. **Scrolling**: where input is wired, mouse wheel events scroll long pages.
7. **Text input**: demos with a URL bar accept text and navigate on Enter.
8. **Keyboard forwarding**: where input is wired, keyboard events reach the page.
9. **Repeated navigation**: loading several URLs in sequence does not crash.

## Demo-specific notes

### demo-servo-winit

- Logs the URL, host backend, and capability matrix to stdout on startup.
- Window title shows the active backend, sync mode, and imported texture size.
- Tries GPU import first; falls back to CPU readback if GL extensions are missing.
- No URL bar — pass URLs via command line.

### demo-servo-xilem

- URL bar + Go button above the viewport.
- Frame delivery via `tokio::sync::watch` channel.

### demo-servo-iced

- URL bar above the viewport.
- Uses `image::allocate()` for flicker-free frame upload.

### demo-servo-gpui

- URL bar above the viewport with focus management.
- RGBA→BGRA conversion for GPUI's `RenderImage` format.
- Continuous rendering via `request_animation_frame()`.

### demo-servo-blitz

- GPU import through Blitz's `anyrender_vello` renderer and wgpu 29.
- Mouse, wheel, and keyboard input forwarding.

### demo-servo-egui

- GPU import through egui's native-texture registration.
- Mouse, wheel, text, and non-text keyboard forwarding.
- Optional `cpu-readback` comparison feature.

### demo-servo-slint

- GPU import through Slint 1.17's `unstable-wgpu-29` texture surface.
- Currently display-only.

### demo-servo-bevy

- Windows/DX12 shared-handle import into Bevy's render world.
- Currently display-only.

### demo-raw-gl

- No Servo dependency — renders a spinning GL triangle.
- Validates the core interop layer independently.
- Should show a smoothly spinning triangle on all supported platforms.

## Fixtures

Each Servo demo includes fixtures in its `fixtures/` directory:

- `animated.html` — frame counter + CSS animations for redraw validation.
- `static.html` — static page for orientation and color checks where present.
- `smoke.html` (winit) — fixed initial and clicked colors for the bounded
  imported-pixel/input/resize gate.

## Platform expectations

| Demo | Render path | Platforms checked in hosted CI | Headed receipt |
| --- | --- | --- | --- |
| winit | GPU import, CPU fallback | Linux, macOS, Windows | RADV, NVIDIA DX12, M4 Metal, Intel Metal |
| Blitz | GPU import | Linux, macOS, Windows | manual |
| egui | GPU import, optional CPU fallback | Linux, macOS, Windows | manual |
| Slint | GPU import | Linux, macOS, Windows | manual |
| Bevy | GPU import | Windows | manual |
| Iced | GPU import, optional CPU fallback | Windows | manual |
| Xilem | CPU readback | Linux, macOS, Windows | manual |
| GPUI | CPU readback | Linux, macOS, Windows | manual |
| raw GL | GPU import | Linux, Windows | low-level tests plus manual window |
