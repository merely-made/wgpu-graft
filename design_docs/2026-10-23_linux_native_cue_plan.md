# Linux native frames through Cue

**Status (2026-10-23): implemented with bounded ThinkPad receiving.** The lower
GL/Vulkan pixel fixtures, registry shared-importer receiving and real Turnstone
Servo controls/lens/mixed-card receiving pass on hardware RADV Renoir in native
GNOME Wayland. Graft 0.6.1 and Wgpu Cue 0.1.2 are published. This is the canonical
cross-repository record; the consumer receipt binds executable and source to
those platform results rather than claiming blanket browser acceptance.

## Feature targets and validation criteria

### Owned producer snapshots

Done-condition: a Servo pre-present hook supplies an immutable, move-owned,
top-left DMA-BUF frame whose producer work is complete, without CPU pixel
readback or a GPU completion wait in acquisition/presentation.

- **A snapshot has its own allocation.** A renderable GBM buffer is allocated
  with the explicit linear modifier on the current EGL display's DRM render
  node. EGL imports that allocation into a GL texture. A pre-present blit
  snapshots and flips the rendered source exactly once. Repainting or recycling
  the Servo back buffer cannot replace its pixels.
- **Completion stays with the producer.** `PendingDmaBufFrame` polls its GL fence
  with a zero timeout and retains the GL context, snapshot resources and owned
  descriptor until ready. Only a completed snapshot crosses as
  `FrameSync::None`. That means producer-complete here; GL flush alone is not a
  cross-API completion guarantee. Cancellation releases custody without waiting.
- **An unspecified modifier is not a layout declaration.** The first hardware
  probe exported an ordinary GL texture and received `DRM_FORMAT_MOD_INVALID`.
  Its pixels happened to import correctly, but that did not establish a portable
  layout promise. The production snapshot instead requests and verifies a
  single-plane linear GBM allocation. There is no guessed modifier fallback.

### Host import and lifetime

Done-condition: Cue imports an owned DMA-BUF on the existing Vulkan device,
validates its declared shape/format/alpha, preserves producer pixels and returns
without a queue-idle or CPU fence wait. Refusal closes image/sync custody.

- **Import policy remains above the neutral envelope.** `grafting-frame` and
  `webcue` remain at 0.1.1. `wgpu-cue` 0.1.2 adds `GraftVulkanImporter` over
  Graft's DMA-BUF importer. It accepts completed snapshots and explicitly refuses
  external-semaphore synchronization that this binding does not implement.
- **The foreign acquisition preserves contents.** It acquires from the foreign
  queue in `GENERAL` layout, following the
  [Khronos external-image layout rules](https://docs.vulkan.org/spec/latest/chapters/resources.html).
  Using `UNDEFINED` would permit discarding producer data. Graft queues the
  acquisition through a wgpu-owned raw command encoder. A submission completion
  callback retains its image; wgpu owns queue synchronization and command-pool
  retirement. Its raw and ordinary encoding APIs cannot be mixed in one encoder,
  so resource retention is explicit at this boundary. No raw queue submit or
  independent command-pool retirement is needed.
- **The device must be provisioned before renderers exist.** The host creates
  its Vulkan device with Graft's DMA-BUF/foreign-queue extension set, then adopts
  those handles through Genet's additive `RenderCore::from_wgpu_handles` entry.
  The primary window, lenses, retained renderer and native importer share that
  device and queue. Live native producers continue to refuse automatic device
  replacement; this is not physical device-loss qualification.

### Product receiving

Done-condition: the real Turnstone pool selects Servo per card on native GNOME
Wayland/hardware Vulkan, samples native frames in primary and lens composition,
preserves navigation/input/zoom and retirement, and samples native and retained
producers together on the shared host. Actual adapter observations and sampled
frame directions accompany each accepted case.

- **The Linux producer uses the shared browser lane.** Turnstone wraps completed
  Servo DMA-BUF payloads in `BrowserSurface`; navigation and ordered browser
  controls retain the existing Pelt routing owner. An outstanding snapshot is
  polled before another paint, so animation cannot continually replace unfinished
  work. The legacy importing context remains necessary for existing Windows and
  Metal consumers; Linux's shared path does not use its `glFinish` presentation.
- **Capabilities describe this binding.** Linux reports `NativeFrame` and the
  Vulkan DMA-BUF build feature. Logical key/text and unmodified pointer/scroll
  projection retain their explicit limits; native keyboard, IME composition and
  semantic exposure remain unavailable in this shared Servo binding. Import
  observations in receiving do not silently turn its `Constructed` report into
  an engine-wide support claim.

## Findings — 2026-10-23

- The initial source has an owned DMA-BUF envelope and a lower Vulkan importer,
  but no shared Linux importer. Turnstone's concrete shared Servo lane is gated
  to Metal; its Linux registry refuses Servo construction.
- The legacy Servo presentation calls `glFinish`; foreign DMA-BUF acquisition
  calls `vkQueueWaitIdle`. The new path removes both waits from shared receiving.
- The ThinkPad exposes native EGL on AMD radeonsi/Renoir and hardware Vulkan on
  RADV/Renoir, with the required Vulkan extensions.
- The lower fixture overwrites the source after snapshot, destroys the producer,
  then verifies four consumer pixels, orientation and half alpha. It and the two
  existing RGBA/BGRA DMA-BUF round trips pass with an explicit linear modifier.
- The new Servo adapter compiles against pinned Servo `aac43a3f` on Linux.

## Progress — 2026-10-23

The snapshot producer, asynchronous foreign import, shared importer, host device
provisioning and Linux browser binding are implemented. Graft 0.6.1 is published
at `db2736815e9d1893b8f9372355126056093f7155`; Wgpu Cue 0.1.2 is published
at `e18584a80e5cf23268ea0330ee70386b01e25de8`. Downloaded Rust sources match
those commits. Shared-importer tests pass 12/0 against registry Graft; the Mac
regression passes 11 portable and two explicit Metal GPU tests. Mere `114f4ea26`
receives that cohort and Genet `2b5c0181`; Pelt passes nine Linux/Vulkan receiving
tests. The root receiving lock and standalone Iced workspace's local package
version rows have separate metadata follow-ups; they do not change published
SDK source bytes.

Turnstone `a2655c6e` builds and passes the full native controls/lifecycle and
mixed-card cases on executable
`5b77504a3bc627d9b70b9de92ec5cbf6da75db0ee33f918bedd75827701a16aa`.
Seven source rasters pass exact opaque RGB/orientation checks through inspector
sizing, primary, navigation, zoom, lens, docking and reopen. Four native rasters
pass in the mixed case, where each checkpoint also captures retained Genet on
the same device. Source IDs must match the actual captured rasters; cached
entries cannot establish that result. The two mixed global idle waits still
reach their 240-frame bounds, retaining the known work/wake scheduling cue.

The first full native scenario shows correct lens pixels but omits the lens
source-raster diagnostic. The stronger driver rejects it. Lens composition now
uses the same asynchronous diagnostic path as primary composition; the repeated
case passes. The [consumer receipt](https://github.com/merely-made/turnstone/tree/main/docs/receipts/cue_native_linux_20261023)
retains both that failed receiving gate and the final positive cases. Diagnostic
readback remains opt-in evidence collection rather than frame transport.

The consumer record belongs to
`turnstone/design_docs/2026-08-25_browser_surface_implementation_plan.md`; the
contract assessment belongs to
`mere/design_docs/inker_docs/research/2026-09-03_web_surface_contract_assessment.md`.
Physical IME, human assistive-technology acceptance, physical device loss,
X11/Xwayland and OS-compositor pixel correlation retain separate bounds.
