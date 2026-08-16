# Changelog

All notable changes to this project will be documented here.

## [Unreleased]

Targets `grafting` 0.5.0, not 0.4.1: surfman is a public dependency of the
`gl` / `surfman` paths, and its major moved. The version is bumped here rather
than at publish time so `main` never claims to be the 0.4.0 that is already on
crates.io, which has caught this repo before.

### Servo

- **Tracking Servo `release/v0.4`** (tag v0.4.0, 4 August 2026), up from
  `release/v0.3`. All nine manifests move together.
- **surfman 0.12 -> 0.13** in `grafting` and `servo-wgpu-interop-adapter`,
  because Servo 0.4 moved to surfman 0.13. Both sides have to name the same
  surfman or the GL producer types stop unifying and every method of the
  `RenderingContext` impl reads as a type mismatch (24 errors, all of them the
  same version split). glow stays at 0.17, which Servo 0.4 also uses.
- **`primeorder` pinned to 0.14.0-rc.14 in the lockfile.** Servo 0.4 pulls
  p256/p384/p521 0.14.0-rc.14, which ask for `primeorder = "0.14.0-rc.14"`.
  Cargo reads that as allowing the final 0.14.0, and 0.14.0 added a `WnafSize`
  bound the release candidates do not satisfy, so a freshly resolved lockfile
  fails to compile before it reaches Servo. Anyone resolving Servo 0.4 from
  scratch hits this; it is not specific to this repo.

### Added

- **`wgpu-30` feature row** in grafting and the adapter, carrying wgpu 30
  alongside 29 and 28. The newest enabled version wins. wgpu-29 stays the
  default while the GUI ecosystem straddles majors: eframe 0.36 is on wgpu 30,
  bevy 0.19 and slint 1.17 on 29, iced git (0.15-dev) on 28. The only
  signature the majors disagree on is `Device::create_texture_from_hal`
  (wgpu 30 adds an explicit tracker `initial_state`; 28/29 hardcode
  `UNINITIALIZED`). Every import routes through one compatibility helper, but
  wgpu 30's state is chosen at the import seam: Metal frames enter for shader
  reads, DX12 shared resources enter in COMMON, and locally created Vulkan
  images enter in UNDEFINED layout. That preserves the older rows' behavior
  without claiming one initial state fits every backend.

### GUI framework updates

- bevy `0.19.0-rc.2` -> `0.19.0` stable (still wgpu 29).
- eframe `0.34` -> `0.35` (wgpu 29). Not 0.36: the iced demo's git iced pins
  `web-sys = "=0.3.85"`, eframe 0.36 needs js-sys ^0.3.103, and one workspace
  lockfile cannot hold both 0.3.x resolutions. Hosts on eframe 0.36 use
  grafting's `wgpu-30` feature; the demo follows when iced lifts its pin.
- slint `1.16` -> `1.17`, and the slint demo moves from `unstable-wgpu-28` to
  `unstable-wgpu-29`, riding the workspace default instead of dragging a second
  wgpu major. The iced demo (git 0.15-dev) is now the only wgpu-28 consumer.
- winit `0.30.12` -> `0.30.13` across all demos.

- `grafting::wgpu` and `grafting::wgpu_hal` re-export the feature-selected pair,
  so a consumer can name exactly the wgpu that grafting was built against
  instead of depending on `wgpu` separately and risking a different major. An
  imported texture only works on a device from the matching one.

### Fixed

- `wgpu-28` on macOS now converts wgpu-hal 28's `metal-rs` device and
  texture handles at the HAL boundary. The earlier 28/29/30 feature matrix
  treated Metal 28 like 29, even though wgpu-hal did not move to
  `objc2-metal` until 29, so the advertised macOS 28 row did not compile.
- `vulkan_dmabuf::create_dmabuf_host_context` under `wgpu-28` on Linux never
  compiled: it called hal 29's four-argument `open_with_callback` (hal 28 has
  no `limits` parameter). No CI config ever combined wgpu-28 with the
  Linux-only module, so the 0.5.0 cross-matrix (three targets x three wgpu
  rows) is what surfaced it.
- `cargo test -p grafting` on Linux. `tests/dmabuf_roundtrip.rs` wrote `wgpu::`,
  but an integration test is its own crate and cannot see the crate-internal
  `extern crate wgpu_29 as wgpu` alias, and grafting has no dependency literally
  named `wgpu`. It now goes through the re-export above. The test is
  `#![cfg(target_os = "linux")]`, so only the Linux job could see the break;
  `cargo check -p grafting --target x86_64-unknown-linux-gnu --tests` reproduces
  it from any host.
- `cargo check -p demo-raw-gl`, which asked for grafting with
  `default-features = false` and no `wgpu-*` feature, so grafting's own feature
  guard rejected it. It only ever built as part of a whole-workspace build,
  where another member turned `wgpu-29` on; the per-package check CI runs gets
  no such unification. Its glow pin also lagged grafting's at 0.16 while
  `RawGlFrameProducer::new` takes grafting's `Arc<glow::Context>`.
- `cargo doc` under `-D warnings`. It documented with `--no-default-features`
  on all three platforms, but `raw_gl`, `surfman_gl`, and `vulkan_dmabuf` are
  feature- and platform-gated, so intra-doc links into them could not resolve
  in that configuration. Docs are now checked once, on Linux with default
  features, which is how docs.rs builds the crate and the only configuration
  where all three modules exist. Three link defects behind that failure are
  fixed: a public item linking to a private one, a link to the Windows-only
  `Dx12FenceSynchronizer`, and a redundant explicit link target.

### Branch automation

- The three sync workflows now share `sync-servo-line.yml` instead of each
  carrying its own copy of the same logic. Every scheduled run had failed since
  8 May 2026: the inline pin rewriters only matched the bare `servo = "X.Y.Z"`
  form, so `servo-wgpu-interop-adapter`'s table-form pin kept its old version
  while the demos moved, and two Servo versions in one graph means two Stylos
  competing for `links = "servo_style_crate"`. The rewrite now lives in
  `scripts/set_servo_pin.py`, handles every pin shape, discovers manifests by
  glob rather than a hardcoded list that never grew past the first five crates,
  and exits non-zero rather than reporting success after changing nothing.
- Line selection moved from the GitHub releases API to `git ls-remote`
  (`scripts/servo_lines.py`), so it reads branches and tags rather than how a
  release happens to be titled, and needs no token.
- `main` now tracks the newest tagged Servo release line. The previous
  "Sync Main To Servo LTS Release" drove `main` back to the v0.1.x LTS line,
  which current adapter code cannot compile against, so it failed in validation
  every week even when its pin rewriting was not the problem.
- Sync runs refresh the lockfile with `cargo metadata` rather than
  `cargo update`, which re-resolved every dependency to its newest match and
  invited exactly the kind of unrelated breakage the `primeorder` pin above
  documents. They also install Servo's Linux build deps and cache the cargo
  registry, neither of which they did before.

### Demo changes

- All five demos now build and run on Linux (Fedora 44 / Mesa-RADV / Vulkan
  verified). `demo-raw-gl`, `demo-servo-winit`, `demo-servo-iced`, and
  `demo-servo-xilem` run directly; `demo-servo-gpui` works via the gpui
  migration below.
- `demo-servo-gpui`: migrated off Zed's published `gpui 0.2.2` (blade
  renderer → naga 25, which collided with wgpu 29's naga 29) onto the
  glass-hq/gpui fork, which renders through `gpui_wgpu` (wgpu 29) — no blade,
  no naga conflict. Construction now goes through `gpui_platform::application()`
  and `FocusHandle::focus` takes `(window, cx)`.

### Patches

- `patches/glass-gpui`: vendored copy of glass-hq/gpui (a wgpu-based,
  Zed-tracking gpui fork) with two Linux build fixes that the fork's
  "extract platform crates" refactor regressed:
  - workspace `ashpd` pin bumped `0.12.1` → `0.13` (the `gpui_linux` code
    already uses the 0.13 API — `ashpd::Uri`, etc. — and 0.13 renamed the
    runtime feature `async-std` → `async-io` and gates portals behind
    per-portal features, so `file_chooser`/`open_uri`/`settings` are enabled
    explicitly);
  - re-added `gpui::layer_shell::LayerShellNotSupportedError`, a 5-line
    `thiserror` struct that the extraction dropped. Both are candidates to
    upstream to glass-hq.
- Removed the previous `patches/gpui` (stale Zed 0.2.2 + blade vendor),
  superseded by `patches/glass-gpui`.

## [grafting 0.4.0]

Everything below landed on `main` after 0.3.0 was published, so the crates.io
0.3.0 and the repo diverged. This release closes that gap.

### Fixed

- **macOS built for the first time since the wgpu-hal 29 bump.**
  `metal_texture_ref` and `raw_gl::metal` still handed `metal` crate types to
  `wgpu_hal::metal::Device::texture_from_raw`, which has taken
  `Retained<ProtocolObject<dyn MTLTexture>>` and `objc2_metal::MTLTextureType`
  since the objc2 migration. Both now go through `objc2-metal`, and
  `raw_gl::metal` calls the typed
  `newTextureWithDescriptor:iosurface:plane:` instead of an untyped `msg_send!`.
  Verified with `cargo check --target aarch64-apple-darwin`, default features
  and `--no-default-features --features wgpu-29`.
- Along the way, the same call sites were passing `array_layers: 0`,
  `mip_levels: 0`, and `CopyExtent { depth: 0 }` for 2D textures. All three are
  now 1.

### Added

- `import_dx12_shared_texture`: the low-level `OpenSharedHandle` to wgpu step,
  exposed as a free function so a consumer can drive it without the high-level
  importer. `wgpu-weld` uses this.
- `Dx12SharedTexture` carries `producer_sync` and `fence_value`, the
  shared-handle sync seam, with a multi-GPU flicker fix.
- Epoch-keyed frame import cache.
- Linux DMABUF import loop closed; the full demo suite runs on Linux.

### Changed (breaking)

- wgpu is selected by feature rather than pinned: `wgpu-29` (default) or
  `wgpu-28`, so the crate builds against whichever major the host already uses.
  Exactly one must be enabled.
- The GL producer path is behind a new `gl` feature, and `surfman` implies it.
  A consumer that only wants the shared-texture import paths (DX12 / Metal /
  Vulkan DMABUF) can now take `default-features = false` plus a `wgpu-*`
  feature and skip glow and surfman entirely.

## [grafting 0.3.0]

### Renamed

- Crate renamed from `wgpu-native-texture-interop` to `grafting`. The
  prior name was published at 0.1.0 / 0.2.0; new releases ship as
  `grafting`. No migration shim — update imports from
  `wgpu_native_texture_interop::` to `grafting::`.

### Breaking

- `ImportOptions` is now `#[non_exhaustive]`. Construct via
  `ImportOptions::default()` and mutate fields, rather than struct-literal
  initialization, so future fields don't break callers
- Removed `ImportOptions::allow_copy_fallback` — it was documented as
  reserved-for-future-use and had no implementation. Will be re-added in
  a future release if/when a CPU fallback path lands
- `servo-wgpu-interop-adapter`: dropped the `InteropImportOptions` /
  `InteropImportedTexture` re-exports. Callers should `use
  grafting::{ImportOptions, ImportedTexture}` directly

### Internal

- Moved `MetalTextureRef` and `Dx12SharedTexture` unsafe import bodies out
  of `lib.rs` into new `metal_texture_ref` and `dx12_shared_texture`
  modules at crate root, mirroring the `vulkan_dmabuf` layout. Public API
  unchanged; `lib.rs` is now ~700 lines of types-and-traits

## [wgpu-native-texture-interop 0.2.0]

### Added

- `surfman_gl::windows_dx12_shared`: ANGLE D3D11 → wgpu DX12 zero-copy
  import path. Allocates an `ID3D11Texture2D` with
  `D3D11_RESOURCE_MISC_SHARED | D3D11_RESOURCE_MISC_SHARED_NTHANDLE` on
  ANGLE's own D3D11 device, wraps it as a transient EGL pbuffer surface
  for ANGLE/GL writes, and opens the same NT handle on the host wgpu
  DX12 device via `ID3D12Device::OpenSharedHandle`. Closes the gap
  where `raw_gl::dx12` could not service ANGLE-Servo (which lacks
  `GL_EXT_memory_object_win32`). Adapted from slint examples/servo
  (#11089). Size-dependent state is cached on `SurfmanFrameProducer`
  via `AngleDx12SharedCache` and reused across frames so the wgpu
  texture handle stays stable
- `surfman_gl::select_adapter_matching_surfman_luid`: Windows multi-GPU
  adapter selection helper that matches wgpu's adapter LUID to
  surfman's underlying D3D11 device LUID. On hosts with both an
  integrated and discrete GPU, wgpu's `request_adapter` and surfman's
  `Connection::create_adapter` may otherwise pick different drivers,
  silently breaking the shared-NT-handle interop. Adapted from slint
  examples/servo (#11439)
- `backend_name(&wgpu::Device) -> &'static str` and
  `print_wgpu_backend(&wgpu::Device)`: reports the active wgpu graphics
  backend in human-readable form for startup observability
- `Dx12FenceSynchronizer`: explicit `D3D12_FENCE_FLAG_SHARED` fence
  synchronizer for cross-API texture handoff. Creates a shared fence on
  the wgpu D3D12 device, exports an NT handle for D3D11/D3D12 producers,
  and queues `ID3D12CommandQueue::Wait` on the wgpu queue before each
  consumer submit
- `VulkanSemaphoreSynchronizer`: external `VkSemaphore` fd-based
  synchronizer for the WPE DMABUF protocol on Linux. Imports a per-frame
  semaphore fd into a persistent `VkSemaphore` with `TEMPORARY` flag and
  issues a standalone wait submit on the wgpu Vulkan queue
- `MetalSharedEventSynchronizer`: precautionary `MTLSharedEvent`
  synchronizer for Apple platforms; CPU-side wait via
  `waitUntilSignaledValue:timeoutMS:`. Not required for correctness on
  Apple silicon (IOSurface coherence is implicit) but provides the API
  anchor for a future GPU-side wait once `wgpu-hal::metal::Queue`
  exposes its raw `MTLCommandQueue`
- `VulkanExternalImage` import path: DMABUF→`VkImage`→`wgpu::Texture` via
  `VK_KHR_external_memory_fd` + `VK_EXT_image_drm_format_modifier`
  (Linux only). Replaces the prior
  `Unsupported(NativeImportNotYetImplemented)` arm with a real import
  for WPE-class DMABUF producers
- `vulkan_dmabuf::create_dmabuf_host_context`: constructs a wgpu device
  with `VK_EXT_image_drm_format_modifier` enabled on top of wgpu-hal's
  default extension set, then wraps it as a `HostWgpuContext`. Required
  for the `VulkanExternalImage` import path — wgpu's stock `Device` does
  not enable the extension, so passing a default-constructed wgpu device
  to `WgpuTextureImporter` would crash inside ash when the missing
  function pointer (`get_image_drm_format_modifier_properties_ext`)
  failed to load
- `HostWgpuContext::dmabuf_support`: bool field, set automatically by
  `HostWgpuContext::new` via runtime inspection of
  `wgpu_hal::vulkan::Device::enabled_device_extensions()`. Drives the
  capability matrix's `vulkan_external_image` reporting so it now
  reflects the actual device rather than just the platform
- `CapabilityMatrix::for_host(backend, dmabuf_support)`: capability shape
  for a specific host configuration, used by
  `HostWgpuContext::capabilities`
- `UnsupportedReason::VulkanDmabufExtensionNotEnabled`: returned by the
  capability matrix when the Vulkan device lacks
  `VK_EXT_image_drm_format_modifier`
- `grafting/tests/dmabuf_roundtrip.rs`: end-to-end integration test for
  the DMABUF import path. Allocates an exportable `VkImage` with
  `DRM_FORMAT_MOD_LINEAR`, clears it via `vkCmdClearColorImage`, exports
  the dmabuf fd, imports through `WgpuTextureImporter`, and asserts the
  imported texture's pixels match the clear color. Gated `#[ignore]`
  (run with `cargo test --test dmabuf_roundtrip -- --ignored`) because
  it requires `VK_EXT_image_drm_format_modifier` which not all CI VMs
  have
- `VulkanExternalImage` fields for DMABUF and semaphore handoff:
  `dmabuf_fd`, `dmabuf_offset`, `dmabuf_stride`, `drm_modifier`,
  `wait_semaphore_fd`

### Changed

- `CapabilityMatrix::vulkan_external_image`: now reports `Supported`
  on Linux + Vulkan host backend (was
  `Unsupported(NativeImportNotYetImplemented)`)
- `InteropBackend::Dx12` doc string updated to reflect that GL→DX12
  import is now supported on ANGLE-backed surfman via
  `surfman_gl::windows_dx12_shared`
- `vulkan_dmabuf::import_vulkan_external_image`: imported textures now
  include `COPY_SRC` (and `TRANSFER_SRC` on the underlying Vulkan image)
  in addition to `TEXTURE_BINDING`, so consumers can readback / debug
  imported frames without rebuilding through a render pass. No runtime
  cost — Vulkan and wgpu both treat extra usage flags as a no-op when
  unused
- `CapabilityMatrix::for_backend` on Linux + Vulkan now reports
  `vulkan_external_image: Unsupported(VulkanDmabufExtensionNotEnabled)`
  by default (was incorrectly reporting `Supported`). The accurate
  per-device shape is available via `HostWgpuContext::capabilities` /
  `CapabilityMatrix::for_host`
- Cargo features: added `Win32_Security` and `Win32_Graphics_Direct3D11`
  to the `windows` crate dep (required by the new shared-D3D11 path);
  added `sm-angle-default` to surfman (required for ANGLE-specific
  `Device::create_surface_texture_from_texture`); added `wio = "0.2"`
  for the surfman ANGLE method's `ComPtr` parameter; added `MTLEvent`
  to `objc2-metal` (required by `newSharedEvent`)
- Surfman rebind errors are now propagated through the Linux Vulkan,
  Windows Vulkan, Windows DX12, and Apple Metal import paths (was
  silently swallowed via `let _ = ...`). Both the import and rebind
  attempt run; whichever fails surfaces (preferring the import error
  if both fail). Adapted from slint examples/servo (#11497)

### Demo changes

- `demo-servo-winit`: switched the Windows wgpu instance from
  `VULKAN | DX12` to forcing DX12 by default so the new
  `surfman_gl::windows_dx12_shared` path is the exercised default.
  `WGPU_BACKEND=vulkan` still selects the legacy ANGLE-D3D11 KMT →
  Vulkan path. Calls `print_wgpu_backend` on startup.

### Added (workspace)

- `README.md`: documented the branch policy for `main`, `latest-release`,
  and `experimental`, and clarified that `main` targets Servo
  `v0.1.x` LTS
- `demo-servo-xilem`: Servo embedded in Xilem 0.4 with URL bar, CPU readback,
  and full input forwarding (mouse, scroll, keyboard)
- `demo-servo-iced`: Servo embedded in iced 0.14 with URL bar, CPU readback,
  flicker-free GPU upload via `image::allocate()`, and full input forwarding
- `demo-servo-gpui`: Servo embedded in GPUI 0.2 (Zed's framework) with URL bar,
  RGBA→BGRA conversion, `request_animation_frame()` render loop, and full input
  forwarding including custom key mapping
- `demo-servo-winit`: added mouse, scroll, and keyboard input forwarding to
  Servo; pages are now fully interactive (links, scrolling, text input)
- `rust-toolchain.toml`: pin workspace to Rust 1.92.0 (required by wgpu 29)
- `patches/glass-gpui` and `patches/taffy-0.9`: local GPUI compatibility
  patches so the demo can use the wgpu-based glass-hq GPUI fork while
  satisfying GPUI's exact taffy 0.9.0 dependency
- `patches/serde_fmt`: local serde_fmt fork removing ambiguous `From` impl
  that breaks stylo's `ToCss` derive on Rust 1.92
- `[patch.crates-io]` override for `glslopt` (webrender's bundled GLSL
  optimizer) to git 0.1.14, which adds `#ifndef __once_flag_defined`
  guards around its C11 threads polyfill. Required to build webrender
  (and therefore Servo) on glibc 2.34+ — i.e. Fedora 40+, Ubuntu 24.04+ —
  where `<stdlib.h>` now declares `once_flag` itself
- `grafting`: public API doc comments on all major types
  (`InteropBackend`, `CapabilityMatrix`, `NativeFrame`, `ImportOptions`, etc.)
- `grafting`: `#[non_exhaustive]` on `NativeFrame`,
  `NativeFrameKind`, `InteropBackend`, `SyncMechanism`, `InteropError`, and
  `UnsupportedReason` to protect downstream users from semver breaks
- `grafting`, `servo-wgpu-interop-adapter`: crate-level
  `#![doc = include_str!("../README.md")]` so docs.rs renders the README

### Fixed

- `raw_gl/linux.rs`, `raw_gl/windows.rs`: Vulkan memory allocation now
  correctly queries `get_physical_device_memory_properties` and selects a
  `DEVICE_LOCAL` memory type index compatible with the image's
  `memory_type_bits`, rather than unconditionally using index 0

## [0.1.0] — Initial release

- GL→wgpu texture interop for Linux/Android (Vulkan opaque FD) and Apple
  (IOSurface→Metal)
- Windows Vulkan path (opaque Win32 NT handle) — builds and runs; depends on
  driver support for `VK_KHR_external_memory_win32` under WGL/EGL
- `grafting`: core library with trait-based API
- `servo-wgpu-interop-adapter`: Servo `RenderingContext` integration
- `demo-raw-gl`: standalone glutin+glow FBO → wgpu demo (no Servo required)
- `demo-servo-winit`: full Servo + winit + wgpu reference application
