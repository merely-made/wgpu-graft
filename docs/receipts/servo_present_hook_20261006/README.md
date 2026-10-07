# Servo pre-present import correction (native qualification pending)

`ServoWgpuInteropAdapter::rendering_context()` now supplies its importing
context to upstream Servo. The old raw context bypassed the pre-present hook,
so a consumer of `take_imported_texture()` received no painted frame. The hook
also clears a previous unconsumed texture before each attempt, preventing an
import failure from presenting an older success as the current paint. The
winit demo consumes this hook result after `WebView::paint()`; its existing
GPU smoke path now fails when the hook produces no frame.

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
version change was adopted. The subsequent selected donor build and its native
smoke remain pending, and their active transcript is excluded until completion.

Remaining gates are the actual Turnstone consumer feature build and native
page/input/resize/orientation/
close-reopen/shutdown evidence. Windows runtime proof must build and identify
the matching mozangle `libEGL.dll` and `libGLESv2.dll`, rather than substitute
similarly named libraries from another browser supplier. No release, tag, or
native qualification is asserted by this receipt.
