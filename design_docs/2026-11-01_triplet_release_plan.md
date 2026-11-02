# Triplet release plan: the next Graft, Scry and Weld train

**Status:** 2026-11-01: cut from `2026-09-03_wgpu_triplet_release_plan.md`,
archived whole in `archive_docs/2026-11-01/` with its receipts and run
narratives. Landed there: the September release (`grafting` 0.6.0, `scrying`
0.7.0/0.7.1, `welding` 0.14.0/0.14.1) with its crates.io-only four-host proof
(runs `33861094065`, `33916390001`); `grafting-frame` 0.1.0 (2026-09-04); Weld's
sandboxed CEF modes, the host-wide hardware lock and the ScreenCaptureKit
diagnostics; `scrying` 0.7.2 and `welding` 0.15.0 (2026-10-07, 2026-10-17); the
Servo v0.7.0 migration qualified on the Windows supplier; the opt-in
`DiagnosticGpuSync` checkpoint; `grafting-frame` 0.1.1 (2026-10-23) and
`grafting` 0.6.1 (2026-10-24, through the Linux native Cue plan). P1 (0.6.1's
disposition and changelog) landed 2026-11-01 (D1). Open: the Graft 0.7 / Scry 0.8
/ Weld 0.15.1 train (P2), a gate mechanism and four-host proof after CI removal
(P3), the Servo GPU ordering fence (P4), the Servo consumer gates (P5), hygiene
(P6) and the combined host demo (P7).

**For:** publishing the next coherent triplet from exact, tagged source, with a
fresh registry-only consumer proof on every claimed host, while keeping the
public claim no wider than its receipts.

Not here:
- Linux native frames through Cue, and its bounded ThinkPad receiving:
  `2026-10-23_linux_native_cue_plan.md`;
- GPUI zero-copy and the demo cleanup: `docs/2026-06-02_bevy_gpui_zero_copy_plan.md`;
- Weld's producer parity and CEF 151 to 154 refresh:
  `wgpu-weld/design_docs/2026-11-01_producer_gates_plan.md`;
- Turnstone's consumer repins and host gates (U9 and the Knot → Redshank →
  Turnstone landing): `turnstone/design_docs/2026-11-02_user_agent_taxonomy_plan.md`
  and `turnstone/design_docs/2026-11-02_browser_surface_implementation_plan.md`;
- the neutral browser-surface contract:
  `mere/design_docs/inker_docs/research/2026-09-03_web_surface_contract_assessment.md`.

Receipts from this plan's runs go to
`archive_docs/receipts/2026-11-01_triplet_release_plan_receipts.md`, never into
this plan (DOC_POLICY §8). Evidence that matters is a test in the repository or a
line in Findings.

## Constraints

Carried from the archived plan's confirmed decisions (2026-09-03):

- Graft's safe API is an owned front door with an unsafe raw escape hatch.
  Descriptors that transfer OS or driver ownership are move-only; copy metadata
  stays copyable. Vulkan descriptor ownership transfers only after a successful
  driver import. DX12 shared handles stay borrowed only in the unsafe raw
  boundary; the safe path uses shared RAII custody. Metal distinguishes borrowed
  pointers from retained ownership.
- Weld spells its CEF sandbox choice explicitly; `CefSandboxMode` stays
  `#[non_exhaustive]`.
- After publishing, a fresh consumer built only from crates.io releases must run
  on DX12, Metal and Vulkan. That is the packaging proof.
- The public claim is limited to the completed receipts. It does not claim API
  equivalence, uniform security models, WPE runtime resizing, or Electron/Tauri
  replacement.
- **Release stop conditions:** a double-close, leak or reusable stale descriptor;
  a platform producer claiming `Send` with unchanged UI-thread affinity; a
  capability report saying `Supported` for a path the build cannot construct; a
  frame poll blocking past its non-blocking contract; a fresh package consumer
  resolving a path or Git Graft; a registry-only hardware consumer failing a
  claimed backend or transport.
- Crates.io versions are immutable. If a published crate proves defective, stop
  the train, yank only the affected version when necessary and issue a new
  release. A failure in Scry or Weld is not a reason to yank a healthy Graft.
- A public API change after publication receives a new crate version.

## Phases

### P1. `grafting` 0.6.1 and the changelog

*Landed 2026-11-01 (D1): `CHANGELOG.md` now has 0.6.0 and 0.6.1 headings, and
0.6.1 is recorded as breaking.*

`grafting` 0.6.1 moved the optional, default-on `surfman` feature from Surfman
0.13 to 0.14, and `surfman_gl::SurfmanFrameContext` exposes Surfman's `Device`,
`Context`, `Surface` and `Error`. The archived plan's 2026-10-22 cue judged that
move breaking and "the honest version is 0.7.0". A registry consumer of
`grafting = "0.6"` with default features now resolves the new Surfman types.
`CHANGELOG.md` still holds the 0.6.0 notes under `[Unreleased]`, with no 0.6.0
or 0.6.1 heading.

Done when the disposition of 0.6.1 is decided and recorded here (see Open
questions), and `CHANGELOG.md` has accurate 0.6.0 and 0.6.1 headings describing
what each published version contains, including the Surfman move.

### P2. The Graft 0.7 / Scry 0.8 / Weld 0.15.1 train

From the 2026-10-22 cues: `grafting` 0.7.0 carries the Surfman move and the
additive `grafting-frame` re-exports, DX12 custody, `DiagnosticGpuSync` and
`InteropError::NormalizationCompletion`. `scrying` re-exports
`grafting::Dx12SharedResource` (`scrying/src/native_frame/mod.rs:155`), so
moving its dependency is `scrying` 0.8.0. Weld's Grafting use is private, so
moving it is a compatible `welding` 0.15.1. Mere's `scrying-engine` requires
`scrying` "0.7.1", so Scry 0.8 needs a Mere change and the family repin. Shipping
Graft 0.7 alone gives registry consumers two Graftings. The Servo adapter
(`grafting-servo`) becomes publishable only by naming registry `servo` 0.7.0
instead of a Git revision, and only a Windows build of Turnstone's DX12 `no-wgl`
route qualifies it.

Done when, for one chosen exact public source per crate: declared-floor,
OS/wgpu, ownership/failure and platform-demo gates rerun; fresh headed receipts
exist on every claimed host; each crate is packaged and tagged from that source;
and the registry-only four-host proof (P3) passes with the new versions. Mere and
Turnstone repin in one coordinated set.

### P3. Gate mechanism and the four-host proof

The CI workflows, including `registry-only-triplet.yml`, were removed on
2026-10-28 (`0743b3b`: gates live in local scripts and dated receipts). The last
complete registry-only proof (`33916390001`) covers 0.6.0 / 0.7.1 / 0.14.1, not
the current heads; there is none for Weld 0.15.0 or Scry 0.7.2 on all four hosts.
Intel Metal cadence failed with an inactive page (`visibilityState = hidden`,
zero animation callbacks) and the NVIDIA runner had no listener; both were open
when CI was removed.

Done when a local script reproduces the registry-only staging and verification
(exact versions, one registry source per triplet crate, retained lock, metadata
and tree) and a dated receipt records green NVIDIA/DX12, M4/Metal, Intel/Metal
and RADV/Vulkan runs for the release under test, or the plan records which hosts
the claim drops.

### P4. Servo two-way GPU ordering fence

The shared GL/DX12 allocation's producer blit normally omits completion before
wgpu normalization, and normalization has no explicit wait before the next
shared-source write. `Existing` stays the default; `Both` in
`DiagnosticGpuSync` is a completion diagnostic, not the fence path.

Implement two-way producer/read/reuse ordering: D3D11 waits before shared-source
overwrite and signals after the GL blit and flush; the host D3D12 queue waits
before normalization and signals after its exact submission, through a
default-compatible source callback bound to the exact allocation rather than the
latest cache slot. Done when actual descriptor support, checked fence values,
precise errors and partial-handoff retirement are validated; all in-flight
texture and fence ownership survives resize and shutdown; and repeated pixel
controls plus lifecycle and failure behavior pass before the fence path is
adopted.

### P5. Servo consumer gates

Each lands with its own receipt; record it here.

- Foreign accessibility: an original AccessKit subtree per foreign surface with
  lifetime, bounds/focus mapping and action ownership by `(TreeId, NodeId)`.
  Servo 0.7's `forward_accessibility_action` executes only Click and forwards
  nodes with empty action masks, so native invocability needs its own supplier
  gate.
- Default reopened-view reliability (one of two `Existing` pixel reviews reopened
  view A white).
- Full process shutdown: the native donor still exits through `process::exit`.
- Fresh Linux and macOS Servo runtime qualification on v0.7.0.
- The Windows `scry,weld,servo` combined build: `quinn-proto`'s rustc reported a
  2 MiB allocation failure (`0xc0000409`); the serialized one-job retry has its
  own gate.
- Linux crash recovery (Weld's crash test is skipped on RADV).

### P6. Hygiene

- The archived Phase 1 condition "a forced Windows demo import error proves
  cleanup is not skipped by an early `?`" was not located in source. Add it or
  record why the owned API made it moot.
- Read-only formatting fails on 138 existing root paths and the Iced demo.
- Scry and Weld each still define their own `NativeFrame` beside
  `grafting-frame`; Weld's adoption is a breaking 0.16.
- Disjoint-fd multi-plane and YUV/ycbcr DMA-BUF import remain unsupported:
  Graft refuses them with `VulkanDisjointDmabufNotSupported`. Carried from Scry's
  archived 4a.x multi-plane plan (Mark, 2026-11-01: "Graft triplet plan"),
  since Graft now owns shared-fd multi-plane handling
  (`grafting/src/vulkan_dmabuf.rs`). Weld's producer gates plan lists the same
  refusal as a Linux import gap.

Done when each item lands or is recorded as dropped with its reason.

### P7. Combined host demo and the neutral contract

A polished host-owned wgpu browser-surface demo comparing Servo, system WebViews
and CEF under one host UI, without implying one process model or security
guarantee. Whether to extract `surface-engine-api` waits on Inker's correlated
asynchronous event model and capability truth.

## Open questions

- Whether to yank 0.6.1 once 0.7.0 is out. D1 does not decide it. `wgpu-cue`
  0.1.3 requires `grafting =0.6.1`, and Mere (`Cargo.toml:190`) and Turnstone
  (`Cargo.toml:317`) pin `wgpu-cue =0.1.3`, so a yank would follow a `wgpu-cue`
  release on 0.7.0 and those repins; existing locks keep working either way.
- Should Scry 0.8 adopt `grafting-frame`'s vocabulary? The cue calls the breaking
  release the cheapest moment; later adoption costs another breaking release.

## Decisions

- **D1, 0.6.1 stays published and is recorded as breaking (2026-11-01), settles
  P1:** Mark: "could we note 0.61 as having a breaking change? And since it's out
  right now, hold it until the next publish, and then publish grafting?"
  `CHANGELOG.md` records 0.6.1's Surfman move as breaking. Nothing is yanked now;
  the next publish is `grafting` 0.7.0 in the P2 train.

  Links: area release · plans triplet_release · summary 0.6.1 stays published with its break recorded; the next Graft publish is 0.7.0.

## Findings

- 2026-10-22: Scry 0.7.2 and Weld 0.15.0 require `grafting ^0.6.0`; Welding
  builds it with default features off. Turnstone pins `grafting` and
  `grafting-servo` by Git revision `9f410f19` and patches crates.io `grafting` to
  it, so one Grafting is shared; publishing `grafting` alone removes neither pin.
- 2026-10-22: Servo v0.7.0 (`aac43a3`, crates.io `servo` 0.7.0) pins glow 0.17,
  gleam 0.15 and Surfman 0.14; taking glow 0.18 or gleam 0.16 would split GL types
  across the adapter boundary.
- 2026-10-07: the measured Turnstone shared resource carries flags `0x21`
  (simultaneous access plus render target), supporting documented COMMON decay
  for that resource without qualifying other GPUs.
- 2026-10-06: Genet `679d8314` resolves the fontsan `woff2` versus `libz-sys,wuff`
  decoder collision in the combined `scry,weld,servo` graph.
- 2026-11-01: history was rewritten; commit hashes cited in the archived plan may
  not be ancestors of `main`. The `grafting-v0.6.0` tag peels to `10963f8`,
  `grafting-v0.6.1` to `50d5f29`, `grafting-frame-v0.1.1` to `3b9b5f6`.
- 2026-11-01: `surfman` is a default feature of `grafting`
  (`grafting/Cargo.toml:12`).

## Progress

- 2026-11-01: cut from the September triplet release plan after a closure audit.
  Its phases 1-6 are marked landed in the archived copy.
- 2026-11-01: P1 landed (D1): `CHANGELOG.md` gains 0.6.0 and 0.6.1 headings with
  0.6.1's Surfman move recorded as breaking.
