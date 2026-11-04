# grafting

Import native GPU textures (GL framebuffers, Vulkan images, Metal IOSurfaces) into host-owned `wgpu` textures. This is the core library of the [servo-wgpu-interop](../) workspace.

This crate is **framework-agnostic** — it has no Servo dependency and can be used by any application that needs to import externally-produced GPU content into wgpu.

## What it does

A "producer" (Servo, a GL renderer, a video decoder, etc.) renders into a native GPU resource. This crate imports that resource into a `wgpu::Texture` owned by the host application, enabling zero-copy compositing across API boundaries.

## Platform support

| Platform | Import path | Status |
| --- | --- | --- |
| Linux / Android | GL FBO → Vulkan external memory → wgpu | Implemented |
| macOS / iOS | IOSurface → Metal texture → wgpu | Implemented |
| Windows (Vulkan) | GL FBO → Vulkan image (NT handle) → wgpu | Implemented |
| Windows (DX12) | GL FBO → DX12 shared texture → wgpu | Hardware-verified through the Servo demos |

## Key types

- `HostWgpuContext` — wraps the host's `wgpu::Device` and `wgpu::Queue`
- `NativeFrame` — platform-specific frame produced by the offscreen renderer
- `ImportedTexture` — the result of importing a `NativeFrame` into wgpu
- `ImportOptions` — controls import behavior (format, usage flags)
- `CapabilityMatrix` — runtime query of what the current platform/driver supports
- `InteropBackend` — detected backend (Vulkan, Metal, DX12)
- `FrameProducer` / `TextureImporter` — traits for the produce/import pipeline
- `WgpuTextureImporter` — default `TextureImporter` implementation
- `InteropSynchronizer` — trait for cross-API synchronization policies

## Native resource ownership

`NativeFrame` is move-only when it carries a native resource. Safe imports
consume the frame, so Graft closes a Vulkan descriptor on failed import and
hands it to the driver only after successful import. Windows producer caches
reuse a cloneable `Dx12SharedResource` token, then create a fresh move-only
`Dx12SharedTexture` with its own metadata for each handoff. Metal's safe frame
takes a retained `MTLTexture`.

Raw native descriptors and borrowed Metal/DX12 handles are available only
through explicitly `unsafe` constructors or import functions. Their safety
documentation names the owner and required lifetime; use them only when an
integration cannot transfer custody to Graft.

## Modules

- `raw_gl` — surfman-independent GL import functions. Use `RawGlFrameProducer` for any GL application without bringing surfman as a dependency (set `default-features = false`).
- `surfman_gl` — surfman-backed frame producer (enabled by default via the `surfman` feature).
- `vulkan_dmabuf` — owned DMABUF import, including explicit DRM modifiers,
  same-buffer multi-plane layouts, and foreign-queue acquisition (Linux). Use
  `create_dmabuf_host_context` when constructing the app's unified wgpu device;
  a default device does not enable `VK_EXT_queue_family_foreign`.

## Usage

For Servo embedding, pair this crate with [`grafting-servo`](../servo-wgpu-interop-adapter/) which handles Servo-specific setup. For standalone GL import, see [`demo-raw-gl`](../demo-raw-gl/) which uses the `raw_gl` module directly.

Pick exactly one `wgpu-*` feature; it must match the wgpu your host already
uses, or the imported texture will not share a device.

```toml
[dependencies]
# Default: wgpu 29, plus the surfman GL producer path.
grafting = "0.6.0"

# Same, against wgpu 28.
grafting = { version = "0.6.1", default-features = false, features = ["wgpu-28", "surfman"] }

# Shared-texture import only (DX12 / Metal / Vulkan DMABUF). No GL, no
# surfman, no glow. This is what wgpu-weld takes.
grafting = { version = "0.6.1", default-features = false, features = ["wgpu-29"] }

# Same shared-texture surface against wgpu 30.
grafting = { version = "0.6.1", default-features = false, features = ["wgpu-30"] }
```

## Completed Linux native snapshots

With `surfman` enabled, `SurfmanFrameProducer::export_dmabuf_frame` creates an
immutable top-left RGBA snapshot in an explicitly linear GBM allocation on the
current EGL display's DRM render node. `PendingDmaBufFrame::try_take_frame`
polls a GL fence with zero timeout. It yields owned descriptors only after the
snapshot completes; cancellation releases custody without waiting. This path
uses EGL DMA-BUF import and GBM, with no guessed layout or CPU pixel readback.

The wgpu-30 host creates its Vulkan device with
`vulkan_dmabuf::create_dmabuf_host_context` before constructing its renderers.
`import_dmabuf` records foreign ownership acquisition through a wgpu-owned raw
command encoder and retains the image through submission completion. It does
not wait for queue idle. A completed producer snapshot is required; merely
flushing GL does not establish completion across the two APIs.

The repository's `grafting-servo` also exports a Linux
`ServoDmaBufAdapter` pre-present hook. Its completed snapshots can enter the
shared `wgpu-cue` native-frame importer while the browser control protocol stays
with its existing host.

## macOS Metal storage handoff (wgpu 30)

The `metal_surface` module allocates RGBA8/BGRA8 IOSurfaces on the host's
Metal device. `SharedMetalSurface::export_right` returns one owned Mach send
right. A host transport must transfer that right using an OS capability
mechanism; serializing its integer name does not transfer the resource.
`lookup_surface` retains the received storage and `import_surface` wraps it
on the consumer's Metal device without a CPU pixel copy.

Readiness must already be established by the host. One supported ordering is
an authenticated control message sent after the producer observes its GPU
completion callback. The frame's `FrameSync::None` then refers to that explicit
prior ordering. This does not mean arbitrary concurrent writes are safe.

After its final consumer submission, call `MetalRelease::seal`, then drop all
texture/view clones. `poll_released` is nonblocking and reports completion only
after both the queue callback and the HAL texture's final drop. Dropping the
release tracker early abandons notification; it does not grant storage reuse.
The producer must wait for this acknowledgement before overwriting a slot.

This API is new on main after registry 0.6.1. Consumers must select the same
exact Graft source for both `grafting` and `grafting-frame` until a release
contains it. The wgpu-cue two-process proof qualifies native Metal on an AMD
Radeon Pro Vega 56, including pixel equality, held clones, and producer exit.
It makes one producer GPU copy into exportable storage; this API does not
claim that arbitrary renderer output allocations are already exportable.

## License

[MPL-2.0](../LICENSE)

### Linux Vulkan export storage (wgpu 30)

`vulkan_surface::SharedVulkanSurface` allocates exportable, single-plane linear
RGBA8 DMA-BUF storage on a context from `create_dmabuf_host_context`. It queries
export/import support for the exact DRM modifier and image usage. `copy_from`
copies a texture on that device, releases ownership to FOREIGN/GENERAL, and
provides nonblocking `poll_ready`; `export_fd` refuses an unfinished frame.

`vulkan_surface::import_surface` consumes an owned descriptor after the host has
observed producer completion. Its `VulkanRelease` waits for a sealed queue fence
and destruction of every wgpu texture/view alias, then submits a real native
ownership release. Only completion of that release permits an acknowledgement.
The final consumer transition discards consumed pixels, so it does not assume a
particular final wgpu layout. Reuse always overwrites the complete image.

The host owns descriptor transport, trust, placement, leases, and stale-message
rejection. Calling the producer's unsafe `acknowledge_release` requires that
validated release witness from every importer. A dead peer is insufficient.
Unsupported devices refuse this path; it does not implement external semaphore
synchronization, modifier negotiation, cross-device qualification, or presentation.

### Windows DX12 export storage (wgpu 30)

`dx12_surface::SharedDx12Surface` allocates shared RGBA8 storage and a shared
fence on the producer's actual device. `copy_from` copies a same-device texture,
transitions the backing to COMMON and queues a positive odd fence value.
`export` duplicates local owned handles for host transport without waiting for
GPU completion. Transport must duplicate them into the receiving process and
bind resource, fence and value to the same trusted publication and adapter LUID.

`dx12_surface::import_surface` opens the received handles, checks the native
resource shape and queues a producer-fence wait on the consumer queue. After
its last use, call `Dx12Release::seal` with the paired texture: this records a
COMMON transition, the following even shared-fence signal, and an independent
consumer-fence signal. The consumer-only fence is necessary because producer
device exit can poison the shared fence with UINT64_MAX. Drop all imported
texture/view aliases and poll for release. A resource-private IUnknown witness
tracks actual native-resource destruction; the tracker retains no resource
reference that would keep its own witness alive.

Only then may the host acknowledge the exact current lease to the producer.
The producer's unsafe `acknowledge_release` independently checks consumer GPU
completion before permitting another copy. A fence by itself, a dropped token,
or a dead peer never authorizes reuse. Hosts serialize native queue operations
with wgpu submission. The API does not implement process launch, handle transport,
lease authentication, cross-adapter sharing, presentation or frame pacing.

Both `grafting` and `grafting-frame` must select the same exact source revision
until a registry release contains this API. The Windows qualification procedure
and evidence belong to wgpu-cue's dated DX12 handoff document.
