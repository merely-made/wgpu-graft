---
name: bevy + gpui zero-copy demos, plus deferred cleanup
description: Plan + verified findings for the two remaining zero-copy demos (Bevy, gpui) and the deferred cleanup, after six frameworks shipped 2026-06-01/02.
type: plan
date: 2026-06-02
---

# Bevy + gpui zero-copy demos (remaining) + deferred cleanup

**Status (2026-11-01):** Bevy shipped (`demo-servo-bevy`, now on stable Bevy
0.19.0, RenderSet ordering `.after(PrepareAssets).before(Queue)` and
`Rgba8Unorm` placeholder confirmed in `demo-servo-bevy/src/main.rs`). GPUI
zero-copy is the remaining goal: Mark, 2026-11-01, "Zero copy should remain the
goal." The current GPUI demo's CPU readback is the interim path, not the
intended design, so `README.md` and `docs/testing.md` calling it intentional are
stale against this ruling. Upstream Zed `main` still leaves the wgpu `Surfaces`
draw empty (`crates/gpui_wgpu/src/wgpu_renderer.rs`, rechecked 2026-11-01), so
the five-seam patch below still stands on Linux.

**Correction and rulings (2026-11-01):** GPUI renders with DirectX 11 on
Windows, not `gpui_wgpu`: the vendored `gpui_windows` uses
`directx_renderer.rs` and references no wgpu, as do Zed, Glass-HQ, gpui-ce and
gpui-pre. Seams 4 and 5 below (the wgpu surfaces draw and device accessor)
therefore help only on Linux; Windows zero-copy means importing Servo's shared
handle into GPUI's D3D11 device. Glass-HQ/gpui has had no commit since
2026-05-16.

- **D1, the GPUI base is gpui-pre (2026-11-01):** Mark chose "gpui-pre" over
  gpui-ce and gpui-unofficial. gpui-pre is the weekly Zed snapshot on crates.io
  (0.3.10, wgpu 30) that longbridge's gpui-component builds on. It carries no
  external-texture support, so every zero-copy seam stays ours.

  Links: area demos · plans bevy_gpui_zero_copy · summary Replace the vendored Glass-HQ GPUI with gpui-pre.

- **D2, Linux first, then D3D11 (2026-11-01):** Mark: "Linux first, then D3D11".
  Swap the base, land Linux zero-copy on the wgpu path, then add Windows D3D11
  shared-handle import as the next slice. How to do it is open to any fork's
  approach (Mark: "We can look at gpui-fast for how, or others. More than just
  those three choices"). References found 2026-11-01, not exhaustive:
  longbridge's gpui-fast (native window composition on Windows, macOS and Linux
  after Zed PR #62379; its `compat/gpui-pre*` crates can stand in for gpui-pre);
  gpui-ce's Linux `SurfaceSource::Texture`
  (`crates/gpui_wgpu/src/wgpu_renderer/surfaces/texture.rs`) with a shared
  device/queue handle; xgpui's Windows `custom_draw/dxgi.rs` (keyed-mutex NT
  handle import) and Linux `custom_draw/dma_buf.rs`; `gpui_interop` 0.2.0 on
  bite-gpui (Linux dma-buf with sync_file, Windows D3D12 to D3D11 shared
  texture); the closed Zed PR #60573 (external compositor for wgpu textures) and
  the forks in Zed discussion #60572; gpui-fusion.

  Links: area demos · plans bevy_gpui_zero_copy · summary Linux wgpu zero-copy lands before the Windows D3D11 import.

- **D3, port bite-gpui's imported-texture path onto gpui-pre (2026-11-09):**
  Mark kept gpui-pre as the base because it is closest to upstream Zed, and
  chose to record a hand port of bite-gpui's imported-texture work as the
  route. bite-gpui's per-backend draw code is the source for all three
  platforms; its core plumbing is rewritten against gpui-pre's layout, and
  its crate restructure is not taken. gpui-ce stays a reference for the Linux
  API shape. Evidence is in the fork survey below.

  Links: area demos · amends bevy_gpui_zero_copy D2 · plans bevy_gpui_zero_copy · summary Port bite-gpui's imported-texture path onto gpui-pre instead of designing the five seams from scratch.

- **D4, run the demo on bite-gpui as published (2026-11-09):** Mark ruled out
  carrying a GPUI patch ("we didn't want to get into a 'maintain a gui toolkit
  for a demo' stance") and chose bite-gpui itself: "let's just use
  bite-gpui", with the demo README recording why upstream GPUI could not do GPU
  import and every variant surveyed. The demo depends on bite-gpui 1.25.203
  from crates.io with no patches. macOS imports through a wgpu device on
  GPUI's `MTLDevice` and an sRGB Metal view; Linux (wgpu) and Windows
  (Direct3D 11) stay on readback until they are wired and checked on those
  machines. The Glass-HQ vendored-patch plan below is retired.

  Links: area demos · replaces bevy_gpui_zero_copy D3 · amends bevy_gpui_zero_copy D1 · plans bevy_gpui_zero_copy · summary The GPUI demo runs on unpatched bite-gpui, GPU import on macOS first.

## GPUI Linux and Windows import: handoff (2026-11-09)

**Update:** Linux landed the same day. It was built and checked on screen on
thinkpad-l14-f (RADV, GNOME Wayland), along the lines below. Windows
remains.

macOS import is on main (`b944556`) and was checked on screen on the M4
iMac: the status bar reads GPU import and the fixture animates. Linux and
Windows still read back. They need a machine on the mesh (o-pc or
thinkpad-l14-f, Mark's choice), because the repository has no CI since
`0743b3b` and the M4 iMac is on another network. Findings for whoever wires
them in `demo-servo-gpui/src/main.rs`'s `Pipeline`:

- **Device access.** bite-gpui lends its device only inside a `gpu_canvas`
  paint: `canvas.try_device::<R>()`, with `R = gpui_wgpu::WgpuRenderer`
  (crate `bite-gp-wgpu`) on Linux and `gpui_windows::DirectXRenderer` on
  Windows. Servo's rendering context is fixed when the webview is built, so
  the demo has to paint one frame first: a small root view renders a
  `gpu_canvas` that stores the device in a shared slot, then builds
  `ServoView` with it on the next render. macOS needs none of this.
- **Linux.** The device is `(Arc<wgpu::Device>, Arc<wgpu::Queue>)`, wgpu 30.
  Build `ServoWgpuInteropAdapter::new` on clones of them. The GL import
  needs a Vulkan device with `VK_KHR_external_memory_fd`, as the winit demo's
  plain device has; keep the readback fallback for anything else. bite's
  `ImportedTextureExt` checks the *texture* format is `Rgba8UnormSrgb` or
  `Bgra8UnormSrgb`, and grafting returns `Rgba8Unorm` with empty
  `view_formats`, so copy each frame into a reused `Rgba8UnormSrgb`
  texture (`copy_texture_to_texture` allows the sRGB pair). Same queue as
  GPUI, so no wait. Alternatively add an sRGB output option to grafting's
  normalizer and skip the copy.
- **Windows.** The frame comes from `current_dx12_shared_texture()`, which
  needs a wgpu DX12 device (the export is initialised with one), so build
  the adapter on a LUID-matched DX12 device as Bevy and Iced do. The shared
  texture is `R8G8B8A8_UNORM` with a bottom-left origin and no fence
  (`SyncMechanism::None`). Open its NT handle on GPUI's `ID3D11Device` with
  `ID3D11Device1::OpenSharedResource1`. bite's `DirectXTextureExt` accepts
  only `B8G8R8A8_UNORM(_SRGB)` and samples straight through, so an RGBA
  texture shows red and blue swapped without any error. Either a small
  swizzle pass on GPUI's device into a reused BGRA texture, or a BGRA option
  on grafting's D3D11 export. Pass `flip_v = true` for the bottom-left
  origin, and check orientation and colour on screen.

## Fork survey (2026-11-09)

Sources: the gpui-archipelago fork map (`gpui-archipelago/map-data`, measured
public API of the crates.io GPUI forks, data synced 2026-11-05), each fork's
GitHub tree, and a blobless fetch of Zed, bite-gpui and gpui-ce to compare
history. Nothing below was built.

| Fork | External texture import |
|---|---|
| bite-gpui (crates.io 1.25.203) | All three native renderers: `ImportedTextureHandle` + `GpuCanvasContext::paint_texture`, with backends for Metal (`MTLTexture`), wgpu (`wgpu::TextureView`) and D3D11 (`ID3D11Texture2D`); dma-buf handles with release fences; device accessor. |
| gpui-ce (crates.io 0.3.0) | `SurfaceSource::Texture` (`wgpu::Texture`) and `Window::gpu_context` on Linux/FreeBSD, and macOS behind `custom-gpu`. No Windows path. |
| xgpui (gpui-luma-dev, git only) | `custom_draw` on Metal (with a CoreVideo path) and wgpu. The Windows `dxgi.rs` cited under D2 no longer appears in its tree. |
| gpui-fusion (cradiy, git only) | Linux dma-buf, aimed at GStreamer media. |
| gpui, gpui-pre, gpui-unofficial, gpui-box, gpui-fast | None beyond the macOS-only YUV `paint_surface`. |
| gpui-kit, gpui-mobile, sonorahq/gpui, Pulsar-Native, kael | None. gpui-kit is the component library (formerly gpui-component) on gpui-pre; kael is not a GPUI fork. |

**bite-gpui against gpui-pre.** bite-gpui is Zed `40180d9` (2026-09-30) plus
62 commits, with no Zed merges since; gpui-pre 0.3.11 is Zed `f816e1f`
(2026-11-01). About 36 of the 62 split GPUI into new crates (`gpui_types`,
`gpui_engine`, `gpui_authoring`, `gpui_runtime`, `gpui_parley` and others);
the whole fork diff is +37k/−14k over 398 files. The texture commits total
about 2.5k lines, but 21 of the 46 files they touch do not exist in gpui-pre,
so cherry-picking fails and the work is a port:

| bite-gpui commits | Size | Port to gpui-pre |
|---|---|---|
| Draw arms: `eae392f18f` (wgpu), `23f88260b5` (Metal), `a6601b26ba` (D3D11), Metal producer `f3c4ba70e2`, per-backend `imported_texture.rs` | +100 / +145 / +502 / +255 | Near-direct: those backend crates exist in gpui-pre. Zed churn since bite-gpui's base is 6 commits on `gpui_wgpu`, 12 on `gpui_apple`, 2 on `directx_renderer.rs`. |
| Custom-render primitive, texture token, window call `be8418bc88` | +593 | Rewrite against `gpui/src/scene.rs` (3 upstream commits since) and `gpui/src/window.rs` (23). |
| Device accessor `99997237d1` | +120 over 13 files | Rewrite. |
| Crate split and renderer seam (`200d24145c`…`ebc105b2c2`) | most of the fork | Skip. |

Estimate: a 1.5–2k-line patch over gpui-pre covering Linux, macOS and
Windows, which supersedes the five-seam Linux-only design below.

**gpui-ce against gpui-pre.** Last shared Zed commit `60f4aa3`
(2025-12-12); about 2,550 own commits against about 7,300 Zed commits since,
with a different crate layout (`gpui_render`, `gpui_elements` and others).
Its texture path came from #39 (Linux wgpu `Surface`, +304), #237 (custom GPU
API, +856) and #215 (macOS onto wgpu, +17k/−15k). #39 and the useful part of
#237 port to gpui-pre's Linux `gpui_wgpu` path; the macOS half needs #215 and
does not port; Windows has nothing to port.

## Status — seven frameworks shipped (confirmed by Mark)

All zero-copy Servo → host-framework, confirmed on the Windows multi-GPU laptop:

| Demo | Host wgpu | Import path | Notes |
|---|---|---|---|
| `demo-servo-winit` | 29 | in-process GL import | reference; resize fixed (sole driver = `webview.resize`) |
| `demo-servo-egui` (new) | 29 | in-process import | eframe forced DX12; `register_native_texture` |
| `demo-servo-iced` (new) | 28 (git rev) | **shared-handle** | `shader` widget Primitive is `Send`; iced 0.15-dev |
| `demo-servo-blitz` (new) | 29 | in-process import | anyrender_vello `try_register_custom_resource` → vello `register_texture` |
| `demo-servo-slint` (new) | 28 | in-process import | slint 1.16 `unstable-wgpu-28`; notifier + `Image::try_from` |
| `demo-servo-bevy` (new) | 29 | **shared-handle** | bevy 0.19.0-rc.2; render world on its own thread; import then `copy_texture_to_texture` into a Bevy-owned `GpuImage` (resize-safe); toolchain bumped to 1.95 |

gpui is the eighth and only remaining framework. It is **not** shipped: zero-copy needs a from-scratch render path in the engine (see the gpui section below). The other seven are committed on `main`.

Plus the grafting core work: the multi-GPU **flicker fix** (`select_adapter_matching_surfman_luid` via `new_for_device`), single canonical normalize-flip, the **shared-handle seam** (`grafting::import_dx12_shared_texture` + `SurfmanFrameProducer::export_dx12_shared_texture` + `ServoWgpuRenderingContext::current_dx12_shared_texture`), and normalizer `COPY_SRC`.

Screenshots: each demo's `screenshots/<name>.png` (Mark saving them; READMEs already reference them).

## Verified version landscape (2026-06-02)

- **Bevy:** 0.18.1 stable = wgpu **27**; 0.19.0-rc.2 = wgpu **29.0.3**; `main` (0.19.0-dev) = wgpu **29.0.3**. → Use **0.19.0-rc.2** (no new grafting version; published crates.io release). No newer wgpu on dev.
- **gpui:** the gpui demo composites on **wgpu 29** via the `gpui_wgpu` crate. Upstream is now verified (2026-06-02): `zed-industries/zed` `main` itself carries the multi-crate split (`gpui`, `gpui_wgpu`, `gpui_linux`, `gpui_macos`, `gpui_windows`, `gpui_web`, `gpui_platform`) and a wgpu 29 backend. The fork is **`Glass-HQ/gpui`** `main` (pushed 2026-05-16), which tracks zed's structure and lags it slightly. The vendored copy at `patches/glass-gpui/` matches both on the surface path. (The earlier "zed = blade only" note was stale; zed mainline has `gpui_wgpu` as of this check.)
- grafting carries wgpu **28 + 29**. iced/slint use 28; winit/egui/blitz/bevy/gpui use 29.
- Note: mixed wgpu versions in one workspace can't share a single `cargo build --workspace`; build demos individually.

## Bevy demo plan (`demo-servo-bevy`)

Bevy 0.19.0-rc.2, wgpu 29. The render world runs on a separate thread (pipelined rendering), so Servo (`!Send`, main world) and `RenderDevice` (render world) are separated → use the **shared-handle seam** (the same reason iced needs it).

Verified API (bevy_render 0.19.0-rc.2):
- `GpuImage { texture, texture_view, sampler, texture_descriptor: TextureDescriptor<Option<&'static str>, &'static [TextureFormat]>, texture_view_descriptor: Option<...>, had_data: bool }` (`src/texture/gpu_image.rs`).
- `RenderDevice::wgpu_device() -> &wgpu::Device` (`src/renderer/render_device.rs:259`); `RenderQueue` derefs to `wgpu::Queue`.
- `RenderAssets<GpuImage>::insert(id: impl Into<AssetId<Image>>, GpuImage)` (`src/render_asset.rs:224`).

Approach:
1. Main world: throwaway HighPerformance-DX12 wgpu device → `new_for_device` anchors surfman to that GPU; force Bevy to DX12+HighPerformance via `WgpuSettings` (in `RenderPlugin`). Servo as a `NonSend` resource.
2. Main-world system: paint Servo → `current_dx12_shared_texture()` → `SharedFrame { handle: u64, w, h }` resource.
3. Extract `SharedFrame` into the render world (`ExtractSchedule`).
4. Render-world system: `import_dx12_shared_texture(&frame, &HostWgpuContext::new(device.wgpu_device().clone(), (**queue).clone()))` → build `GpuImage` → `RenderAssets::<GpuImage>::insert(handle_id, gpu_image)`.
5. Main world: a fullscreen `Sprite`/UI node on a `Handle<Image>` placeholder (size/format only, `RENDER_WORLD` usage). 2D camera.

### Verified Bevy 0.19.0-rc.2 API (read from source 2026-06-02)

- `bevy::render::settings`: `WgpuSettings { backends: Option<Backends>, power_preference, device_label, features, limits, .. }` (Default); `RenderCreation::Automatic(WgpuSettings)`; `RenderPlugin { render_creation: RenderCreation, .. }`. Force DX12+HP: `DefaultPlugins.set(RenderPlugin { render_creation: RenderCreation::Automatic(WgpuSettings { backends: Some(Backends::DX12), power_preference: PowerPreference::HighPerformance, ..default() }), ..default() })`.
- `bevy::render::{RenderApp, Render, RenderSet, ExtractSchedule, Extract}` (lib.rs). `RenderApp` is the sub-app label: `app.sub_app_mut(RenderApp)`. `Extract<'w,'s, P>` is the system param to read main-world data in an `ExtractSchedule` system.
- `RenderDevice::wgpu_device() -> &wgpu::Device` (renderer/render_device.rs:259).
- `RenderQueue(pub Arc<WgpuWrapper<Queue>>)` with `Deref`/`DerefMut` (renderer/mod.rs:124) → get the wgpu queue via `(**render_queue.0).clone()` (Arc→WgpuWrapper→Queue; `wgpu::Queue: Clone`).
- `GpuImage { texture, texture_view, sampler, texture_descriptor: wgpu_types::TextureDescriptor<Option<&'static str>, &'static [TextureFormat]>, texture_view_descriptor: Option<...>, had_data: bool }`. Build: `texture.create_view(&Default::default())` for the view; sampler from `Res<DefaultImageSampler>` (`(***default_sampler).clone()`); `texture_descriptor` = a `TextureDescriptor { label: None, size, mip_level_count:1, sample_count:1, dimension: D2, format: Rgba8Unorm, usage: TEXTURE_BINDING|COPY_DST, view_formats: &[] }`.
- `RenderAssets::<GpuImage>::insert(id: impl Into<AssetId<Image>>, GpuImage)` (render_asset.rs:224). Key it by the placeholder handle's `AssetId`.
- `bevy_image::Image::new_uninit(size: Extent3d, dimension: TextureDimension, format: TextureFormat, asset_usage: RenderAssetUsages)` for the placeholder; use `RenderAssetUsages::RENDER_WORLD`. Sprite: `Sprite::from_image(handle)`; 2D camera: `Camera2d` (prelude).

### Wiring (to finalize against the build)

- Main world: `WgpuSettings` DX12; throwaway HP-DX12 device → `new_for_device` anchors surfman; Servo as `NonSendMut`/`NonSend` resource; `Assets<Image>` placeholder (`new_uninit`, RENDER_WORLD) → `Handle<Image>` on a `Sprite` + `Camera2d`. Store the handle in a `Resource`.
- Main-world `Update` system (`NonSend`): paint Servo → `current_dx12_shared_texture()` → write a `SharedFrame { handle:u64, w,h }` resource.
- `app.sub_app_mut(RenderApp)`: add an `ExtractSchedule` system using `Extract<Res<SharedFrame>>` + `Extract<Res<ServoImageHandle>>` to copy them into the render world; add the inject system to `Render` in `RenderSet::Prepare` (after `RenderSet::PrepareAssets`) that imports via `grafting::import_dx12_shared_texture` and `RenderAssets::<GpuImage>::insert`.
- Open: confirm exact `RenderSet` variant ordering (PrepareAssets → Prepare); whether to cache the imported texture per-size to avoid per-frame `OpenSharedHandle`; placeholder image format must match the imported texture (`Rgba8Unorm`, top-left from the default normalize path).

Recreate `demo-servo-bevy/` with the Cargo.toml below (scaffold + Cargo.toml committed as WIP head-start).

```toml
bevy = { version = "0.19.0-rc.2", default-features = false, features = [
    "bevy_winit", "bevy_core_pipeline", "bevy_sprite", "bevy_window", "bevy_asset", "x11" ] }
grafting = { path = "../grafting" }                 # default wgpu-29
servo-wgpu-interop-adapter = { path = "...", features = ["servo"] }   # default wgpu-29
# + servo (git release/v0.2), demo-support, euclid, url, winit, wgpu 29, pollster, rustls
```

## gpui demo plan (`demo-servo-gpui` — update)

Existing demo = CPU readback (`CapturingRenderingContext` → BGRA `RenderImage`, driven by `request_animation_frame`; Servo `!Send` on the main thread). gpui's UI renderer is single-threaded, so zero-copy can use **in-process import** (like Blitz/Slint), not the shared handle.

### Upstream verification (2026-06-02, both repos read at `main`)

Checked `zed-industries/zed` (`main`, pushed 2026-06-02) and `Glass-HQ/gpui` (`main`, pushed 2026-05-16). Findings are identical on the surface path; the vendored copy is current. There is **no** existing way to render an external `wgpu::Texture` into a gpui scene:

1. The surface fragment shader is **YUV-video-only**: `fs_surface` samples `t_y` + `t_cb_cr` (NV12 planes), not RGBA. zed `shaders.wgsl:1352`, Glass-HQ `:1323`.
2. The whole surface element is **macOS-gated**: `SurfaceSource` has only `#[cfg(target_os="macos")] Surface(CVPixelBuffer)`; `surface()`, `window.paint_surface`, and `PaintSurface` (one macOS field `image_buffer: CVPixelBuffer`) are all `#[cfg(target_os="macos")]`.
3. The wgpu **surfaces draw is a no-op stub**: `PrimitiveBatch::Surfaces` in `gpui_wgpu/src/wgpu_renderer.rs` returns `true` with the comment "Surfaces are macOS-only for video playback / Not implemented for Linux/wgpu". zed `:1303`, Glass-HQ `:1257`. The pipeline + bind-group-layout + `SurfaceParams` uniform exist (built around the YUV shader), but nothing draws.
4. **No alternative GPU entry point.** `ImageSource` is `Resource | Render | Image | Custom`, all resolving to a CPU `RenderImage`; `canvas()` is a CPU paint-instruction callback. Code search for `register_native_texture` / "external texture" is empty.
5. **No work in flight.** Zero open PRs for a wgpu surface/external-texture path; the last 30 `gpui_wgpu` commits are text/atlas/device-recovery/backend-selection only.

Two bonuses found while verifying:
- `WgpuContext` (`gpui_wgpu/src/wgpu_context.rs:9-13`) already exposes `pub device: Arc<wgpu::Device>` and `pub queue: Arc<wgpu::Queue>`. The field is public; only an app-facing accessor chain (window → renderer → context) is missing.
- `CompositorGpuHint { device_id: u32 }` exists in the same file. It may be a cleaner way to make gpui pick the surfman/ANGLE-matched GPU than the throwaway-DX12 LUID-anchor used for iced/Bevy. Probe it before falling back to the anchor.

### Patch design (from-scratch RGBA external-texture path)

The vendored patch (authorized) lives in `patches/glass-gpui`. It is the largest of the eight because it adds a render path the engine does not have. Keep it upstreamable: implementing the wgpu surfaces draw is a gap zed itself flags.

Five seams to cut, in this order:

1. **gpui core, renderer-agnostic handle.** gpui core has no wgpu dep, so the texture cannot be a `wgpu::Texture` there. Carry it as `Arc<dyn Any + Send + Sync>`. Add a non-macOS field to `PaintSurface` (`crates/gpui/src/scene.rs:715`) or a sibling primitive holding `{ bounds, content_mask, texture: Arc<dyn Any + Send + Sync> }`. Extending `PaintSurface` reuses the existing `Primitive::Surface` / `PrimitiveBatch::Surfaces` batching, which is the smaller change.
2. **paint method.** Add a non-macOS `window.paint_external_texture(bounds, Arc<dyn Any + Send + Sync>)` (mirror `paint_surface` at `crates/gpui/src/window.rs:4336`) that inserts the primitive.
3. **surface element, Windows variant.** `crates/gpui/src/elements/surface.rs`: add a cross-platform `SurfaceSource::Texture(Arc<dyn Any + Send + Sync>)`, ungate `surface()` for Windows, and add a `Surface::paint` arm routing it to `paint_external_texture`.
4. **gpui_wgpu, RGBA draw.** `crates/gpui_wgpu/src/shaders.wgsl`: add `fs_surface_rgba` that samples a single RGBA `texture_2d<f32>` (reuse `vs_surface` + `SurfaceParams` unchanged). Add a second pipeline (`surfaces_rgba`) using it. Implement `PrimitiveBatch::Surfaces`: downcast the handle to `wgpu::Texture`, make a bind group (texture view + sampler + `SurfaceParams` from bounds/content_mask), draw the quad.
5. **device accessor.** Expose gpui's `WgpuContext.device`/`.queue` to app code through a window/renderer accessor so the demo can `ServoWgpuInteropAdapter::new_for_device(gpui_device)` (LUID-match) and import each frame in-process.

### Demo wiring (`demo-servo-gpui`)

- On startup, reach gpui's wgpu device via the new accessor; build the Servo adapter with `new_for_device` (or set `CompositorGpuHint` if that path works) so surfman/ANGLE anchors to gpui's GPU.
- Each frame (`request_animation_frame`, main thread): paint Servo, `import_current_frame_default()` onto gpui's device to get a `wgpu::Texture`, wrap it `Arc::new(texture) as Arc<dyn Any + Send + Sync>`, and render `surface(SurfaceSource::Texture(arc))` sized to the view. Keep the URL bar + input forwarding from the current demo.
- Format/orientation: the default normalize path is `Rgba8Unorm`, top-left, so `fs_surface_rgba` samples straight (no V-flip, unlike the bottom-left shared-handle export used by Bevy).

### Cost + risk

Multi-file change across gpui core (scene/window/surface) and gpui_wgpu (shader/pipeline/draw/accessor), plus a multi-minute glass-gpui rebuild per iteration. Run it as its own session so a half-finished engine patch never lands on the seven shipped demos. The seven are committed; gpui starts from a clean tree.

## Deferred cleanup (do alongside / after)

1. **iced cpu-readback feature** — re-add the feature-gated CPU readback path to `demo-servo-iced` (dropped to ship zero-copy first).
2. ~~**Dead `ImportingRenderingContext`**~~ — obsolete (2026-11-01): the type is live in `grafting-servo` (formerly `servo-wgpu-interop-adapter`), restored for pre-present import in `02ddc11`. Do not remove it.
3. **Minimal sync set review** — with LUID-match as the real flicker fix, re-check whether `present()`'s `PreserveBuffer::Yes` + `glFinish` and the normalizer copy are all still required, or if the set can shrink. Verify on winit + egui after any change.
4. **Screenshots into READMEs** — Mark saving `screenshots/<name>.png` per demo; main README references them.
5. **Main README** — done: the demo list and build-individually note are in `README.md`; the per-demo render-path table is in `docs/testing.md`.
