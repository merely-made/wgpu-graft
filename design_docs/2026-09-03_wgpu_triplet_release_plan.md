# Wgpu Triplet Release Plan

**Status (2026-09-03):** plan; implementation not started; release not ready.

This plan is the cross-repo release gate for:

- `wgpu-graft` / `grafting`
- `wgpu-scry` / `scrying`
- `wgpu-weld` / `welding`
- Mere/Inker adapter crates that prove host use of those surfaces

The release target is not "Electron/Tauri replacement" as a blanket claim. The
target is narrower and defensible: browser engines can be embedded as
host-owned `wgpu` composition surfaces on DX12, Metal, and Vulkan, with
capabilities and security posture reported honestly. Weld may ship first as
trusted-content CEF embedding while sandboxing remains an explicit later
milestone.

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

- Fix Scry WPE capabilities so `preferred_mode`, `imported_texture`,
  `cpu_snapshot`, and `supported_frames` match the feature and host state.
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
4. Publish `scrying` 0.7.0 and `welding` 0.14.0.
5. Build a fresh consumer using only the three crates.io releases.
6. Run fresh, registry-only platform harnesses on DX12, Metal, and Vulkan. The harnesses must
   record `cargo metadata`/`cargo tree` evidence that all three library sources
   are registry packages, with no Git or path override.

Done conditions:

- DX12 proof uses Windows hardware and imports a live browser frame into a
  host-owned wgpu composition path.
- Metal proof runs on both Apple Silicon and Intel Mac when available.
- Vulkan proof runs the Linux DMABUF path on the RADV host.
- Graft's Servo lane and Weld's CEF lane each pass on DX12, Metal, and Vulkan.
- Scry passes WebView2/DX12, WKWebView/Metal, and WPE/Vulkan import. Its
  WebKitGTK 4.1 and WebKit6 CPU frames are uploaded into a host-owned wgpu
  texture, or those backends are explicitly excluded from the wgpu-composition
  claim. A native child overlay does not count as wgpu composition.
- Each claimed browser path loads a deterministic local page, produces a page
  pixel, observes pointer input, resizes, and proves script/cookie behavior
  wherever its capability report says those operations are supported.
- The receipt records exact crate versions, host backend, selected wgpu row,
  hardware host, and the observed frame/pixel/input result.

## Non-gating parallel item

The neutral contract assessment lives in
`mere/design_docs/inker_docs/research/2026-09-03_web_surface_contract_assessment.md`. It should proceed
in parallel, but it does not block publishing the current Scry/Weld/Graft
releases once the release gates above are green.

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

- Implement CEF sandboxed modes for Weld and update the security posture from
  trusted-content preview to sandboxed embedding where proven.
- Decide whether to publish a neutral `surface-engine-api` crate or keep
  complete per-engine capability structs without a shared crate.
- Add a polished host-owned wgpu browser-surface demo that compares Servo,
  system WebViews, and CEF under one host UI without implying they have the same
  process model or security guarantees.

## Progress

- 2026-09-03: Plan founded in `wgpu-graft/design_docs/` after the ownership,
  sandbox, neutral-contract, and post-publish-consumer decisions were confirmed.
