# Servo pre-present import correction (qualification pending)

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

The immutable-source `wgpu-30,servo` adapter check is still running with desktop
LLVM and the Visual Studio 2022 CMake generator in the reusable
`C:/t/cargo-targets/wgpu-graft` target. Its active transcript is excluded from
this commit until completion. This check does not enable upstream Servo's
Windows `no-wgl` feature and cannot qualify delivery of its ANGLE DLLs.

Remaining gates are the completed exact-source typed check, the actual
Turnstone consumer feature build, and native page/input/resize/orientation/
close-reopen/shutdown evidence. Windows runtime proof must build and identify
the matching mozangle `libEGL.dll` and `libGLESv2.dll`, rather than substitute
similarly named libraries from another browser supplier. No release, tag, or
native qualification is asserted by this receipt.
