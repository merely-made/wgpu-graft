# Wgpu Triplet Release Plan

**Status (2026-10-06):** the September registry release is complete;
the bounded Windows Turnstone Scry/direct Weld slice has native receipts.
Post-release contracts, Servo and wider application gates remain in progress.
The completed baseline is `grafting` 0.6.0, `scrying` 0.7.1,
and `welding` 0.14.1; all are published, and the crates.io-only four-host proof is
green. Weld 0.14.1 supersedes 0.14.0 after the first consumer proof exposed an
unsafe DevTools-window capability claim. Host-wide hardware serialization, the
first ScreenCaptureKit reliability pass, and sandboxed CEF embedding are also
complete. Scry 0.7.1 supersedes 0.7.0 with native ordered asynchronous
script-result and cookie-read completions. Post-release contract hardening is
active. A polished combined host demo remains open.

**2026-09-04 follow-up:** `grafting-frame` 0.1.0 establishes the wgpu-free
ownership seam selected below. It owns neutral pixel metadata, DX12 shared
resource and fence handles, Linux DMABUF descriptor tables and semaphore
descriptors, retained Metal textures and IOSurfaces, and retained Metal shared
events. `grafting` consumes and re-exports the new vocabulary under explicit
compatibility names while its released 0.6 API remains intact. Scry and Weld
adoption, importer conversion, and the breaking `grafting` 0.7 cleanup remain
open. Commits `7f7e089080d4ef21327ac35ff4f21d8735ddd0c2` and
`9e25904166185da4b55b157d54b03af5e5d1839d` landed the package and made the
existing DX12 resource wrapper use its custody token internally. CI run
`33939904620` passed the Linux, macOS, Windows core, Windows Iced, and Windows
Servo-demo jobs. The package dry-run and upload each verified six files at
23.2 KiB unpacked / 6.7 KiB compressed, and `grafting-frame` 0.1.0 is published
on crates.io.

This plan is the cross-repo release gate for:

- `wgpu-graft` / `grafting`
- `wgpu-scry` / `scrying`
- `wgpu-weld` / `welding`
- Mere/Inker adapter crates as non-gating architectural consumers, not release
  artifacts or substitutes for the registry-only receipt

The public claim is limited to the completed receipts for Servo/Graft, Scry
WebView2 on DX12, Scry WKWebView on Metal, Scry WPE headless DMABUF on
RADV/Vulkan at its fixed backend size, and Weld CEF accelerated OSR on the
recorded platforms. It does not claim API equivalence, uniform security models,
WPE runtime resizing, or Electron/Tauri replacement. Weld exposes the security
choice explicitly; the completed receipts prove its sandboxed CEF mode on
Windows, Linux, and macOS while retaining the named trusted-content mode.

## Confirmed decisions

1. Graft's public safe API becomes an owned front door with an unsafe raw escape
   hatch. Descriptors that transfer OS or driver ownership are move-only. Copy
   metadata stays copyable.
2. Vulkan descriptor ownership transfers only after a successful driver import.
   The owned wrapper must close on failure or return ownership in the error.
3. DX12 shared handles may remain borrowed only in the unsafe raw boundary
   because `OpenSharedHandle` does not consume them. The safe frame path uses
   shared RAII custody (`Arc<OwnedHandle>`) to match the export cache's reuse.
4. Metal must distinguish borrowed texture pointers from retained ownership.
   Retained wrappers belong in the safe front door; borrowed raw pointers remain
   unsafe.
5. Weld gets an explicit CEF sandbox config. Use
   `CefSandboxMode::UnsandboxedTrustedContent` now, mark the enum
   `#[non_exhaustive]`, and add the real sandboxed variants when they are
   implemented. Construction and demos must spell this choice rather than
   receiving the unsandboxed mode invisibly.
6. The neutral web-surface contract assessment runs separately and does not
   gate publishing.
7. After publishing all three crates, a fresh consumer built only from crates.io
   releases must run on DX12, Metal, and Vulkan. This is the packaging proof.

## Findings

### 2026-10-06 supplier hardware follow-up

The active-session preflight changes are pushed: Weld `b5cf043` and Scry
`9c41e2d`. Eight mocked session cases pass in each repository. Weld hardware
[run 37530088110](https://github.com/merely-made/wgpu-weld/actions/runs/37530088110)
passes RADV, M4 and Intel; its NVIDIA job is queued. The earlier four-host
`65d057d` parity receipt remains scoped to that source.

The first Scry RADV rerun exposed a real WPE main-thread assertion: libtest
constructed WebKit on a worker. Scry `d23ba45` changes the native integration
binaries to run on the process main thread. `97b7579` additionally refuses
worker-thread construction before native initialization, with a typed error
and an actual-constructor regression. At that exact source,
[run 37533331164](https://github.com/merely-made/wgpu-scry/actions/runs/37533331164)
passes the worker-refusal regression, RADV imported-pixel gate and page-input
gate. The NVIDIA battery is queued.

Both Macs pass the capture-size/DPI expectation unit test now wired into the
hardware workflow. This is the size test, correcting the earlier reference to
a capture-counter test. Native cadence remains failed: Intel observes four
base portrait frames against five required; M4 observes three against five,
and two final portrait resize frames against three. Source diagnostics are
retained with unchanged acceptance thresholds in Turnstone's
`docs/receipts/browser_scry_windows_20261006/`. These failures do not establish
an importer defect or qualify the next release. Fresh passing Mac capture and
queued Windows execution remain required, followed by exact-source package,
tag and registry-only consumer qualification. No publication occurred.

### 2026-10-06 direct consumer input finding

The first direct Turnstone Weld find/zoom run passes after fixing the product
command target: Workbench A owns controls while graph selection remains B.
A real geolocation fixture then exposes a shared-adapter input seam: the pinned
and current `weld-engine/src/welding_0_15.rs::map_pointer` reject mouse pointers,
while Turnstone's winit path supplies richer PointerEvent to preserve modifiers
and held buttons. Its keyboard mapping sends raw key events without CEF's
separate CHAR event. Converting to legacy MouseEvent would lose those fields.
Turnstone qualifies a narrow native mouse/character bridge around the same
producer; the adapter retains owned-frame and ordered-event translation.
Actual lowercase typing exposed a second distinction: CEF CHAR requires the
character code rather than the virtual key used for raw down/up. Corrected
two-page alpha/bravo input, native find, requested zoom (Partial), real permission
denial/callback navigation and zero-producer teardown pass in the Windows
consumer receipt. Scry's two-page input/current-pixel, cookie, resize,
reconstruction and separate-process restart runs also pass at browser commit
`a383cdd`; the receipt remains frozen under
`turnstone/docs/receipts/browser_scry_windows_20261006/`.

The subsequent current-main merge preserves the stable Burn dependency graph
and qualifies the browser again at Turnstone `7176581`. Its
[integration receipt](https://github.com/merely-made/turnstone/blob/7176581a86d77a516717db5862cb65e1240b357c/docs/receipts/browser_main_integration_20261006/README.md)
records a locked combined build, 83 passing focused browser tests and all four
passing native scenarios on one fingerprinted executable. Mere's new
`3d1cdacc` pin has identical Inker/surface-api trees to the earlier pin; only
Cargo.toml/Cargo.lock differ from the prior application qualification inputs.
The older broad-suite network sync-round failure, which passed in isolation,
remains recorded at its original source. This is not a clean full-suite claim.
The follow-up belongs to Mere's shared adapter and requires both mouse/text
contracts and the actual consumer fixture before removing that bridge. This
finding does not change the producer hardware or registry release claims.

The next Servo consumer is a real process-owned browser composite: select an
immutable upstream Servo source, retain one event loop with per-view ordered
delegates, state profile support explicitly, import on the host device with
GL frame-origin handling, and prove input/resize/teardown. The existing Graft
importer and a split navigation/message merge do not supply that composite.
Turnstone's broader Mere integration remains held at its reviewed boundary.

### 2026-10-05 source audit and next gates

The source audit inspected local source, tags, and retained receipt records.
The orchestrator then read the current registry and GitHub workflow results;
it did not rerun native hardware. The recorded September
registry proof (`33916390001`) qualifies the released triplet above. It does
not qualify later source changes or an application that uses those libraries.

A separate fresh registry read by the Turnstone orchestrator on 2026-10-05
confirmed latest stable versions `grafting` 0.6.0, `grafting-frame` 0.1.0,
`scrying` 0.7.1 and `welding` 0.14.1 through the crates.io package API. This
confirms publication state, not a rerun of the earlier hardware receipts.

- Graft `main` is `403a30c2fab39c573d1eebb57a0995e2c3347ff1`; the
  `grafting-v0.6.0` tag peels to
  `816f3e7857afee863200e1c25b300c43b1532aae`. The manifest still says
  `0.6.0`, but `grafting/Cargo.toml` now depends on `grafting-frame` and
  `grafting/src/lib.rs` exposes its neutral custody vocabulary. Treat this as
  post-release source, not the exact registry 0.6.0 package. The new package's
  publication receipt is recorded above; Scry/Weld adoption, importer
  conversion, and the proposed breaking 0.7 cleanup remain open.
- Weld `main` is `65d057def7db2b5034add6e316cbd51d97c163a1`; its manifest
  says `0.15.0`, while the latest local release tag is `welding-v0.14.1`
  at `c7b7b4c52138643e22580a918542e8e036a0b24a`. Required owned frame
  delivery and ordered caller-id script/cookie completions have landed.
  The earlier parity run `33935649650` at `4784d07` was cancelled. Fresh
  [parity run 37205146754](https://github.com/merely-made/wgpu-weld/actions/runs/37205146754)
  at the current `65d057d` head passed NVIDIA, M4, Intel and RADV. This closes
  the old pending NVIDIA parity gate. Hardware run `37197779933` passed
  NVIDIA and both Macs but stopped RADV before product execution because its
  preflight hardcoded an expired logind session ID.
- Scry hardware run
  [37210994529](https://github.com/merely-made/wgpu-scry/actions/runs/37210994529)
  at current `99c0a9d` passed Windows and M4. Intel passed nine of ten gates:
  base capture cadence produced three complete frames against the required
  five, while resize passed. That acceptance failure does not identify its
  cause. RADV also stopped at the expired-session preflight. Scry and Weld
  now have local preflight patches that enumerate the runner user's sessions
  and require an active Wayland session for that UID; eight mocked acceptance
  and refusal cases passed in each repository. A real RADV rerun is still
  required. Scry's new capture-counter unit test also needs a native Mac run;
  the hardware workflow does not execute it.
- Mere's opt-in direct `WeldingSurface` adapter is implemented in
  `mere/crates/inker/engines/weld-engine/src/welding_0_15.rs`. It retains owned
  frames and projects ordered completions using the exact surface config.
  Runtime/bootstrap, profiles, host import, and app policy remain host-owned.
  Its reviewed Weld pin is `4784d07`; Turnstone currently pins `65d057d`.
  Align package identity before adopting it, then retain app-specific limits
  for winit input, native dragging, find selection, and zoom readback.
- Graft's Servo adapter supplies rendering and interop, not a complete browser
  producer. `mere/crates/inker/engines/graft-engine/src/producer.rs` still
  delegates through the host's `GraftSurface` composite and merges separate
  navigation/message polls. A real host must provide an ordered event source
  and explicit capability truth before claiming a unified browser contract.

**Next library release done-conditions:** choose and version the exact public
source; rerun declared-floor, OS/wgpu, ownership/failure, and platform-demo
gates; obtain fresh headed receipts on every claimed host; package and tag the
same source; repeat the registry-only four-host proof with the new versions.
The existing release stop conditions continue to apply. A unified neutral
browser contract is a separate claim and still requires its consumer proof.

**Application consumer gates:** Turnstone's Windows Weld consumer adopts the
shared version-pinned adapter, preserving ordered script/cookie completions
and owned-frame custody. The host validates each envelope and imports every
paint on its existing device. A narrow mouse/CHAR bridge remains until Mere
closes its shared input gate. Unsafe native DevTools stays refused; correlated
hosted capture, popup-widget composition, downloads and inspection remain
separate slices. The bounded Windows Scry and Weld factory/input/profile/find,
requested zoom/permission/teardown scope has native receipts. Servo still needs
a real process-owned browser producer, factory and registration. These
application gates inform practice and the combined-demo milestone; they do
not reopen the completed September package release. Track implementation in
`turnstone/design_docs/2026-08-03_user_agent_taxonomy_plan.md`.

### 2026-10-05 B1 Windows Scry consumer source findings

Turnstone now has local Windows Scry factory, registry, input, and owned-frame
import changes. These are source implementation findings; full native consumer
qualification remains gated on cookie retention and current captured pixels.
The receipt home is
`turnstone/docs/receipts/browser_scry_windows_20261005/README.md`. Earlier
compile and diagnostic receipts remain separate from full native qualification
and do not expand the released triplet claim. Initial two-page captures and
distinct-fence counters establish bounded construction/paint evidence; later
unassisted timer-driven progression and zero-resource close assertions are
recorded below. The latest full and separate restart scripts complete without
external repaint; bounded DOM input, owner selection, switching, resource
teardown and saved pin/localStorage/text restoration pass. Cookie and pixel
failures prevent full consumer acceptance.

- **Fence scope is per producer.** Scry 0.7.1's D3D11 capture signaler starts
  its counter independently for each capture context
  (`wgpu-scry/scrying/src/windows_capture.rs`, `D3d11FenceSignaler::signal`).
  A single fence shared by two producers could therefore satisfy a wait for
  one page with the other page's signal. Turnstone's
  `src/shell/scry.rs` creates a dedicated `Dx12FenceSynchronizer` for each
  producer and retains that same Arc in its wrapper and node importer. The
  outer Inker frame's borrowed fence identity/value must match the selected
  importer and native payload. Simultaneously live producers need distinct
  fence identities in the headed proof; handle identity is diagnostic and may
  be reused after close.
- **Mouse input is an adapter boundary.** The pinned Inker Scry adapter lowers
  a mouse `PointerEvent` into Scry's portable pointer path, while WebView2's
  native `SendPointerInput` accepts touch and pen. Turnstone's local wrapper
  routes mouse events through Scry's `send_mouse_input`, preserving changed
  buttons, held buttons, modifiers, and leave/cancel behavior. Touch/pen still
  use the existing pointer path. This host workaround does not qualify other
  hosts or change the published Scry API; an owner-side adapter correction is
  a separate slice.
- **Keyboard dispatch must return to the host event loop.** The published
  Scry 0.7.1 Windows keyboard helper submits CDP `Input.dispatchKeyEvent` and
  blocks while pumping thread messages with `DispatchMessageW`. Turnstone's
  scenario text input calls it inside a winit event handler. The initial
  two-page capture/count guards passed in the subsequent native run, but the
  input-stage capture never arrived; the stopped run is retained in
  `turnstone/docs/receipts/browser_scry_windows_20261005/native/aborted.json`.
  Nested message dispatch is a concrete source seam consistent with that
  stall, rather than a captured native stack diagnosis. Turnstone's local
  `src/shell/scry.rs` now queues keyboard CDP commands per producer, with
  one request in flight and a completion callback through the already-locked
  native `webview2-com` 0.39.1 binding. Callbacks only record completion through
  Weak queue state; normal host polling submits the next command, without a
  nested message pump or dispatch from inside the callback. The queue preserves
  raw-key-down, character, and key-up completion order; asynchronous submission
  order alone would not prove processing order. Dispatch/completion errors or
  a two-second in-flight deadline retire queued commands and surface on the
  next fallible acquire/send. Pure regressions cover input parameters,
  completion ordering, timeout/failure retirement, and Weak teardown. The
  latest callback source compiled, and all five keyboard regressions passed
  within the 48 passing Shell tests; 15 Inspector tests also passed
  (`shell-tests-final.log` and `inspector-tests-final.log` in the receipt home).
  This did **not resolve or qualify native progression**: the callback
  executable, SHA-256
  `9221c083785fddf15c0a00347714532a0323e587682d3922496c1d64e95e30b0`,
  again reached the initial two-page capture but produced no input checkpoint
  after more than six minutes. The stopped run is retained in
  `turnstone/docs/receipts/browser_scry_windows_20261005/native-nonblocking/aborted.json`.
  That stopped run did not establish its native stack or root cause. The
  subsequent loop trace below measures a consumer wake boundary; blocked COM
  and capture/occlusion are not established. Ordered DOM text without external
  intervention, current pixels after interaction, input, teardown, and restart
  qualification remain pending. This host adaptation changes neither the
  published Scry API nor any repository dependency pin.
- **Producer availability needs a host wake hook.**
  `turnstone/docs/receipts/browser_scry_windows_20261005/native-loop-trace/`
  records native focus and mouse down/up returning, followed by completed
  render, accessibility update, and `trace-after-input-click` checkpoint.
  Further redraw callbacks then stop. `wait-chain.json` reports `cycle: false`;
  this is not a native stack diagnosis or evidence of blocked COM. A repaint
  to the uniquely named owned Turnstone product window resumes the remaining
  steps, ordered CDP `alpha`, and `RESULT ok`. This is an **intervened
  diagnostic**, not native acceptance. The first repaint probe targeted the
  private capture `Static` HWND returned by `Process.MainWindowHandle`;
  `owned-windows.txt` and `redraw-product-window.json` retain the corrected
  product-window target and intervention. The measured boundary is consumer
  window scheduling/wake, rather than a proved blocked native input call.
  Turnstone's retained deadline wake/redraw fallback now has three passing
  standalone timer regressions and 51 passing targeted Shell tests, including
  those three cases (`surface-poll-standalone-tests.log` and
  `shell-tests-wake-final.log` in the receipt home). The deliberately broken
  reset-on-every-event control fails the early-wake regression as expected
  (`surface-poll-reset-positive-control.log`). This distinguishes an independent
  retained deadline from a timer continually postponed by unrelated events.
  The fresh `native-wake/` full scenario completes normally **without external
  repaint**, but its final result is **`RESULT fail`**, not B1 acceptance.
  `source-manifest-wake-first.json` preserves that source and executable
  SHA-256 `1a1a2dc8fb512bd635984b3caba2618ea6273f687f55ab91fbd0ec66a44095ee`.
  Native frame/import/wait, custody, and teardown assertions advanced through
  all stages; `native-wake/closed.surface-frames.json` records zero live
  producers, cached frames, and importers. The strong fixture also exposed
  missing Enter carriage-return activation (`clicks=1`, expected `2`), missing
  cookies after reload/reconstruction while localStorage text/markers persist,
  and scenario selection through `OpenAddress` reloading an old page instead
  of selecting it. This remains a historical failed diagnostic. The latest
  result and repaired input/owner boundaries are recorded below; cookie and
  presentation gates remain open. Successful scheduling and teardown
  progression do not waive those product gates.
  The concrete trio/Inker integration requirement is a producer event-loop wake
  hook for frame, event, and completion availability, so host progress does not
  depend on paint coalescing or continuous polling. This belongs at the
  producer/adapter-to-host seam; Graft's resource importer retains its existing
  device/import ownership boundary.
- **Owned custody and every paint are retained.**
  `turnstone/src/shell/surface_frames.rs` validates the surface envelope
  against its owned DX12 payload before import and retains the imported
  texture on the host device. Fresh allocations wait through Scry's importer;
  an unchanged source/epoch/metadata reuse explicitly waits on its new producer
  fence before sampling. A failed owned-frame operation clears the cached
  texture. Replacement/close removes the producer, cached texture, and node
  importer with the synchronizer retained through disposal. Legacy Weld
  callback copying/import remains separate. Local regressions cover custody
  refusal/drop and mocked repeated-allocation waits. The unassisted failed
  native run above advances frame counters and zero-resource close checks;
  full product qualification still requires native cookie persistence and
  continued current pixels on both producers. Current
  WebView2 capture allocates a new texture per paint, so native frame/import/wait
  counters qualify that path rather than replacing the reuse regression.

**Latest bounded Turnstone result, 2026-10-05.**
`turnstone/docs/receipts/browser_scry_windows_20261005/source-manifest.json`
binds 152 source/fixture inputs to executable SHA256
`a245130a4becccd35c81c78a5108a2a74d0e980fa52744937711f4e678421948`.
The `native-clipped/` full script completes without external repaint and fails
only four cookie assertions: B in-place reload, A reconstruction, and reopening
both closed pages. DOM input reports alpha/bravo and two activations (pointer
plus ordered CDP Enter); wheel/reset and sibling-preservation, pane resizing,
engine switching and capability refusal assertions pass. Closing both pages
records zero live producers, cached frames and importers. Workbench commands
follow their active member independently of graph selection; normal tab presses
use painted, cell-clipped hit rectangles. All 18 current Workbench tests pass,
including the long-title collision regression. Locked all-target default and
combined `scry,weld` checks pass. Earlier targeted Shell/Inspector runs passed
51/15 tests; there is no new full-suite success receipt.

`native-restart/` runs in a separate process with the same executable, profile
and origin, without reseeding or toggling content. Engine pins, two restored
cells/producers, localStorage markers and saved text pass. Its two cookie
assertions fail. Native CookieManager reads correlated across in-place reload
and the exact profile-store behavior are the next Scry/host boundary evidence;
the backend root cause is not established.

Manual review in `visual-review.json` detects additional presentation failures:
B's body shows an older click count than its current title/DOM assertion; A's
body shows scroll 285/wheels 1 after current DOM reset 0/wheels 2. Both reopened
tiles are blank in the final full-run capture; B is blank in the first restart
capture and appears later. Positive matching frame/import/wait counts and
distinct fences do not prove current pixels. Correlating native captured,
imported and composited images with the current page generation is a Scry plus
host adapter/compositor gate, with no proved cause yet. The producer wake hook
above also remains a reusable integration requirement. **B1 stays open.**
Turnstone's combined build uses Git `welding` 0.15.0 at `65d057d`, whereas Scry
and Graft are registry 0.7.1/0.6.0 through the existing Mere pin. This is not
the registry-only stable welding 0.14.1 release-consumer gate. RADV reruns,
Intel capture cadence, the new Mac counter-unit headed receipt and exact-source
package/registry-consumer gates remain separate; no release/workflow was
dispatched by this consumer lane.

Supplier review finds a concrete Scry 0.7.1 capture freshness defect: arrival
notifications are coalesced by marking all seen, but acquisition dequeues one
sample from the two-slot frame pool. The newer sample can remain unread if
painting stops. The bounded Scry owner fix drains the pool while retaining the
newest sample and closing superseded custody. All five focused Windows capture
tests pass, including the broken single-dequeue positive control; exact source
hash and command are in Turnstone's `supplier-scry-freshness.json`. A new
exact-source native consumer must qualify Scry commit `39818a7`. Only its source,
Windows plan and scoped index hunks are committed; the baseline Scry
audit/preflight WIP remains outside that commit because initial hunk ownership
is uncertain. This is consistent with the stale images,
not a proved explanation for all blank tiles. Turnstone commit `529d839`
retains the frozen published-0.7.1 receipt and pins. Mere's capability correction
has since landed at `b6a50a4d` on its own main; Weld's session-discovery repair
is committed locally at `b5cf043` with its native RADV rerun still open.

### Assessment snapshot

| Repository | Assessed HEAD |
|---|---|
| `wgpu-graft` | `2df70d69109c0e351cb9436a181c867f6f43efae` |
| `wgpu-scry` | `4aea1af654d507ae3e95c2506f55bc179a0c5c15` |
| `wgpu-weld` | `20577d81d256371ff8a3f3b0b22acd4f5aa95377` |
| `mere` (Inker and adapters) | `b57d2021bac2bb32febfd5b96098384a63ef58a4` |

These hashes anchor the findings, not the eventual release candidates. Each
phase records newer exact commits as it lands.

**2026-09-03. Dirty state:** `wgpu-graft`, `wgpu-scry`, `wgpu-weld`, `mere`,
and `genet` all reported `## main...origin/main` before this plan was written.

**2026-09-03. Published crate state:** crates.io reports `grafting` 0.5.1,
`scrying` 0.6.0, and `welding` 0.13.0. The local manifests are already bumped
to `grafting` 0.6.0, `scrying` 0.7.0, and `welding` 0.14.0.

**2026-09-03. Graft ownership blocker:** `grafting/src/lib.rs:322` documents
that `VulkanExternalImage` consumes `dmabuf_fd` and `wait_semaphore_fd`, but
`VulkanExternalImage` is `Clone + Copy` at `grafting/src/lib.rs:326` and
`TextureImporter::import_frame` still takes `&NativeFrame` at
`grafting/src/lib.rs:459`. The lower-level `VulkanDmaBufImport` path is closer
to the right shape: `grafting/src/vulkan_dmabuf.rs:195` takes the import by
value and `PlaneFdGuard` closes descriptors on error.

**2026-09-03. Weld threading blocker:** `welding/src/surface.rs:528` requires
`CefSurfaceProducer: Send`. `welding/src/linux_cef/mod.rs:119` and
`welding/src/macos_cef/mod.rs:150` have bare `unsafe impl Send` declarations.
`welding/src/windows_cef/mod.rs:154` has a Windows-specific rationale and can
remain if the trait-level `Send` requirement is removed.

**2026-09-03. Weld sandbox blocker:** `welding/src/runtime.rs:231` hardcodes
`no_sandbox: 1`, and `CefRuntimeConfig` has no public sandbox choice.

**2026-09-03. Scry capability blocker:** `scrying/src/wpe_producer/mod.rs:48`
returns a WPE capability struct whose `preferred_mode` and `imported_texture`
still say unsupported, while its reason string describes the working WPE
DMABUF path and `scrying/src/wpe_producer/producer.rs:214` installs those
capabilities into real producers.

**2026-09-03. GTK blocking bug:** the WebKitGTK and WebKit6 CPU snapshot paths
hardcode a two-second timeout at `scrying/src/webkitgtk_producer/capture.rs:35`
and `scrying/src/webkit6_producer/capture.rs:181`, despite comments naming the
configured `frame_timeout`.

**2026-09-03. Adapter contract mismatch:** Mere/Inker's
`SurfaceProducer` is deliberately not `Send`
(`mere/crates/inker/inker/src/surface_engine.rs:870`). Its `WebSurface`
contract already wants an ordered event stream
(`mere/crates/inker/inker/src/surface_engine.rs:942`), but cookies and
script-result APIs are still synchronous at
`mere/crates/inker/inker/src/surface_engine.rs:1003` and `:1035`. Weld already
exposes request/poll pairs for script results and cookies in
`welding/src/surface.rs:930` and `:958`.

**2026-09-03. Registry order gate:** Scry and Weld currently require
`grafting` 0.6.0 from a pinned Git revision while crates.io contains only
Graft 0.5.1. Their packages cannot become registry-only until Graft publishes.
`servo-wgpu-interop-adapter` has its own registry-dependency question and does
not gate the `grafting` crate publication.

**2026-09-03. Feature unification is not a release blocker:** the manifests
document that the newest enabled wgpu row wins, and Graft re-exports its
selected `wgpu` types. A consumer that also names an incompatible `wgpu` gets a
compile-time integration error, not an unchecked runtime mismatch. Clearer
diagnostics remain useful follow-up.

## Phase 1: Graft owned import API

Goal: make the safe public API impossible to misuse for consumed native
handles.

Planned changes:

- Add move-only owned wrappers for consumed Linux descriptors, including
  per-plane DMABUF fds and optional semaphore fd.
- Represent shared-allocation multi-plane DMABUF as owned buffers plus
  plane-to-buffer indices so one kernel fd cannot be closed twice.
- Change safe Vulkan import entry points to take the owned frame by value.
- Remove `Copy` from any public type that owns or may own consumed descriptors.
- Represent reusable DX12 resource custody with `Arc<OwnedHandle>` plus a
  separate allocation key for import caches. `OpenSharedHandle` creates the
  imported COM reference without consuming the exported handle.
- Represent Metal custody with a retained Objective-C texture in the safe path.
- Keep raw borrowed/scalar descriptors available under an explicitly unsafe API
  whose docs state who closes each handle.
- Make failure semantics explicit: if Vulkan import fails before ownership
  transfers to the driver, Graft closes the owned descriptors or returns an
  error carrying the still-owned frame. Pick one behavior and test it.

Done conditions:

- Unit tests prove owned fds close once on validation failure.
- A forced Windows demo import error proves cleanup is not skipped by an early
  `?`; the current import-then-close call pattern leaks on that path.
- Linux DMABUF round-trip still passes on a compatible Vulkan host.
- Public docs and changelog describe the breaking API change.
- Scry and Weld can adapt without retaining duplicate fd-close sites.

## Phase 2: Consumer ownership cleanup

Goal: remove hand-managed descriptor lifetime from producers and demos where
Graft can own it.

Planned changes:

- Update Scry's WPE DMABUF frame handoff to produce the new owned Graft input
  or a clearly borrowed raw descriptor, not a copyable consumed fd struct.
- Update Weld's Linux CEF DMABUF path the same way.
- Remove duplicate manual close logic from demos after ownership crosses into
  Graft.
- Keep producer-side stale-frame eviction closing only descriptors that have
  not been handed to Graft.

Done conditions:

- `cargo test -p grafting`
- focused Scry WPE ownership tests
- focused Weld Linux native-frame tests
- no reachable path closes a descriptor after a successful import handoff

## Phase 3: Weld threading and sandbox honesty

Goal: make the CEF producer API match the thread-affinity contract hosts
actually need.

Planned changes:

- Drop the `Send` supertrait from `CefSurfaceProducer`.
- Remove `unsafe impl Send` from Linux and macOS producers.
- Keep the Windows `unsafe impl Send` only if its documented proxying rationale
  survives `cargo check` and local review after the trait bound is gone.
- Add `#[non_exhaustive] pub enum CefSandboxMode` with the initial variant
  `UnsandboxedTrustedContent`.
- Add `sandbox: CefSandboxMode` to `CefRuntimeConfig`; the current initializer
  sets `no_sandbox` only when the mode is `UnsandboxedTrustedContent`.
- Carry the same choice through CEF's subprocess entry path; its current
  path-only signature cannot express a process-wide sandbox policy.
- Update README and runtime docs to call Weld a trusted-content embedder until
  a sandboxed mode lands.

Done conditions:

- `cargo check -p welding --no-default-features`
- `cargo check -p welding --features cef-runtime`
- Weld docs no longer imply a sandboxed production process model.
- Linux/macOS producers no longer claim cross-thread movability.

## Phase 4: Capability and timeout repairs

Goal: make probes and adapters describe actual runtime behavior rather than
hopeful defaults.

Planned changes:

- Record WPE headless as an imported-DMABUF Vulkan path with a fixed native
  render target. Its capability and degradation report must expose the runtime
  resize limit. The current WPE 2.52 headless toplevel remains 1024x768 and
  `resize` returns `Unsupported`.
- Make Scry capability fields one-to-one enough that adapters stop guessing
  about cookies, script, capture, devtools, downloads, popups, drag/drop, IME,
  accessibility, and degradation reasons.
- Fix WebKitGTK and WebKit6 CPU snapshot timeout handling to use
  `frame_timeout`.
- Override GTK producer non-blocking acquisition so Inker does not block and
  discard CPU frames in `scrying-engine`.
- Make Weld's capability probe account for the `cef-runtime` feature, not just
  the target OS.
- Add explicit capability tests for every changed field.

Done conditions:

- Scry and Weld capability probes have tests for feature-off and feature-on
  behavior.
- `scrying-engine` does not block inside `SurfaceProducer::acquire_frame` when
  the producer only has CPU snapshot output.
- Capability rows that are unsupported carry actionable reasons.

## Phase 5: MSRV and packaging hygiene

Goal: ensure the release can be consumed from crates.io without local checkouts.

Planned changes:

- Declare an MSRV in every published crate manifest. Start from 1.92 because
  the supported wgpu 28 row declares that floor (wgpu 29/30 declare 1.87), then
  raise it only if an exact-version build of a complete publishable feature
  graph requires more. The demo workspace's 1.97.1 pin is not the library MSRV.
- Replace Scry and Weld git-rev `grafting` dependencies with crates.io
  `grafting = "0.6.0"` after Graft publishes.
- Ensure package metadata and READMEs match the release posture. Remove
  generated-by-AI boilerplate from public crate READMEs; it is process residue,
  not useful package documentation.
- Dry-run packaging before publishing each crate.

Done conditions:

- `cargo package -p grafting` and each crate's verified publish dry-run pass
  from clean candidate commits. Package contents are inspected before upload.
- Fresh temporary consumers can resolve `grafting`, `scrying`, and `welding`
  without git dependencies.
- `cargo metadata` in those consumers shows one selected wgpu major per test
  row.

## Phase 6: Publish and prove

Goal: publish in the only order that can prove the stack.

Order:

1. Land Graft ownership changes and publish `grafting` 0.6.0.
2. Update Scry and Weld to depend on crates.io `grafting` 0.6.0.
3. Build fresh external consumers of Scry and Weld against the published Graft.
4. Publish `scrying` 0.7.0 and `welding` 0.14.1. Weld 0.14.0 was published in
   the original sequence and superseded by 0.14.1 before the release gate
   closed.
5. Build a fresh consumer using only the three crates.io releases.
6. Dispatch `.github/workflows/registry-only-triplet.yml` with the immutable
   Scry/Weld source refs that produced the selected releases. It stages their
   demo/test source as fresh consumers; those checkouts are fixtures only and
   never Cargo dependencies. Each staged manifest pins `grafting` 0.6.0,
   `scrying` 0.7.0, and `welding` 0.14.1 exactly.
7. Run fresh, registry-only platform harnesses on DX12, Metal, and Vulkan. The harnesses must
   record `cargo metadata`/`cargo tree` evidence that all three library sources
   are registry packages, with no Git or path override.

Done conditions:

- DX12 proof uses Windows hardware and imports a live browser frame into a
  host-owned wgpu composition path.
- Metal proof runs on both Apple Silicon and Intel Mac when available.
- Vulkan proof runs the Linux DMABUF path on the RADV host.
- Graft's Servo lane and Weld's CEF lane each pass on DX12, Metal, and Vulkan.
- Scry passes WebView2/DX12, WKWebView/Metal, and WPE/Vulkan import at WPE's
  fixed native size. The WPE battery proves frame import, input, script, and
  cookies, plus an asserted `Unsupported` runtime-resize result. Its
  WebKitGTK 4.1 and WebKit6 CPU frames are uploaded into a host-owned wgpu
  texture, or those backends are explicitly excluded from the wgpu-composition
  claim. A native child overlay does not count as wgpu composition.
- Each claimed browser path loads a deterministic local page, produces a page
  pixel, observes pointer input, and proves script/cookie behavior. Paths that
  support runtime resize also prove resize; WPE instead proves its fixed render
  target and honest resize rejection. Weld's CEF path supports an explicit
  sandboxed mode on all three desktop platforms, with a bootstrap-owned sandbox
  context on Windows and native CEF sandbox initialization on Linux and macOS.
- Each WKWebView capture receipt wakes and holds the headed WindowServer session
  before the battery. At least one complete ScreenCaptureKit sample carrying an
  image buffer must arrive before acquire/import is testable. Status-only
  samples fail the capture preflight rather than counting as a zero-frame
  product result.
- The receipt records exact crate versions, host backend, selected wgpu row,
  hardware host, and the observed frame/pixel/input result.
- The registry-proof artifacts retain each staged `Cargo.lock`, full metadata,
  and dependency tree. The verifier requires exactly one registry source for
  every triplet crate. Graft's copied local Servo adapter and its commit-pinned
  Servo 0.5/git build scaffolding are recorded separately and cannot replace a
  triplet package.
- The Scry hardware batteries must cover script/cookie when their capability
  reports claim them. The staged Weld battery adds an ephemeral cookie
  round-trip case and requires its asynchronous `weld_probe` readback, in
  addition to the existing script receipt.
- One completed registry-only workflow run records green NVIDIA DX12, M4 Metal,
  Intel Metal, and RADV Vulkan jobs. Retained artifacts include each staged
  `Cargo.lock`, Cargo metadata/tree, registry verifier output, exact crate
  versions, physical adapter/backend, and browser gate logs. Progress records
  the run, harness commit, artifact names, and superseded failed runs.

## Non-gating parallel item

The neutral contract assessment landed in Mere commit `2f1fc77c` at
`mere/design_docs/inker_docs/research/2026-09-03_web_surface_contract_assessment.md`.
It recommends first settling Inker's correlated asynchronous request/event
model and capability truth, then factoring Graft's native-frame transport
behind a wgpu-free seam, and only then deciding whether to extract
`surface-engine-api`. This did not gate versions 0.6.0/0.7.0/0.14.1, but it
gates any future claim of a unified public browser-surface contract.

## Release stop conditions

- Any ownership test shows a double-close, leak, or reusable stale descriptor.
- A platform producer still claims `Send` while its required UI-thread affinity
  is unchanged.
- A capability report says `Supported` when the selected build cannot construct
  or emit that path.
- An ordinary frame poll blocks past its non-blocking contract.
- A fresh package consumer resolves a path or Git Graft dependency.
- A registry-only hardware consumer fails a backend or transport that the
  release claims.

Crates.io versions are immutable. If a published crate proves defective, stop
the train, yank only the affected version when necessary, and issue a new patch
release. A failure in Scry or Weld is not a reason to yank a healthy Graft.

## Post-release milestones

- [x] Implement CEF sandboxed modes for Weld and update the security posture from
  trusted-content preview to sandboxed embedding where proven.
- [x] Assess whether to publish a neutral `surface-engine-api` crate or keep
  complete per-engine capability structs without a shared crate. The assessment
  is recorded under the non-gating item above; implementation awaits the
  correlated asynchronous event model and capability cleanup it identifies.
- [ ] Add a polished host-owned wgpu browser-surface demo that compares Servo,
  system WebViews, and CEF under one host UI without implying they have the same
  process model or security guarantees.
- [x] Add an OS-level, host-wide mutual-exclusion lock shared by Graft, Scry, and
  Weld hardware workflows. GitHub concurrency groups do not coordinate across
  repositories or differently named workflows.
- [x] Add per-exit ScreenCaptureKit status/cadence diagnostics and a display-awake
  preflight so a sleeping WindowServer is distinct from a product capture
  failure.

## Progress

- 2026-09-03: Plan founded in `wgpu-graft/design_docs/` after the ownership,
  sandbox, neutral-contract, and post-publish-consumer decisions were confirmed.
- 2026-09-03: Phase 1 implementation started in Graft. Safe native frames now
  carry owned Vulkan descriptors, retained Metal textures, or shared RAII DX12
  resource custody and are consumed by import. Raw borrowed imports remain
  explicitly unsafe. Windows, Linux, and Apple compile/hardware receipts and
  the required new hardware workflow run remain open; the earlier green
  hardware baseline (workflow run 33826415538, commit 12dba60b) predates this
  ownership change and is not validation for it.
- 2026-09-03: Windows local receipt for the Phase 1 slice: isolated
  `cargo check -p grafting --no-default-features --features wgpu-29 -j 1` and
  `cargo test -p grafting --no-default-features --features wgpu-29 -j 1`
  passed. The latter exercised move-only structural assertions and real Win32
  handle custody tests. wgpu-28/30 and Linux/Apple compilation plus fresh
  hardware workflow proof remain required before release.
- 2026-09-03: Packaging hygiene added `rust-version = "1.92"` to the
  publishable `grafting` manifest. A standalone, source-identical package
  audit under Rust 1.92 listed only Cargo metadata, README/build script, crate
  sources, and `tests/dmabuf_roundtrip.rs`. Its exact publishable-core gate,
  `cargo +1.92.0 check --no-default-features --features wgpu-28 -j 1`, passed
  in 4m38s with `rustc 1.92.0`; an isolated all-features check passed in 5m48s.
  `cargo publish --dry-run` remains deferred: CI run 33831062281 is red at the
  pre-fix ownership commit and must rerun green before a registry simulation.
- 2026-09-03: Corrected the GL dispatch boundary exposed by Linux CI and the
  three non-Windows Servo hardware builders. `GlFramebufferSource` remains
  borrow-imported, while resource-bearing native frame variants are consumed;
  the all-features receipt above verifies that combined feature shape locally.
- 2026-09-03: The declared-floor complete publishable graph gate passed in the
  standalone source-identical package copy: Rust 1.92.0 (with `RUSTC`
  explicitly pinned to that toolchain) ran `cargo check --all-features -j 1`
  in 11m30s. This compiled Graft's wgpu 28, 29, and 30 feature rows together.
  `RUSTDOCFLAGS="-D warnings" cargo doc -p grafting --no-deps -j 1` then
  passed from the repository source. The pass required making the Windows-only
  DX12 link and the Linux-only DMABUF links plain code text when their modules
  are cfg-absent. `cargo publish --dry-run` remains deferred until the fresh
  CI and hardware candidate is confirmed green.
- 2026-09-03: Added the manually dispatched `registry-only-triplet` workflow,
  not yet runnable until all three specified versions exist on crates.io. It
  stages immutable Scry/Weld release-source demos and WPE integration tests in
  temporary standalone consumers, rewrites their manifests to exact registry
  triplet dependencies, records metadata/tree/lockfile proof, then runs the
  existing headed batteries on NVIDIA/DX12, M4/Metal, Intel/Metal, and
  RADV/Vulkan. Staging-source checkouts are never triplet dependencies in the
  resolved Cargo graph. Graft's unpublished Servo adapter remains copied test
  scaffolding, but both it and the live Servo demo consume the exact registry
  `grafting` release. The verifier permits only Servo's pinned release commit
  and the exact glslopt build fix outside the registry, records both, and still
  requires registry sources for all three triplet packages. The WPE gate
  rejects a prerequisite `SKIP`; its separate pixel/import and input tests
  jointly cover host-owned-wgpu composition, deterministic pixel, pointer,
  script, and cookie behavior at the backend's fixed 1024x768 render size. The
  Graft demo adds its own registry-backed live frame, page pixel, pointer, and
  resize proof on all four hosts.
- 2026-09-03: CI now enforces that same Rust 1.92 all-features library check on
  Linux, macOS, and Windows. Servo demos keep the workspace toolchain and do
  not expand the published library's MSRV contract.
- 2026-09-04: Graft phases 1 and 5 landed and `grafting` 0.6.0 was published
  from tag commit `816f3e7857afee863200e1c25b300c43b1532aae`. Candidate CI run
  `33845695867` passed all eight jobs, and hardware run `33845695847` passed on
  NVIDIA/DX12, M4/Metal, Intel/Metal, and RADV/Vulkan.
- 2026-09-04: Scry phases 2, 4, and 5 landed and `scrying` 0.7.0 was published
  from tag commit `f421c2ed9e312de112a003d7056cda7b4251da1a`. Exact-candidate
  runs passed the four-host hardware matrix (`33852408046`), wgpu matrix
  (`33852407929`), Rust 1.92 MSRV gate (`33852407757`), macOS tests
  (`33852407955`), and Linux tests (`33852408184`). Its package proof contained
  92 files and verified at 1.3 MiB unpacked / 318.3 KiB compressed.
- 2026-09-04: Weld phases 2 through 5 landed and `welding` 0.14.0 was published
  from tag commit `c8c7bc5b4d02433e683a26de5ddcd4d3e5e102fa`. Candidate runs
  passed the MSRV gate (`33850335500`), wgpu matrix (`33850335575`), parity
  battery (`33850335433`), and four-host hardware matrix (`33850335455`). The
  M4 job's successful rerun is part of that same hardware run.
- 2026-09-04: crates.io resolves exact, non-yanked `grafting` 0.6.0, `scrying`
  0.7.0, and `welding` 0.14.1. Scry and Weld use the published Graft release.
  Fresh staged registry consumers verify a single crates.io source for each
  triplet package before starting hardware work.
- 2026-09-04: Mere's non-gating consumer lane landed on `main` through
  `d7158864`, `a96f9e86`, and `d52428a3`. `OwnedSurfaceFrame` keeps
  `scrying::NativeFrame` owned until host import. Focused receipts passed Inker's
  98 tests, the scrying-engine's 15 tests, and the Scry/Weld adapter checks. The
  present downcast seam is temporary; CPU snapshot handling, complete capability
  projection, correlated ordered completions, direct Weld binding, and the
  factory's `Send + Sync` requirement remain outside this release claim.
- 2026-09-04: Registry-only runs before the final candidate exposed harness
  faults rather than published-crate defects: Windows metadata was decoded with
  the active code page instead of UTF-8, the staged WPE fixture lacked its local
  default/wgpu-30 feature and `pollster`, Weld executable paths were wrong on
  Windows and Linux, and an idle M4 display produced status-only
  ScreenCaptureKit samples. Commits `301d739`, `6bffe71`, `36e24be`, and
  `6f2afc7` corrected the harness. Manual replay of the exact registry-built M4
  application under a display wake/hold delivered five 1024x1536 frames; this
  diagnosed the preflight but does not replace the final workflow receipt.
- 2026-09-04: Registry-only run `33856680667` passed Graft and Scry on every
  host and passed Weld on RADV, M4, and Intel. NVIDIA's sole failure was Weld's
  native DevTools-window case: `open_devtools() ok` was followed by repeated
  CEF 151 GPU-process exits and the fatal `GPU process isn't usable` shutdown.
  The public README already described that Windows/Linux regression, while the
  capability probe still reported it supported and both producers still called
  `show_dev_tools`. This was a real 0.14.0 release defect, not a harness fault.
- 2026-09-04: Weld 0.14.1 was published from
  `c7b7b4c52138643e22580a918542e8e036a0b24a`. It refuses the unsafe native
  DevTools window on all platforms, reports the capability unsupported, and
  keeps CDP supported. Candidate MSRV run `33860036391` passed 12/12 jobs; wgpu
  matrix `33860036415` passed 13/13; headed parity `33860036458` passed on all
  four hosts; and headed hardware `33860036577` passed on all four hosts. Every
  parity job observed a CDP response and the native-window refusal followed by
  another imported frame. The package dry-run and upload each verified 35 files
  at 581.9 KiB unpacked / 136.4 KiB compressed.
- 2026-09-04: Final crates.io-only run `33861094065`, from harness commit
  `9beddecd167eca55591d0a7d169f27db9020fe5a`, passed M4/Metal
  (`100985482661`), NVIDIA/DX12 (`100985482854`), Intel/Metal
  (`100985482969`), and RADV/Vulkan (`100985482994`). Every host independently
  verified registry sources for exact `grafting` 0.6.0, `scrying` 0.7.0, and
  `welding` 0.14.1. Graft imported live 960x640 frames with the expected page
  pixel on all hosts. Scry passed WKWebView/ScreenCaptureKit frame and resize
  coverage on both Macs, WebView2/WGC frame import and scaling on NVIDIA, and
  WPE/DMABUF pixel, input, script, cookie, and post-input-frame coverage on
  RADV. Weld passed import and CDP on every host; its native DevTools refusal
  was followed by another imported frame. Artifacts `registry-triplet-metal-m4`,
  `registry-triplet-nvidia`, `registry-triplet-metal-intel`, and
  `registry-triplet-radv` contain the per-host receipts. This closes the release
  gate for the public claim stated above.
- 2026-09-04: Graft commit `781aa6aad3f63b83ad48b192a6a5d078be1d71b8`
  completed the shared OS-level host lock. The Node 24 action uses an atomic
  directory lock, owner metadata, bounded waiting, stale-lock takeover, and
  token-checked post cleanup. Graft hardware run `33866354785` passed all five
  NVIDIA, RADV, Intel, and M4 jobs. Scry and Weld consume the action through a
  sparse checkout; directly referencing the action subdirectory had caused
  GitHub to stage unrelated repository content and was rejected as the wrong
  integration shape. The final runs below exercised real cross-repository lock
  contention and completed without overlapping a host.
- 2026-09-04: Scry commit `17eb18f9e4fd4804fbb07d361b046093d381f144`
  closed the first macOS capture-reliability pass. The demo now treats winit
  resize targets as logical sizes, derives expected physical dimensions from
  the scale factor, keeps compositor presentation alive without stealing the
  producer sample, requires a distinct final resize frame, and uses the proven
  display wake/hold sequence. Earlier M4 runs exposed status-only
  ScreenCaptureKit samples and a false-positive final resize; those runs are
  superseded. Final headed run `33869967986` passed WKWebView capture and resize
  on M4 and Intel, WebView2 capture and scale on NVIDIA/DX12, and WPE DMABUF on
  RADV/Vulkan.
- 2026-09-04: Weld commit `759555c67f6e858f77deecbed7cf52a2fd000440`
  made the parity battery hermetic and taught it to fail on its own
  `VALIDATION FAIL` receipt. The visibility, API, and CDP cases now use the
  deterministic local animated fixture rather than the network default. That
  exposed zero-frame self-consistency passes before the fix. Final parity run
  `33868840816`, headed hardware run `33868840814`, wgpu matrix run
  `33868840900`, and MSRV run `33868840798` all passed; the hardware runs were
  green on NVIDIA, RADV, Intel, and M4.
- 2026-09-04: Weld commits `730c2d1b10e539f8f54bfebd27ce77fc35d59653`
  and `a4f5cf725b2cc3489930aed50bc4dd496f741295` completed the sandbox milestone.
  Linux enables CEF's native sandbox, macOS initializes and retains
  `libcef_sandbox.dylib` in helper processes, and Windows packages the CEF
  bootstrap executable with a client DLL that carries one bootstrap-owned
  sandbox context through subprocess entry and browser initialization. Headed
  parity run `33899247918` first proved the Linux, Intel Mac, and M4 paths.
  Final headed parity run `33902324155` and headed hardware run `33902324150`
  passed on NVIDIA, RADV, Intel, and M4; wgpu matrix `33902324151` and MSRV run
  `33902324233` also passed. The Windows DX12 pixel fixture additionally passed
  twice from a locally packaged sandbox bundle.
- 2026-09-04: Mere commits `f77e716e` and `b9b0ee13` advanced the neutral
  contract work without changing the released triplet. Inker now carries
  correlated script and cookie request ids on its ordered event stream, and the
  old blocking compatibility calls are gone from Inker and its Graft, Scry, and
  Weld adapters. Pelt consumes page readiness as a web message and imports
  Scry's owned DX12 frame with the same shared synchronizer used by WebView2.
  The headed mixed receipt passed at 1280x800 and 960x640 with three imports,
  three fence waits, thirteen host compositions, and artifact digest
  `85d1b0ba8a86778f`. Direct Weld binding and real Graft/Weld factories in the
  combined Pelt demo remain open.
- 2026-09-04: Scry commits `8be67eec9b75d25f952eb64208749a30a8eacfa6`
  and `8386502915e6bad0f9d9d13cc17bb7d4bbaa769a` added one native ordered event
  stream for navigation, page messages, script completions, and cookie
  completions on WebView2, WKWebView, and WPE. `scrying` 0.7.1 was published
  from the latter commit. Rust 1.92 MSRV run `33913518467`, wgpu matrix
  `33913492908`, Linux run `33913492164`, macOS run `33913491981`, and clean
  four-host hardware rerun `33914446585` passed. The first hardware run's three
  Windows startup/resource failures were superseded by that clean rerun.
- 2026-09-04: Mere commit `c767cb925b84188440712806d9cc02ef79bbd9bb`
  adopted `scrying` 0.7.1 in `scrying-engine`, forwards caller request ids,
  consumes only Scry's ordered queue, maps native completion payloads, and
  reports backend-specific capability limits. Registry-backed tests passed 99
  Inker tests, 24 Graft/Scry/Weld adapter tests, and six Pelt integration tests.
- 2026-09-04: Fresh crates.io-only run `33916390001` passed on NVIDIA/DX12,
  M4/Metal, Intel/Metal, and RADV/Vulkan with exact `grafting` 0.6.0,
  `scrying` 0.7.1, and `welding` 0.14.1. Each host independently resolved the
  registry packages before running the same live Servo/Graft, system-webview,
  and CEF batteries used for the prior release proof. This supersedes the
  0.7.0 registry receipt for the published triplet.
- 2026-09-04: Weld commit `52d74161fb5235c85372641c44fc1f2a101a2c2b`
  opened the breaking 0.15 line by requiring cross-platform owned-native-frame
  delivery on `CefSurfaceProducer`. The direct wgpu import and neutral-frame
  paths consume the same latest-frame mailbox. Local validation passed 49
  library tests and a Windows `cef-runtime` type check; wgpu matrix run
  `33918122258` passed all nine wgpu-version/platform rows and all three
  platform demo builds. Ordered native events and caller-minted completion ids
  still gated the opt-in Mere adapter at this checkpoint; they subsequently
  landed with the direct adapter, as recorded in the October audit above and
  Weld's producer parity plan. The new contract's Windows hardware gate remains
  open in the retained receipt.
