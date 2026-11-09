# demo-servo-gpui

Servo embedded in a [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) application (Zed's UI framework) with a URL bar and full input forwarding. The GPUI here is [bite-gpui](https://github.com/bite-gpui/bite-gpui), a Zed fork, for the reason below.

## What it demonstrates

- macOS: zero-copy GPU import. grafting imports each Servo frame onto the Metal device GPUI's renderer uses, and GPUI draws it as an imported texture through `gpu_canvas`.
- Linux and Windows: CPU readback for now (`read_full_frame()`, RGBA→BGRA into a `RenderImage`). bite-gpui has the import path on both (wgpu on Linux, Direct3D 11 on Windows); wiring it here waits on a machine to check it on.
- Continuous rendering via `request_animation_frame()` in the `Render` implementation
- URL bar with focus management via `FocusHandle`
- Mouse, scroll, and keyboard events forwarded to Servo via GPUI's element event handlers
- Custom key mapping (GPUI uses its own key types, not winit)

## Usage

```bash
cargo run -p demo-servo-gpui                         # built-in animated fixture
cargo run -p demo-servo-gpui -- https://example.com  # load a URL
cargo run -p demo-servo-gpui -- servo.org            # auto-prefixes https://
```

## Why bite-gpui, not upstream GPUI

Upstream GPUI has no way to draw a texture produced outside it, so this demo could not do GPU import on Zed's GPUI or on `gpui-pre`, its weekly crates.io snapshot (checked at 0.3.10/0.3.11, Zed 2026-11-01):

- The only external-surface element, `surface()` / `paint_surface`, is macOS-only and draws YUV video frames (NV12 `CVPixelBuffer`s for Zed's screen sharing), not RGBA.
- On Linux the wgpu renderer's surface draw is an empty stub.
- Windows renders with Direct3D 11, not wgpu, so there is no wgpu device there to import onto.
- App code cannot reach GPUI's GPU device, encoder or render pass, so nothing outside GPUI can add the path.

Doing it on upstream would have meant carrying our own GPUI patch, which this repository does not want to maintain for a demo. A native layer placed over the GPUI window would avoid the patch, but GPUI could not draw over that region and it is OS composition rather than import into the host's frame.

The GPUI variants surveyed (2026-11-09), from the [gpui-archipelago fork map](https://github.com/gpui-archipelago/map-data) and each fork's source:

| Variant | External texture import |
|---|---|
| Zed `gpui`, `gpui-pre` | None (above). |
| gpui-unofficial, gpui-box, gpui-fast | None beyond upstream's YUV `paint_surface`. |
| gpui-ce | Linux and macOS (`custom-gpu`, a wgpu macOS renderer); nothing on Windows. Diverged from Zed in Dec 2025. |
| xgpui | `custom_draw` on Metal and wgpu; small, early. |
| gpui-fusion | Linux dma-buf for its media stack. |
| Glass-HQ/gpui | None; this demo's previous base, idle since 2026-05. |
| gpui-kit, gpui-mobile, sonorahq/gpui, Pulsar-Native, kael | None (gpui-kit is a component library on gpui-pre; kael is not a GPUI fork). |
| **bite-gpui** | **All three renderers**: `ImportedTextureHandle` + `gpu_canvas`/`paint_texture`, with Metal, wgpu and Direct3D 11 backends and a device accessor. Zed as of 2026-09-30 plus its own commits. |

bite-gpui is the only one covering every GPU backend, and it is published on crates.io, so the demo uses it as published.

## GPUI-specific notes

- bite-gpui is pre-1.0 and does not use winit, so this demo has its own key mapping in `keyutils.rs` rather than sharing the winit-based one used by the other demos.
- bite-gpui's `platform` feature also pulls its wasm backend, which pins `unicode-properties =0.1.3` against Servo's `^0.1.4`. The demo turns default features off and builds the native platform from `bite-gp-macos` / `bite-gp-linux` / `bite-gp-windows` directly.
- GPUI's `Pixels` type has a crate-private field, so all pixel math uses `f32::from(px)` rather than direct field access.
- bite-gpui's macOS renderer is Metal. The demo makes its own wgpu device, which lands on the same `MTLDevice` (macOS hands every creator the one device), and passes GPUI an sRGB view of the imported texture. It waits for the import's GPU work before GPUI samples it, since the two use different command queues.

## Platform notes

- **macOS**: GPU import; falls back to CPU readback (logged) if the wgpu device or adapter cannot be created.
- **Linux, Windows**: CPU readback.
- **Windows**: requires ANGLE DLLs next to the executable (see [workspace README](../README.md#prerequisites)).

## License

[MPL-2.0](../LICENSE)
