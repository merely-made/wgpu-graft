# Servo pre-present import and swap correction

The qualified supplier correction is committed at
`dec11bbd2c9c8676e66987fb6ca32cd4ae6310eb`. Its adapter and donor source hashes
match the native smoke and fresh default29 typed inputs recorded below. A
subsequent documentation-only update points the installation example to that
immutable source; it changes neither implementation nor manifests.

`ServoWgpuInteropAdapter::rendering_context()` now supplies its importing
context to upstream Servo. The old raw context bypassed the pre-present hook,
so a consumer of `take_imported_texture()` received no painted frame. The hook
also clears a previous unconsumed texture before each attempt, preventing an
import failure from presenting an older success as the current paint. The
winit demo consumes this hook result after `WebView::paint()`; its existing
GPU smoke path now fails when the hook produces no frame.

The pinned upstream `WebView::paint()` renders without presenting. Its own
servoshell explicitly presents after painting (`ports/servoshell/window.rs`,
`repaint_webviews`). The host sequence must therefore be `WebView::paint()`,
then `adapter.rendering_context().present()`, then `take_imported_texture()`.
Returning the importing context alone does not invoke the hook. This required
host/donor correction was identified during the native build; the typed checks
below predate that call-site correction and are not runtime qualification.

All nine Servo dependency declarations and both lockfiles select the exact
upstream revision `1d44e5dd6a8b64c02f9dbf7fcbdf4ebdd0740019`. Each lock has one
distinct Servo source query. The lock edits replace only the branch query with
the immutable revision query, retaining package versions and dependency lists.

These files are diagnostics, with their limits retained:

- `graft-ordered-events-test.log`: four focused Mere Graft tests and doctests
  passed. Mere commit `69cb39214d5fedb5bfb665383393c383f01aaa45` preserves a
  producer's FIFO events across navigation and message kinds. This is shared
  adapter evidence, not a running upstream Servo browser.
- `adapter-wgpu30-mutable-query-partial.log`: the earlier branch-source cold
  check was stopped before adoption of the immutable source. It is incomplete
  and cannot qualify the final source graph.
- `immutable-lock-update.log`: offline broad lock regeneration refused an
  unrelated GPUI Core Foundation cache mismatch. The exact dependency policy
  was retained; no version relaxation was adopted.
- `immutable-metadata-unbuilt-platform.log`: complete offline workspace
  metadata needs uncached `accesskit 0.21.1` from an unbuilt demo/platform.
- `scenario_parse.rs` and its logs: source grammar diagnostics for the
  Turnstone fixture. The failed app-only attempt predates use of both actual
  Taproot and Turnstone parsers. A successful grammar parse is not a host build
  or native browser receipt.

The immutable-source `wgpu-30,servo` adapter check passed (exit 0), including the
full upstream Servo graph, in 12 minutes 20 seconds with supplier Rust 1.97.1,
desktop LLVM and the Visual Studio 2022 CMake generator. Its byte-preserved
finished transcript is `adapter-wgpu30-immutable-check.log`. The source checked
was supplier commit `01f3c9f3d68df3247e6b7ec7f65ffbca57e9b2d4` at the exact Servo
revision above. The command used the reusable target:

```powershell
$env:LIBCLANG_PATH = 'C:/Program Files/LLVM/bin'
$env:CMAKE_GENERATOR = 'Visual Studio 17 2022'
cargo check --offline --locked -p servo-wgpu-interop-adapter --no-default-features --features wgpu-30,servo --target-dir C:/t/cargo-targets/wgpu-graft -j 2
```

That typed check does not enable upstream Servo's Windows `no-wgl` feature and
cannot qualify delivery of its ANGLE DLLs. The winit proof host now enables
`no-wgl` on Windows only, using the same exact source as its generic dependency.
This feature builds mozangle's `libEGL.dll` and `libGLESv2.dll`; those DLLs need
staging beside the actual host executable before a native run. The other demo
feature policies are unchanged.

The completed repeat adapter check also passed with `--locked`, recorded in
`adapter-wgpu30-immutable-repeat.log`. The winit donor now keeps `wgpu-29,servo`
as its default and offers `--no-default-features --features wgpu-30,servo`,
forwarding exactly that GPU selection to the adapter and Graft. Both or neither
GPU selections are rejected by an explicit compile guard. Its lockfile change
adds one existing `wgpu 30.0.1` dependency edge for that donor only. The initial
locked attempt refused that absent edge (`winit-wgpu30-check.log`); no package
version change was adopted.

The selected Windows donor typed check passed with `wgpu-30,servo` and `no-wgl`
in 2 minutes 32 seconds (`winit-wgpu30-check-fixed.log`). The preceding attempt
compiled the native prerequisites but exposed four wgpu 30 API differences,
recorded in `winit-wgpu30-check-locked.log`. The donor fixes supply the new
request-adapter option and surface color space, handle the fallible mapped
range, and present through the queue. Its default wgpu 29 branches are retained.
The command is the adapter command above with `-p demo-servo-winit`.

That check built these exact mozangle DLLs under
`C:/t/cargo-targets/wgpu-graft/debug/build/mozangle-369fc6b9fd278688/out`, using
MSVC 14.51.36231 selected by `cc` from Visual Studio 18/Insiders. The CMake
generator setting above is separate from that compiler selection.

| DLL | Bytes | SHA-256 |
| --- | ---: | --- |
| `libEGL.dll` | 569344 | `06E79A1A8EF2A65751160BEA504645E854EC6CCB315339CDA1F79055CB3EE6F5` |
| `libGLESv2.dll` | 19594240 | `A78527011E2EF9DC954B79E936FD459A3DF81E2D30EDC3C8453F9C49D44C233E` |

Their `angle-*-pe-imports.log` files record actual PE import tables for these
typed-check outputs. They do not establish which DLLs a host loads. Native
codegen rebuilds ANGLE in a distinct Cargo unit; its completed OUT_DIR and DLL
hashes must be recorded separately. `run-winit-smoke.ps1` refuses execution
until bound to the executable and its completed native outputs. It stages only their
hash-verified DLLs and record the owned donor's loaded module paths, hashes,
and file/product versions.
The first native compilation passed in 58 minutes 1 second, but predates the
explicit present call and remains a diagnostic (`winit-wgpu30-build.log` and
`winit-pre-present-build-inputs.json`). It produced native ANGLE prerequisites
in `mozangle-f2b6807973fe390c/out`; `angle-native-outputs.json` records their
distinct hashes and absent file/product version resources, and the
`angle-native-*-pe-imports.log` files preserve their import tables. These
completed prerequisites do not qualify a running host.

The corrected source also passed the default wgpu 29 typed check in 35 minutes
12 seconds, including shared-cache waits (`winit-default29-check.log`):

```powershell
cargo check --offline --locked -p demo-servo-winit --target-dir C:/t/cargo-targets/wgpu-graft -j 2
```

The corrected-source wgpu 30 native rebuild passed in 6 minutes 40 seconds,
including shared-cache waits (`winit-wgpu30-present-build.log`).
`winit-final-build-inputs.json` preserves its source and executable hashes;
`winit-wgpu30-final-compiler-inputs.json` identifies the actual native ANGLE
`mozangle-f2b6807973fe390c/out` linked by that compiler invocation.

Both runs of that exact executable failed before the initial pixel assertion,
with Surfman 0.13's preservation blit asserting GL error 1282. The second run's
backtrace locates the failure in `SwapChainData::swap_buffers`, called by the
raw adapter's `present`, inside the importing context's explicit `present`.
The `winit-wgpu30-smoke-failed-*` and `winit-wgpu30-smoke-backtrace-*` receipts
preserve stdout, stderr, exit 101, executable hashes, and both loaded ANGLE
module paths and hashes. `run-winit-smoke-pre-control.ps1` preserves that
runner. These native failures qualify neither import nor displayed pixels.

A subsequent bounded `--smoke --raw-present-control` mode tests the same raw
context swap without running the import hook. Its first compile failed on a
missing trait import (`winit-raw-control-compile-failed.json`); the corrected
build passed in 3 minutes 4 seconds. Its exact source/executable/native DLL
inputs and PE imports are recorded in `winit-raw-control-build-inputs.json`
and `winit-wgpu30-raw-control-pe-imports.log`. The raw control exited 101 in
9.040338 seconds with the same Surfman preservation assertion, with the raw
adapter directly on the stack and no importing context. Its
`winit-wgpu30-raw-present-control*` receipts establish that the swap failure is
reproducible without the import hook. `run-winit-smoke-raw-control.ps1`
preserves its hash-bound runner. Its separate marker and scope cannot count
as imported-frame qualification. The pinned upstream
[`software context`](https://github.com/servo/servo/blob/1d44e5dd6a8b64c02f9dbf7fcbdf4ebdd0740019/components/shared/paint/rendering_context.rs#L363-L368)
swaps with `PreserveBuffer::No`; the supplier currently uses preservation.
Upstream's separate offscreen context renders into a private FBO and has an
empty `present`, with a color-only blit callback to the native parent context.
These differences are construction leads, not evidence that changing the
supplier is safe after input or resize.
Surfman 0.13's Windows ANGLE `Device::surface_info` reports no named framebuffer
object for either pbuffer (`src/angle/device.rs:1146`). Its preservation branch
swaps to the new back buffer, then binds both source and destination using
those default framebuffer IDs and blits color, depth, and stencil
(`src/chains.rs:184-213`). A same-current-default-framebuffer blit is a source
inference consistent with error 1282. The raw control now reproduces the same
failure without the importer.

The subsequent supplier correction uses `PreserveBuffer::No` while retaining
`GL.finish`, frame custody, and import-before-swap order. The pinned
[`paint implementation`](https://github.com/servo/servo/blob/1d44e5dd6a8b64c02f9dbf7fcbdf4ebdd0740019/components/paint/painter.rs#L398-L437)
clears the entire context and renders with buffer age zero on every paint.
This supports discarding old pixels in the next back buffer. The corrected
native build passed in 2 minutes 7 seconds (`winit-wgpu30-no-preserve-build.log`).
`winit-no-preserve-build-inputs.json` freezes the exact source, fixture,
executable, and native DLL hashes; the separate compiler-input and PE-import
logs retain actual native linkage.

The subsequent native wgpu 30 smoke passed in 2.5015623 seconds, observing
49 imported frames. Its initial pixel was exactly `[23, 97, 181, 255]` at
1024x600. After a real center mouse down/up and resize, it observed exactly
`[221, 79, 54, 255]` at 960x640. Pixel tolerance 18 and the existing missing-GPU
failure remain unchanged. `winit-wgpu30-smoke.*.log` and its provenance JSON
identify executable `26C74CA2812D39737DB94D6D96C9CD4FA326DFF4C1422D9AFCE473C229D7995C`
and both loaded ANGLE DLLs from the completed native `f2` unit. Stderr warnings
are retained. This qualifies this donor's GPU initial/click/resize path only.
The changed-source default wgpu 29 typed check passed in 3 minutes 10 seconds
(`winit-default29-no-preserve-check.log`) with `-j 1`, BelowNormal priority,
the same locked source graph, and the donor's default features. It checked both
the changed adapter and retained wgpu 29 donor branches. This is typed
compatibility, distinct from the wgpu 30 Windows hardware result.

The ordinary fallback's repaint is source-supported: pinned
`servo/webview.rs:755-757` invokes `paint/paint.rs:714-717`, which calls
`Painter::render` unconditionally. The repaint selector does not gate explicit
painting. The first failed import has already swapped, so the donor explicitly
paints the current buffer before CPU readback. CPU fallback remains unqualified
and cannot satisfy the GPU smoke.

The smoke measures an initial
imported center pixel, a real click changing that pixel, and resize to 960x640.
It does not qualify text input, top/bottom orientation, or full teardown:
successful donor smoke uses `process::exit`, leaving the real Turnstone
process-root shutdown gate open.

Remaining gates are the actual Turnstone consumer feature build and native
page/input/resize/orientation/
close-reopen/shutdown evidence. Windows runtime proof must build and identify
the matching mozangle `libEGL.dll` and `libGLESv2.dll`, rather than substitute
similarly named libraries from another browser supplier. No release, tag, or
Turnstone native qualification is asserted by this receipt.

The subsequent accessibility ruling also requires each foreign view to join
the host AccessKit tree. Pixel qualification alone cannot complete that gate.
The pinned Servo exposes
[`WebView::set_accessibility_active`](https://github.com/servo/servo/blob/1d44e5dd6a8b64c02f9dbf7fcbdf4ebdd0740019/components/servo/webview.rs#L900-L999)
and a delegate `notify_accessibility_tree_update` callback, retaining separate
view and document tree IDs. Its activation contract requires the host graft
node to be submitted before any corresponding subtree update. The current
Turnstone Servo host has not activated or forwarded these trees through Inker.
Upstream's
[`servoshell` action branch](https://github.com/servo/servo/blob/1d44e5dd6a8b64c02f9dbf7fcbdf4ebdd0740019/ports/servoshell/desktop/headed_window.rs#L784-L796)
still marks forwarding non-root assistive actions as TODO; no public Servo
semantic accessibility-action handler was found in this exact source. These
are open integration and supplier gates, not native accessibility results.

Read-only inspection of Servo 0.7.0 at immutable commit
`aac43a3f31a259f04a574f5ec4e959c943ad7cc7` found a newer public
[`Servo::forward_accessibility_action`](https://github.com/servo/servo/blob/aac43a3f31a259f04a574f5ec4e959c943ad7cc7/components/servo/servo.rs#L1120-L1131).
Its [constellation routing](https://github.com/servo/servo/blob/aac43a3f31a259f04a574f5ec4e959c943ad7cc7/components/constellation/constellation.rs#L3259-L3277)
uses `target_tree`, dropping unknown trees; script and layout queue the action
through reflow. The [retained node lookup](https://github.com/servo/servo/blob/aac43a3f31a259f04a574f5ec4e959c943ad7cc7/components/layout/accessibility/mod.rs#L1447-L1461)
drops stale node IDs. The final [DOM handler](https://github.com/servo/servo/blob/aac43a3f31a259f04a574f5ec4e959c943ad7cc7/components/script/dom/window/window.rs#L3832-L3843)
implements only `Click`, firing an untrusted synthetic event. Other actions
fall through and the public method returns no completion or typed refusal.
Its AccessKit requirement remains the 0.24 family. This creates a concrete
migration candidate for assistive click routing; focus, text/value/scroll,
native platform results, and host graft integration remain open. The baseline
lacks this internal routing/queue/reflow/action chain, so a backport requires
engine changes beyond a public wrapper. No Servo upgrade was adopted here.
The producer chain also lacks action advertisement.
`layout/accessibility/mod.rs:129-136` makes the retained node and its AccessKit
data private; its sole constructor at `750-759` creates `Node::new(role)`.
Every field mutation in the complete module changes structural, textual, or
geometric properties; none sets actions or child actions. Finalization at
`1428-1442` clones those nodes into the update. `layout/layout_impl.rs:1003-1023`
sends that update unchanged, `servo/servo.rs:783-787` dispatches it unchanged,
and `servo/webview.rs:998-1019` filters epochs and forwards unchanged. Its
wrapper nodes at `952` and `980` also have empty action masks.
`servo-0.7-accessibility-source.json` identifies these immutable raw files,
hashes, and source lines.

AccessKit 0.24.1 `src/lib.rs:1737-1777` initializes action masks empty and
checks their bits. AccessKit consumer 0.35 `src/node.rs:595-596,638-666`
requires advertised `Click` support (or the direct parent's child action) for
invocability; Windows 0.32.1 `src/node.rs:572` uses that result for UIA Invoke.
Thus forwarding a programmatic click and advertising native invocability are
separate gates. Native requests, other actions, and host graft integration
remain unqualified.

The named process profile is backed by upstream construction, rather than
only a host label. At exact Servo commit
`1d44e5dd6a8b64c02f9dbf7fcbdf4ebdd0740019`,
[`components/servo/servo.rs`](https://github.com/servo/servo/blob/1d44e5dd6a8b64c02f9dbf7fcbdf4ebdd0740019/components/servo/servo.rs#L965-L981)
passes `opts.config_dir` to the resource and storage threads.
[`components/net/resource_thread.rs`](https://github.com/servo/servo/blob/1d44e5dd6a8b64c02f9dbf7fcbdf4ebdd0740019/components/net/resource_thread.rs#L209-L213)
loads the authentication cache, HSTS list, and cookie jar from that directory;
[`components/storage/storage_thread.rs`](https://github.com/servo/servo/blob/1d44e5dd6a8b64c02f9dbf7fcbdf4ebdd0740019/components/storage/storage_thread.rs#L20-L60)
passes it to client, web, and cache storage construction. These are source
observations. Turnstone's host cookie API remains unsupported, and runtime
persistence has not been qualified by either the donor smoke or the browser
fixture.

The official crates.io adapter lookup returned HTTP 404 on 2026-10-07 UTC
(2026-10-06 local), stating that `servo-wgpu-interop-adapter` does not exist.
`adapter-crates-io-status.json` preserves the timestamp, endpoint, status, and
response. The adapter README now uses the exact qualified Git correction
`dec11bbd2c9c8676e66987fb6ca32cd4ae6310eb`. Turnstone's prior candidate
`01f3c9f3d68df3247e6b7ec7f65ffbca57e9b2d4` predates this native swap fix;
consumer adoption and its native gates remain separate. This lookup does not
describe the separate published Graft core package.

## GPU ordering diagnostic controls

The follow-up adds an explicit, default-off diagnostic to the Windows
ANGLE/DX12 Servo pre-present import. `DiagnosticGpuSync::Existing` retains
the existing path. `ProducerCompletion` finishes the GL shared-texture blit
before normalization reads it. `NormalizationCompletion { timeout }` waits
for the exact wgpu normalization submission before returning the frame;
`Both { timeout }` selects both waits. The caller supplies the queue-wait
timeout. GL finish itself has no timeout, so native controls retain a guarded
process deadline. These are GPU waits, with no CPU pixel fallback.

The source ordering gap is explicit: the producer normally blits into a
size-cached shared D3D11/D3D12 allocation with `finish=false`; normalization
submits its read/copy asynchronously; the importing hook calls the raw
context's GL finish after that submission. The default synchronizer validates
the declared mode without adding an external fence or waiting for the copy.
Submission alone establishes neither GL-write completion before the wgpu
read nor wgpu-read completion before the next GL overwrite. Submitted wgpu
work retains source texture custody; this is an ordering concern, not a
dropped-lease finding.

`ServoWgpuInteropAdapter::new_with_diagnostic_sync` forwards the selected
control into the pre-present hook. `take_imported_texture_result` returns
the original typed `wgpu::PollError` through
`InteropError::NormalizationCompletion`, or a GL import/completion error.
A failed diagnostic blocks subsequent import/swap attempts; the caller must
retire the context on error. Native diagnostics require Windows DX12, a GL
framebuffer source, and origin normalization. Existing constructors and
their default synchronization remain unchanged.

Turnstone ran six guarded, same-executable controls with independent fresh
application/Servo profiles and a 5000 ms queue-wait timeout:

| Mode | Complete pixel checks passed | Native exit / final producers, caches, views |
| --- | --- | --- |
| Existing | 1 of 2 | Both exit 0; all final counts zero |
| Producer completion | 1 of 1 | Exit 0; all final counts zero |
| Normalization completion | 1 of 1 | Exit 0; all final counts zero |
| Both | 2 of 2 | Both exit 0; all final counts zero |

The second Existing run reproduced a completely white reopened A view while
B remained correct. Its title/address assertions and `RESULT ok` still
passed; pixel review rejected the run. All six controls exited normally
without timeout. The bounded sample shows an association with the waits;
it does not establish visual causality or a production fix. The default stays
Existing. Accessibility, coordinated consumer adoption, and release gates
are not accepted by these observations.

`turnstone-sync-diagnostic-comparison.json` is a byte-preserved copy of
Turnstone's `docs/receipts/browser_supplier_integration_20261006/sync-diagnostic/comparison.json`
(SHA-256 `3d2635abb102d261c90855be39ea52313fde544e3aeb6d72e401ba80034d30c2`).
It records all six native-file hashes, 98 distinct passing browser tests,
the 38-input supplier archive hash, and the root source archive hash. Full
captures and source archives remain in that Turnstone receipt tree. No donor
smoke substitutes for these consumer controls.

After the controls, only the new queue-wait expression was formatted.
`grafting-lib-before-sync-poll-format.rs` preserves its exact native-control
source. `sync-poll-format-equivalence.json` proves full-file rustfmt-normalized
text identical before/after. Fresh, sequential `-j1` BelowNormal typed checks
passed for adapter wgpu 30 (26.13 s), core wgpu 28 with surfman (5.00 s), and
core wgpu 29 with surfman (5.91 s). The postformat inputs, logs, and result
JSON retain exact source/manifest/lock hashes; native controls used the
preformat bytes. These checks qualify Windows type compatibility, not other
platform runtime behavior.

The read-only whole-workspace formatting check failed with 138 reported
paths. Per-owned-file HEAD/current comparisons isolate the one new poll
formatting hunk, now corrected; existing formatting debt is preserved.
`supplier-sync-diagnostic-fmt-check.log`, its result JSON, and the per-file
baseline/current diffs record that failed gate. This diagnostic checkpoint
does not claim a green whole-workspace formatting or release gate.
